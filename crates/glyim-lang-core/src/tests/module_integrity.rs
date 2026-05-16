//! Tests for the structural integrity of core library .g files.

use glyim_lang_core::core_source;

/// Assert that a module defines the expected key items.
#[test]
fn option_defines_unwrap() {
    let src = core_source("option").unwrap();
    assert!(src.contains("unwrap"), "option.g should define unwrap");
    assert!(src.contains("is_some"), "option.g should define is_some");
    assert!(src.contains("is_none"), "option.g should define is_none");
    assert!(src.contains("expect"), "option.g should define expect");
    assert!(src.contains("unwrap_or"), "option.g should define unwrap_or");
    assert!(src.contains("map"), "option.g should define map");
    assert!(src.contains("take"), "option.g should define take");
    assert!(src.contains("replace"), "option.g should define replace");
}

#[test]
fn result_defines_ok_err() {
    let src = core_source("result").unwrap();
    assert!(src.contains("is_ok"), "result.g should define is_ok");
    assert!(src.contains("is_err"), "result.g should define is_err");
    assert!(src.contains("unwrap"), "result.g should define unwrap");
    assert!(src.contains("expect"), "result.g should define expect");
    assert!(src.contains("map"), "result.g should define map");
    assert!(src.contains("map_err"), "result.g should define map_err");
}

#[test]
fn iter_defines_traits() {
    let src = core_source("iter").unwrap();
    assert!(src.contains("trait Iterator"), "iter.g should define Iterator trait");
    assert!(src.contains("trait IntoIterator"), "iter.g should define IntoIterator trait");
    assert!(src.contains("trait FromIterator"), "iter.g should define FromIterator trait");
    assert!(src.contains("fn next"), "iter.g should define next method");
    assert!(src.contains("fn collect"), "iter.g should define collect method");
    assert!(src.contains("fn map"), "iter.g should define map method");
    assert!(src.contains("fn filter"), "iter.g should define filter method");
    assert!(src.contains("fn fold"), "iter.g should define fold method");
}

#[test]
fn slice_defines_methods() {
    let src = core_source("slice").unwrap();
    assert!(src.contains("fn len"), "slice.g should define len");
    assert!(src.contains("fn is_empty"), "slice.g should define is_empty");
    assert!(src.contains("fn first"), "slice.g should define first");
    assert!(src.contains("fn swap"), "slice.g should define swap");
    assert!(src.contains("fn reverse"), "slice.g should define reverse");
    assert!(src.contains("fn sort"), "slice.g should define sort");
    assert!(src.contains("fn contains"), "slice.g should define contains");
}

#[test]
fn str_defines_methods() {
    let src = core_source("str").unwrap();
    assert!(src.contains("fn len"), "str.g should define len");
    assert!(src.contains("fn is_empty"), "str.g should define is_empty");
    assert!(src.contains("fn contains"), "str.g should define contains");
    assert!(src.contains("fn chars"), "str.g should define chars");
    assert!(src.contains("fn trim"), "str.g should define trim");
    assert!(src.contains("fn parse"), "str.g should define parse");
    assert!(src.contains("trait FromStr"), "str.g should define FromStr trait");
}

#[test]
fn cell_defines_types() {
    let src = core_source("cell").unwrap();
    assert!(src.contains("UnsafeCell"), "cell.g should define UnsafeCell");
    assert!(src.contains("Cell"), "cell.g should define Cell");
    assert!(src.contains("RefCell"), "cell.g should define RefCell");
    assert!(src.contains("Ref"), "cell.g should define Ref");
    assert!(src.contains("RefMut"), "cell.g should define RefMut");
}

