use super::token_tree::TokenTree;
use glyim_syntax::SyntaxKind;
use smol_str::SmolStr;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) enum PatternPiece {
    Token(SyntaxKind, SmolStr),
    Repetition {
        inner: Vec<PatternPiece>,
        separator: Option<TokenTree>,
        kind: RepetitionKind,
    },
    Metavar {
        name: SmolStr,
        fragment: FragmentSpec,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FragmentSpec {
    Expr,
    Ty,
    Ident,
    Path,
    Block,
    Stmt,
    Item,
    Pat,
    Lifetime,
    Literal,
    Vis,
    Meta,
    Tt,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RepetitionKind {
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Clone, Debug)]
pub(crate) struct Pattern {
    pieces: Vec<PatternPiece>,
}

#[derive(Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum MatchResult {
    FullMatch(HashMap<SmolStr, Vec<TokenTree>>),
    PartialMatch,
    NoMatch,
}

impl Pattern {
    pub(crate) fn new(pieces: Vec<PatternPiece>) -> Self {
        Self { pieces }
    }
}

pub(crate) fn parse_pattern_from_node(node: &glyim_syntax::SyntaxNode) -> Option<Pattern> {
    let trees = super::token_tree::collect_token_trees(node);
    parse_pattern(&trees)
}

fn parse_pattern(trees: &[TokenTree]) -> Option<Pattern> {
    // If the pattern is wrapped in a Group (e.g., (...) from CST), unwrap it.
    let effective = if trees.len() == 1 {
        if let TokenTree::Group(_, inner, _) = &trees[0] {
            inner.as_slice()
        } else {
            trees
        }
    } else {
        trees
    };
    let (pieces, _) = parse_pattern_pieces(effective, 0)?;
    Some(Pattern::new(pieces))
}

fn parse_pattern_pieces(trees: &[TokenTree], pos: usize) -> Option<(Vec<PatternPiece>, usize)> {
    let mut pieces = Vec::new();
    let mut i = pos;
    while i < trees.len() {
        let tree = &trees[i];
        if let TokenTree::Token(SyntaxKind::Dollar, _) = tree {
            i += 1;
            if i >= trees.len() {
                return None;
            }
            match &trees[i] {
                TokenTree::Token(SyntaxKind::Ident, name) => {
                    let name = name.clone();
                    i += 1;
                    if i >= trees.len() {
                        return None;
                    }
                    if !matches!(&trees[i], TokenTree::Token(SyntaxKind::Colon, _)) {
                        return None;
                    }
                    i += 1;
                    if i >= trees.len() {
                        return None;
                    }
                    let fragment = parse_fragment_spec(&trees[i])?;
                    i += 1;
                    pieces.push(PatternPiece::Metavar { name, fragment });
                }
                TokenTree::Group(SyntaxKind::LParen, inner, SyntaxKind::RParen) => {
                    let (inner_pieces, _) = parse_pattern_pieces(inner, 0)?;
                    i += 1;
                    let separator = if i < trees.len() {
                        let sep_tree = &trees[i];
                        if matches!(
                            sep_tree,
                            TokenTree::Token(
                                SyntaxKind::Star | SyntaxKind::Plus | SyntaxKind::Question,
                                _
                            )
                        ) {
                            None
                        } else {
                            let sep = sep_tree.clone();
                            i += 1;
                            Some(sep)
                        }
                    } else {
                        None
                    };
                    if i >= trees.len() {
                        return None;
                    }
                    let rep_kind = match &trees[i] {
                        TokenTree::Token(SyntaxKind::Star, _) => RepetitionKind::ZeroOrMore,
                        TokenTree::Token(SyntaxKind::Plus, _) => RepetitionKind::OneOrMore,
                        TokenTree::Token(SyntaxKind::Question, _) => RepetitionKind::ZeroOrOne,
                        _ => return None,
                    };
                    i += 1;
                    pieces.push(PatternPiece::Repetition {
                        inner: inner_pieces,
                        separator,
                        kind: rep_kind,
                    });
                }
                _ => {
                    pieces.push(PatternPiece::Token(SyntaxKind::Dollar, SmolStr::from("$")));
                }
            }
        } else {
            pieces.push(PatternPiece::Token(tree.kind()?, tree.text()));
            i += 1;
        }
    }
    Some((pieces, i))
}

fn parse_fragment_spec(tree: &TokenTree) -> Option<FragmentSpec> {
    match tree {
        TokenTree::Token(SyntaxKind::Ident, text)
        | TokenTree::Token(SyntaxKind::Lifetime, text) => match text.as_str() {
            "expr" => Some(FragmentSpec::Expr),
            "ty" => Some(FragmentSpec::Ty),
            "ident" => Some(FragmentSpec::Ident),
            "path" => Some(FragmentSpec::Path),
            "block" => Some(FragmentSpec::Block),
            "stmt" => Some(FragmentSpec::Stmt),
            "item" => Some(FragmentSpec::Item),
            "pat" => Some(FragmentSpec::Pat),
            "lifetime" => Some(FragmentSpec::Lifetime),
            "literal" => Some(FragmentSpec::Literal),
            "vis" => Some(FragmentSpec::Vis),
            "meta" => Some(FragmentSpec::Meta),
            "tt" => Some(FragmentSpec::Tt),
            _ => None,
        },
        _ => None,
    }
}

/// Check if a token tree matches a fragment specifier.
fn matches_fragment_spec(tree: &TokenTree, spec: &FragmentSpec) -> bool {
    match spec {
        FragmentSpec::Ident => matches!(tree, TokenTree::Token(SyntaxKind::Ident, _)),
        FragmentSpec::Literal => matches!(
            tree,
            TokenTree::Token(
                SyntaxKind::IntLit
                    | SyntaxKind::FloatLit
                    | SyntaxKind::StringLit
                    | SyntaxKind::BoolLit
                    | SyntaxKind::CharLit,
                _
            )
        ),
        FragmentSpec::Lifetime => matches!(tree, TokenTree::Token(SyntaxKind::Lifetime, _)),
        FragmentSpec::Tt => true,
        // For expr, ty, path, etc. — match greedily (any token tree is acceptable)
        // These are complex to parse correctly, so we accept any single token tree
        // and let the parser validate later.
        FragmentSpec::Expr
        | FragmentSpec::Ty
        | FragmentSpec::Path
        | FragmentSpec::Block
        | FragmentSpec::Stmt
        | FragmentSpec::Item
        | FragmentSpec::Pat
        | FragmentSpec::Vis
        | FragmentSpec::Meta => true,
    }
}

/// Check if a token kind is a closing delimiter that should stop greedy matching.
fn is_closing_delim(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::RParen | SyntaxKind::RBrace | SyntaxKind::RBracket
    )
}

