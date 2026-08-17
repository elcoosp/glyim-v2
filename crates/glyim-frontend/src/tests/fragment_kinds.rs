//! Regression tests for de-stubbing plan §5.1: the four fragment specifiers
//! `:lifetime`, `:literal`, `:vis`, `:tt` that previously fell through to
//! `_ => None` and were always rejected.
//!
//! Per the plan, each fragment kind is exercised at least once, including the
//! `vis`-matches-nothing case and the `tt`-matches-one-delimited-group case.

use crate::try_parse_fragment;

#[test]
fn lifetime_fragment_accepts_lifetime() {
    assert!(
        try_parse_fragment("lifetime", "'a").is_some(),
        "`'a` should be a valid :lifetime fragment"
    );
    assert!(
        try_parse_fragment("lifetime", "'static").is_some(),
        "`'static` should be a valid :lifetime fragment"
    );
}

#[test]
fn lifetime_fragment_rejects_non_lifetime() {
    assert!(
        try_parse_fragment("lifetime", "a").is_none(),
        "bare identifier `a` is not a :lifetime fragment"
    );
    assert!(
        try_parse_fragment("lifetime", "'a 'b").is_none(),
        "two lifetimes are not a single :lifetime fragment"
    );
}

#[test]
fn literal_fragment_accepts_literals() {
    assert!(
        try_parse_fragment("literal", "42").is_some(),
        "`42` should be a valid :literal fragment"
    );
    assert!(
        try_parse_fragment("literal", "\"hello\"").is_some(),
        "`\"hello\"` should be a valid :literal fragment"
    );
    // Unary-minus literals must be accepted (plan §5.1).
    assert!(
        try_parse_fragment("literal", "-1").is_some(),
        "`-1` (unary-minus literal) should be a valid :literal fragment"
    );
}

#[test]
fn literal_fragment_rejects_non_literals() {
    assert!(
        try_parse_fragment("literal", "x").is_none(),
        "identifier `x` is not a :literal fragment"
    );
    assert!(
        try_parse_fragment("literal", "1 2").is_none(),
        "two literals are not a single :literal fragment"
    );
}

#[test]
fn vis_fragment_matches_nothing() {
    // `vis` must successfully match zero tokens (private-by-default).
    assert!(
        try_parse_fragment("vis", "").is_some(),
        ":vis fragment must match an empty token stream"
    );
}

#[test]
fn vis_fragment_matches_pub_forms() {
    assert!(
        try_parse_fragment("vis", "pub").is_some(),
        "`pub` should be a valid :vis fragment"
    );
    assert!(
        try_parse_fragment("vis", "pub(crate)").is_some(),
        "`pub(crate)` should be a valid :vis fragment"
    );
    assert!(
        try_parse_fragment("vis", "pub(super)").is_some(),
        "`pub(super)` should be a valid :vis fragment"
    );
    assert!(
        try_parse_fragment("vis", "pub(self)").is_some(),
        "`pub(self)` should be a valid :vis fragment"
    );
    assert!(
        try_parse_fragment("vis", "pub(in path::to::mod)").is_some(),
        "`pub(in path)` should be a valid :vis fragment"
    );
}

#[test]
fn vis_fragment_rejects_garbage() {
    assert!(
        try_parse_fragment("vis", "pub(crate").is_none(),
        "unbalanced `pub(crate` should be rejected"
    );
    assert!(
        try_parse_fragment("vis", "private").is_none(),
        "`private` is not a valid :vis fragment"
    );
}

#[test]
fn tt_fragment_matches_single_token() {
    assert!(
        try_parse_fragment("tt", "x").is_some(),
        "a single leaf token is a valid :tt fragment"
    );
    assert!(
        try_parse_fragment("tt", "42").is_some(),
        "a single literal token is a valid :tt fragment"
    );
}

#[test]
fn tt_fragment_matches_one_delimited_group() {
    assert!(
        try_parse_fragment("tt", "(a, b, c)").is_some(),
        "one balanced `(...)` group is a valid :tt fragment"
    );
    assert!(
        try_parse_fragment("tt", "[a, b]").is_some(),
        "one balanced `[...]` group is a valid :tt fragment"
    );
    assert!(
        try_parse_fragment("tt", "{ a; b; }").is_some(),
        "one balanced `{{...}}` group is a valid :tt fragment"
    );
}

#[test]
fn tt_fragment_rejects_unbalanced_or_multiple() {
    assert!(
        try_parse_fragment("tt", "(a, b").is_none(),
        "unbalanced `(a, b` should be rejected"
    );
    assert!(
        try_parse_fragment("tt", "a b").is_none(),
        "two token trees are not a single :tt fragment"
    );
    assert!(
        try_parse_fragment("tt", "").is_none(),
        "empty stream is not a :tt fragment"
    );
}