#[test]
fn mem_defines_functions() {
    let src = core_source("mem").unwrap();
    assert!(src.contains("fn replace"), "mem.g should define replace");
    assert!(src.contains("fn swap"), "mem.g should define swap");
    assert!(src.contains("fn size_of"), "mem.g should define size_of");
    assert!(src.contains("fn align_of"), "mem.g should define align_of");
    assert!(src.contains("fn take"), "mem.g should define take");
    assert!(src.contains("ManuallyDrop"), "mem.g should define ManuallyDrop");
    assert!(src.contains("MaybeUninit"), "mem.g should define MaybeUninit");
}

#[test]
fn ptr_defines_functions() {
    let src = core_source("ptr").unwrap();
    assert!(src.contains("NonNull"), "ptr.g should define NonNull");
    assert!(src.contains("fn null"), "ptr.g should define null");
    assert!(src.contains("fn null_mut"), "ptr.g should define null_mut");
    assert!(src.contains("fn read"), "ptr.g should define read");
    assert!(src.contains("fn write"), "ptr.g should define write");
    assert!(src.contains("fn copy"), "ptr.g should define copy");
}

#[test]
fn ops_defines_traits() {
    let src = core_source("ops").unwrap();
    assert!(src.contains("trait Deref"), "ops.g should define Deref");
    assert!(src.contains("trait DerefMut"), "ops.g should define DerefMut");
    assert!(src.contains("trait Drop"), "ops.g should define Drop");
    assert!(src.contains("trait Fn"), "ops.g should define Fn");
    assert!(src.contains("trait Add"), "ops.g should define Add");
    assert!(src.contains("trait Sub"), "ops.g should define Sub");
    assert!(src.contains("trait Mul"), "ops.g should define Mul");
    assert!(src.contains("trait Index"), "ops.g should define Index");
    assert!(src.contains("Range"), "ops.g should define Range");
}

#[test]
fn cmp_defines_traits_and_functions() {
    let src = core_source("cmp").unwrap();
    assert!(src.contains("trait PartialEq"), "cmp.g should define PartialEq");
    assert!(src.contains("trait Eq"), "cmp.g should define Eq");
    assert!(src.contains("trait PartialOrd"), "cmp.g should define PartialOrd");
    assert!(src.contains("trait Ord"), "cmp.g should define Ord");
    assert!(src.contains("fn min"), "cmp.g should define min");
    assert!(src.contains("fn max"), "cmp.g should define max");
    assert!(src.contains("Ordering"), "cmp.g should define Ordering");
}

#[test]
fn marker_defines_traits() {
    let src = core_source("marker").unwrap();
    assert!(src.contains("trait Sized"), "marker.g should define Sized");
    assert!(src.contains("trait Copy"), "marker.g should define Copy");
    assert!(src.contains("trait Send"), "marker.g should define Send");
    assert!(src.contains("trait Sync"), "marker.g should define Sync");
    assert!(src.contains("PhantomData"), "marker.g should define PhantomData");
}

#[test]
fn panic_defines_macros() {
    let src = core_source("panic").unwrap();
    assert!(src.contains("macro panic!"), "panic.g should define panic! macro");
    assert!(src.contains("macro assert!"), "panic.g should define assert! macro");
    assert!(src.contains("macro assert_eq!"), "panic.g should define assert_eq! macro");
    assert!(src.contains("macro assert_ne!"), "panic.g should define assert_ne! macro");
}

#[test]
fn hint_defines_functions() {
    let src = core_source("hint").unwrap();
    assert!(src.contains("fn black_box"), "hint.g should define black_box");
    assert!(src.contains("fn spin_loop"), "hint.g should define spin_loop");
}

#[test]
fn convert_defines_traits() {
    let src = core_source("convert").unwrap();
    assert!(src.contains("trait Into"), "convert.g should define Into");
    assert!(src.contains("trait From"), "convert.g should define From");
    assert!(src.contains("trait TryFrom"), "convert.g should define TryFrom");
    assert!(src.contains("trait TryInto"), "convert.g should define TryInto");
    assert!(src.contains("trait AsRef"), "convert.g should define AsRef");
    assert!(src.contains("Infallible"), "convert.g should define Infallible");
}

