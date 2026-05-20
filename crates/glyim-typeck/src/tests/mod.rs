// Glyim Type Checker - Unit Tests
//
// This file declares all test modules for the glyim-typeck crate.
// Each module corresponds to a .rs file in the tests/ directory.
// Do not remove modules without verifying they are no longer needed.

mod assign;
mod binary_error;
mod binary_i32;
mod break_continue;
mod cast;
mod coherence;
mod default_methods;
mod edge_cases;
mod empty_crate;
mod fn_sig_inst;
mod fn_unit;
mod function_call;
mod harness_tests;
mod inference;
mod match_expr;
mod method_call;
mod multi_seg_path;
mod obligation;
mod pattern_matching;
mod projection_typeck;
mod ref_mut;
mod ref_x;
mod return_stmt;
mod struct_field;
mod thir_body;
mod tuple_index;
mod typeck_result_accessors;
mod where_clause;
mod while_loop;

// Helper modules
mod common;
mod test_utils;
