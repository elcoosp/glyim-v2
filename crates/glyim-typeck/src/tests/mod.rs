// Glyim Type Checker - Unit Tests
//
// This file declares all test modules for the glyim-typeck crate.
// Harness tests are currently disabled because they require I/O and
// the test harness implementation is not fully compatible with the
// current workspace setup. They will be re-enabled in a future update.
// TODO: Re-enable harness_tests after fixing the test runner paths.

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