#[test]
fn default_defines_trait_and_impls() {
    let src = core_source("default").unwrap();
    assert!(src.contains("trait Default"), "default.g should define Default trait");
    assert!(src.contains("impl Default for bool"), "default.g should impl Default for bool");
    assert!(src.contains("impl Default for u32"), "default.g should impl Default for u32");
    assert!(src.contains("impl Default for i32"), "default.g should impl Default for i32");
    assert!(src.contains("impl Default for f64"), "default.g should impl Default for f64");
    assert!(src.contains("impl Default for Option"), "default.g should impl Default for Option");
}
MODINTEG

echo "Writing compile-pass test .g files"

cat > crates/glyim-lang-core/tests/compile-pass/option_unwrap.g << 'OPTUNWRAPG'
// test-mode: compile-pass
// V16-T01: Option::unwrap works

fn main() {
    let val: Option<i32> = Option::Some(42);
    let unwrapped = val.unwrap();
    assert_eq!(unwrapped, 42);
}
OPTUNWRAPG

cat > crates/glyim-lang-core/tests/compile-pass/iter_slice.g << 'ITERSLICEG'
// test-mode: compile-pass
// V16-T02: Iterate over slice using for loop (requires IntoIterator)

fn main() {
    let data: [i32; 3] = [1, 2, 3];
    let mut sum = 0;
    for item in &data {
        sum += *item;
    }
    assert_eq!(sum, 6);
}
ITERSLICEG

cat > crates/glyim-lang-core/tests/compile-pass/panic_message.g << 'PANICMSG'
// test-mode: compile-pass
// V16-T03: panic! macro prints message and aborts

fn main() {
    panic!("this is a panic message");
}
PANICMSG

cat > crates/glyim-lang-core/tests/compile-pass/mem_replace_swap.g << 'MEMREPLACEG'
// test-mode: compile-pass
// V16-T04: mem::replace, mem::swap

fn main() {
    let mut a = 10;
    let mut b = 20;
    let old = mem::replace(&mut a, 99);
    assert_eq!(old, 10);
    assert_eq!(a, 99);
    mem::swap(&mut a, &mut b);
    assert_eq!(a, 20);
    assert_eq!(b, 99);
}
MEMREPLACEG

cat > crates/glyim-lang-core/tests/compile-pass/cmp_min_max.g << 'CMPMINMAXG'
// test-mode: compile-pass
// V16-T05: cmp::min, cmp::max

fn main() {
    let a = 5;
    let b = 10;
    assert_eq!(cmp::min(a, b), 5);
    assert_eq!(cmp::max(a, b), 10);
    assert_eq!(cmp::min(b, a), 5);
    assert_eq!(cmp::max(b, a), 10);
}
CMPMINMAXG

cat > crates/glyim-lang-core/tests/compile-pass/default_trait.g << 'DEFAULTTRAITG'
// test-mode: compile-pass
// V16-T06: Default trait

fn main() {
    let b: bool = Default::default();
    assert_eq!(b, false);
    let i: i32 = Default::default();
    assert_eq!(i, 0);
    let opt: Option<i32> = Default::default();
    assert!(opt.is_none());
}
DEFAULTTRAITG

echo "All test files written"
find crates/glyim-lang-core -type f | sort

echo "Checking compilation of glyim-lang-core"
if ! cargo check -p glyim-lang-core 2>&1; then
  echo "Compilation check failed for glyim-lang-core -- will investigate"
  COMPILE_OK=false
fi

echo "Checking workspace compilation"
if ! cargo check --workspace 2>&1; then
  echo "Workspace compilation check failed -- will investigate"
  COMPILE_OK=false
fi

if [ "$COMPILE_OK" = true ]; then
  echo "Running tests for glyim-lang-core"
  cargo test -p glyim-lang-core 2>&1 || echo "Some tests failed"
fi

echo "Done with workspace setup and initial tests"
