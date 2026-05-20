//! Tests for `for` loop desugaring into `IntoIterator`.

use glyim_test::{AnalysisTester, assert_no_errors};

#[test]
fn test_for_loop_over_array() {
    let source = r#"
        fn main() {
            let arr = [1, 2, 3];
            let mut sum = 0;
            for x in arr {
                sum += x;
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_for_loop_over_vec() {
    let source = r#"
        struct Vec<T> { data: *mut T, len: usize, cap: usize }
        impl<T> Vec<T> {
            fn push(&mut self, value: T) { }
        }
        fn main() {
            let mut v = Vec::<i32> { data: 0 as *mut i32, len: 0, cap: 0 };
            v.push(1);
            v.push(2);
            let mut sum = 0;
            for x in v {
                sum += x;
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_for_loop_with_into_iterator_trait() {
    let source = r#"
        trait IntoIterator {
            type Item;
            type IntoIter: Iterator<Item = Self::Item>;
            fn into_iter(self) -> Self::IntoIter;
        }
        trait Iterator {
            type Item;
            fn next(&mut self) -> Option<Self::Item>;
        }
        struct Range { start: i32, end: i32 }
        impl IntoIterator for Range {
            type Item = i32;
            type IntoIter = RangeIter;
            fn into_iter(self) -> RangeIter {
                RangeIter { current: self.start, end: self.end }
            }
        }
        struct RangeIter { current: i32, end: i32 }
        impl Iterator for RangeIter {
            type Item = i32;
            fn next(&mut self) -> Option<i32> {
                if self.current < self.end {
                    let val = self.current;
                    self.current += 1;
                    Some(val)
                } else {
                    None
                }
            }
        }
        fn main() {
            let mut sum = 0;
            for i in Range { start: 1, end: 5 } {
                sum += i;
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}