pub(crate) fn match_pattern(pattern: &Pattern, input: &[TokenTree]) -> MatchResult {
    let mut bindings: HashMap<SmolStr, Vec<TokenTree>> = HashMap::new();
    match match_pieces(&pattern.pieces, input, 0, &mut bindings) {
        Ok((consumed, _)) if consumed == input.len() => MatchResult::FullMatch(bindings),
        Ok((_, _)) => MatchResult::PartialMatch,
        Err(()) => MatchResult::NoMatch,
    }
}

fn match_pieces(
    pieces: &[PatternPiece],
    input: &[TokenTree],
    pos: usize,
    bindings: &mut HashMap<SmolStr, Vec<TokenTree>>,
) -> Result<(usize, usize), ()> {
    let mut i = pos;
    for piece in pieces {
        match piece {
            PatternPiece::Token(expected_kind, expected_text) => {
                if i >= input.len() {
                    return Err(());
                }
                let input_tree = &input[i];
                match input_tree {
                    TokenTree::Token(k, t) => {
                        if *k != *expected_kind || t != expected_text {
                            return Err(());
                        }
                        i += 1;
                    }
                    _ => return Err(()),
                }
            }
            PatternPiece::Metavar { name, fragment } => {
                match fragment {
                    // For expr/ty/path/etc., match greedily: consume tokens
                    // until we hit a closing delimiter or end of input.
                    // But only if the next pattern piece can still match after.
                    FragmentSpec::Expr
                    | FragmentSpec::Ty
                    | FragmentSpec::Path
                    | FragmentSpec::Block
                    | FragmentSpec::Stmt
                    | FragmentSpec::Item
                    | FragmentSpec::Pat
                    | FragmentSpec::Vis
                    | FragmentSpec::Meta => {
                        if i < input.len() {
                            // If this is the last piece, consume all remaining tokens
                            // Otherwise, try to find how many tokens to consume
                            // For simplicity, if it's the last piece, consume all
                            // For non-last pieces, try consuming one token at a time
                            // and see if the remaining pieces match
                            let _remaining_pieces = &pieces[pieces.iter().position(|p| matches!(p, PatternPiece::Metavar { name: n, .. } if n == name)).map_or(0, |idx| idx + 1)..];
                            // Check if there are more pieces after this metavar
                            let has_more = pieces.iter().position(|p| matches!(p, PatternPiece::Metavar { name: n, .. } if n == name))
                                .is_some_and(|idx| idx + 1 < pieces.len());

                            if !has_more {
                                // Last piece: consume all remaining tokens that match fragment spec
                                let mut captured = Vec::new();
                                while i < input.len() {
                                    let tree = &input[i];
                                    // Stop at closing delimiters that aren't wrapped in a group
                                    if let TokenTree::Token(kind, _) = tree
                                        && is_closing_delim(*kind)
                                    {
                                        break;
                                    }
                                    if matches_fragment_spec(tree, fragment) {
                                        captured.push(tree.clone());
                                        i += 1;
                                    } else {
                                        break;
                                    }
                                }
                                bindings.entry(name.clone()).or_default().extend(captured);
                            } else {
                                // Not the last piece: try to match greedily but leave tokens for remaining pieces
                                // Strategy: try consuming all tokens first, then backtrack
                                let best_match = try_greedy_match(
                                    name, fragment, input, i, &pieces[pieces.len()..], bindings,
                                );
                                // Fallback: just consume one token
                                if let Some((captured, new_i)) = best_match {
                                    i = new_i;
                                    bindings.entry(name.clone()).or_default().extend(captured);
                                } else if i < input.len() && matches_fragment_spec(&input[i], fragment) {
                                    let captured = vec![input[i].clone()];
                                    i += 1;
                                    bindings.entry(name.clone()).or_default().extend(captured);
                                } else if i < input.len() {
                                    // Token doesn't match fragment spec
                                    return Err(());
                                }
                            }
                        }
                    }
                    // For ident/literal/lifetime/tt, match exactly one token tree
                    FragmentSpec::Ident | FragmentSpec::Literal | FragmentSpec::Lifetime | FragmentSpec::Tt => {
                        if i < input.len() {
                            if matches_fragment_spec(&input[i], fragment) {
                                let captured = vec![input[i].clone()];
                                i += 1;
                                bindings.entry(name.clone()).or_default().extend(captured);
                            } else {
                                // Token doesn't match the fragment spec
                                return Err(());
                            }
                        } else {
                            // No token to match — metavar at end of input
                            // Only allow this for optional-like contexts
                            return Err(());
                        }
                    }
                }
            }
            PatternPiece::Repetition {
                inner,
                separator,
                kind,
            } => {
                let mut repetitions: Vec<HashMap<SmolStr, Vec<TokenTree>>> = Vec::new();
                loop {
                    let mut rep_bindings: HashMap<SmolStr, Vec<TokenTree>> = HashMap::new();
                    match match_pieces(inner, input, i, &mut rep_bindings) {
                        Ok((new_i, _matched_count)) => {
                            // Require at least one token matched if inner is non-empty
                            if new_i == i && !inner.is_empty() {
                                break;
                            }
                            i = new_i;
                            repetitions.push(rep_bindings);
                            // Check for separator
                            if let Some(sep) = separator {
                                if i < input.len() && input[i] == *sep {
                                    i += 1;
                                } else {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let count = repetitions.len();
                match kind {
                    RepetitionKind::ZeroOrMore => {
                        for rep in &repetitions {
                            for (k, v) in rep {
                                bindings.entry(k.clone()).or_default().extend(v.clone());
                            }
                        }
                    }
                    RepetitionKind::OneOrMore => {
                        if count == 0 {
                            return Err(());
                        }
                        for rep in &repetitions {
                            for (k, v) in rep {
                                bindings.entry(k.clone()).or_default().extend(v.clone());
                            }
                        }
                    }
                    RepetitionKind::ZeroOrOne => {
                        if count > 1 {
                            return Err(());
                        }
                        for rep in &repetitions {
                            for (k, v) in rep {
                                bindings.entry(k.clone()).or_default().extend(v.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    Ok((i, i - pos))
}

/// Try greedy matching for a metavar that isn't the last piece.
/// Returns the captured tokens and the new position if successful.
fn try_greedy_match(
    _name: &SmolStr,
    fragment: &FragmentSpec,
    input: &[TokenTree],
    start: usize,
    _remaining: &[PatternPiece],
    _bindings: &HashMap<SmolStr, Vec<TokenTree>>,
) -> Option<(Vec<TokenTree>, usize)> {
    // Simple strategy: consume one token that matches the fragment spec
    if start < input.len() && matches_fragment_spec(&input[start], fragment) {
        Some((vec![input[start].clone()], start + 1))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyim_syntax::SyntaxKind;

    fn tok(kind: SyntaxKind, text: &str) -> TokenTree {
        TokenTree::Token(kind, SmolStr::from(text))
    }

    #[test]
    fn test_metavar_matches_one_token() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Expr,
        }]);
        let input = vec![tok(SyntaxKind::IntLit, "42")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
    }

    #[test]
    fn test_metavar_ident_matches_ident() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Ident,
        }]);
        let input = vec![tok(SyntaxKind::Ident, "foo")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
    }

    #[test]
    fn test_metavar_ident_rejects_literal() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Ident,
        }]);
        let input = vec![tok(SyntaxKind::IntLit, "42")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn test_metavar_literal_matches_int() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Literal,
        }]);
        let input = vec![tok(SyntaxKind::IntLit, "42")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
    }

    #[test]
    fn test_metavar_literal_rejects_ident() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Literal,
        }]);
        let input = vec![tok(SyntaxKind::Ident, "foo")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn test_expr_metavar_greedy() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Expr,
        }]);
        let input = vec![
            tok(SyntaxKind::IntLit, "1"),
            tok(SyntaxKind::Plus, "+"),
            tok(SyntaxKind::IntLit, "2"),
        ];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
        if let MatchResult::FullMatch(bindings) = result {
            let captured = &bindings[&SmolStr::from("x")];
            assert_eq!(captured.len(), 3, "Expected 3 tokens captured for expr");
        }
    }
}
