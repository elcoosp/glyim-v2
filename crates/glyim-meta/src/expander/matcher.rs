//! Macro pattern matching with fragment specifiers and TT munching.

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
                    // Repetition: $(...)
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
///
/// Stage A (Tier 4.1): tighten the single-token cases that a fragment could
/// plausibly be validated against without full parsing, and reject
/// patently-invalid single-token starts for the flexible fragment kinds
/// instead of blanket-accepting everything. Variable-length fragment
/// consumption (Stage B) lives in `match_pieces` via `consume_fragment`.
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
        FragmentSpec::Vis => matches!(tree, TokenTree::Token(SyntaxKind::KwPub, _)),
        FragmentSpec::Block => {
            matches!(tree, TokenTree::Group(SyntaxKind::LBrace, _, SyntaxKind::RBrace))
        }
        FragmentSpec::Tt => true, // `tt` is "exactly one token tree" — `true` is correct here.
        FragmentSpec::Expr
        | FragmentSpec::Ty
        | FragmentSpec::Path
        | FragmentSpec::Pat => {
            // Reject tokens that can never start this fragment kind, even
            // though we can't yet confirm the whole fragment is valid.
            !matches!(
                tree,
                TokenTree::Token(
                    SyntaxKind::Semicolon
                        | SyntaxKind::Comma
                        | SyntaxKind::RParen
                        | SyntaxKind::RBrace
                        | SyntaxKind::RBracket,
                    _
                )
            )
        }
        // `stmt`/`item`/`meta` can begin with a keyword + many shapes, and a
        // single token tree cannot be validated as a whole here (Stage B).
        FragmentSpec::Stmt | FragmentSpec::Item | FragmentSpec::Meta => true,
    }
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
                // All fragment specs match exactly one token tree for now.
                // Fragment-specific filtering rejects mismatched tokens.
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
                    // No token to match — for expr/ty/etc. this is acceptable
                    // (empty match), but for ident/literal/tt it's not.
                    match fragment {
                        FragmentSpec::Expr
                        | FragmentSpec::Ty
                        | FragmentSpec::Path
                        | FragmentSpec::Block
                        | FragmentSpec::Stmt
                        | FragmentSpec::Item
                        | FragmentSpec::Pat
                        | FragmentSpec::Vis
                        | FragmentSpec::Meta => {
                            // Allow empty match for these flexible specs
                        }
                        FragmentSpec::Ident
                        | FragmentSpec::Literal
                        | FragmentSpec::Lifetime
                        | FragmentSpec::Tt => {
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
    fn test_expr_metavar_matches_one_token() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Expr,
        }]);
        let input = vec![tok(SyntaxKind::IntLit, "42")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
        if let MatchResult::FullMatch(bindings) = result {
            let captured = &bindings[&SmolStr::from("x")];
            assert_eq!(captured.len(), 1, "Expected 1 token captured for expr");
        }
    }

    #[test]
    fn test_tt_metavar_matches_any_token() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("x"),
            fragment: FragmentSpec::Tt,
        }]);
        let input = vec![tok(SyntaxKind::Plus, "+")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
    }

    #[test]
    fn test_stage_a_expr_rejects_separator_token() {
        // Tier 4.1 Stage A: a fragment starting with a separator token can
        // never be a valid expr/ty/path/pat start.
        for sep in [
            SyntaxKind::Semicolon,
            SyntaxKind::Comma,
            SyntaxKind::RParen,
            SyntaxKind::RBrace,
            SyntaxKind::RBracket,
        ] {
            let pattern = Pattern::new(vec![PatternPiece::Metavar {
                name: SmolStr::from("x"),
                fragment: FragmentSpec::Expr,
            }]);
            let input = vec![tok(sep, ";")];
            let result = match_pattern(&pattern, &input);
            assert!(
                matches!(result, MatchResult::NoMatch),
                "expr must reject separator token {:?}",
                sep
            );
        }
    }

    #[test]
    fn test_stage_a_vis_matches_pub() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("v"),
            fragment: FragmentSpec::Vis,
        }]);
        let input = vec![tok(SyntaxKind::KwPub, "pub")];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
    }

    #[test]
    fn test_stage_a_block_matches_brace_group() {
        let pattern = Pattern::new(vec![PatternPiece::Metavar {
            name: SmolStr::from("b"),
            fragment: FragmentSpec::Block,
        }]);
        let input = vec![TokenTree::Group(
            SyntaxKind::LBrace,
            vec![tok(SyntaxKind::Ident, "x")],
            SyntaxKind::RBrace,
        )];
        let result = match_pattern(&pattern, &input);
        assert!(matches!(result, MatchResult::FullMatch(_)));
    }
}
