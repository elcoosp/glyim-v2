use super::token_tree::TokenTree;
use glyim_syntax::SyntaxKind;
use smol_str::SmolStr;
use std::collections::HashMap;

pub(crate) fn substitute(
    template: &[TokenTree],
    bindings: &HashMap<SmolStr, Vec<TokenTree>>,
) -> Vec<TokenTree> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < template.len() {
        let tree = &template[i];
        match tree {
            TokenTree::Token(SyntaxKind::Dollar, _) => {
                i += 1;
                if i >= template.len() {
                    result.push(TokenTree::Token(SyntaxKind::Dollar, SmolStr::from("$")));
                    break;
                }
                match &template[i] {
                    TokenTree::Token(SyntaxKind::Ident, name) => {
                        let text = name.as_str();
                        if text == "crate" {
                            result.push(TokenTree::DollarCrate);
                        } else if let Some(captured) = bindings.get(name) {
                            result.extend(captured.clone());
                        } else {
                            result.push(TokenTree::Token(SyntaxKind::Dollar, SmolStr::from("$")));
                            result.push(TokenTree::Token(SyntaxKind::Ident, name.clone()));
                        }
                        i += 1;
                    }
                    TokenTree::Group(SyntaxKind::LParen, inner, SyntaxKind::RParen) => {
                        i += 1;
                        let separator = if i < template.len()
                            && !matches!(
                                &template[i],
                                TokenTree::Token(SyntaxKind::Star | SyntaxKind::Plus | SyntaxKind::Question, _)
                            )
                        {
                            let sep = template[i].clone();
                            i += 1;
                            Some(sep)
                        } else {
                            None
                        };
                        if i >= template.len() {
                            break;
                        }
                        let rep_kind = match &template[i] {
                            TokenTree::Token(SyntaxKind::Star, _) => RepKind::ZeroOrMore,
                            TokenTree::Token(SyntaxKind::Plus, _) => RepKind::OneOrMore,
                            TokenTree::Token(SyntaxKind::Question, _) => RepKind::ZeroOrOne,
                            _ => break,
                        };
                        i += 1;

                        let var_name = find_first_metavar(inner);
                        let repetitions: usize = if let Some(ref name) = var_name {
                            bindings.get(name).map(|v: &Vec<TokenTree>| v.len()).unwrap_or(0)
                        } else {
                            0
                        };

                        match rep_kind {
                            RepKind::ZeroOrMore | RepKind::OneOrMore => {
                                for rep_idx in 0..repetitions {
                                    if rep_idx > 0 {
                                        if let Some(ref sep) = separator {
                                            result.push(sep.clone());
                                        }
                                    }
                                    let rep_bindings = extract_repetition_bindings(bindings, inner, rep_idx);
                                    let subbed = substitute(inner, &rep_bindings);
                                    result.extend(subbed);
                                }
                            }
                            RepKind::ZeroOrOne => {
                                if repetitions > 0 {
                                    let rep_bindings = extract_repetition_bindings(bindings, inner, 0);
                                    let subbed = substitute(inner, &rep_bindings);
                                    result.extend(subbed);
                                }
                            }
                        }
                    }
                    _ => {
                        result.push(TokenTree::Token(SyntaxKind::Dollar, SmolStr::from("$")));
                        result.push(template[i].clone());
                        i += 1;
                    }
                }
            }
            TokenTree::Group(open, inner, close) => {
                let subbed_inner = substitute(inner, bindings);
                result.push(TokenTree::Group(*open, subbed_inner, *close));
                i += 1;
            }
            other => {
                result.push(other.clone());
                i += 1;
            }
        }
    }
    result
}

#[derive(Clone, Copy, Debug)]
enum RepKind {
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

fn find_first_metavar(trees: &[TokenTree]) -> Option<SmolStr> {
    let mut i = 0;
    while i < trees.len() {
        if let TokenTree::Token(SyntaxKind::Dollar, _) = &trees[i] {
            if i + 1 < trees.len() {
                if let TokenTree::Token(SyntaxKind::Ident, name) = &trees[i + 1] {
                    return Some(name.clone());
                }
            }
        }
        i += 1;
    }
    None
}

fn extract_repetition_bindings(
    bindings: &HashMap<SmolStr, Vec<TokenTree>>,
    _inner: &[TokenTree],
    index: usize,
) -> HashMap<SmolStr, Vec<TokenTree>> {
    let mut result = HashMap::new();
    for (name, tokens) in bindings {
        if index < tokens.len() {
            result.insert(name.clone(), vec![tokens[index].clone()]);
        }
    }
    result
}
