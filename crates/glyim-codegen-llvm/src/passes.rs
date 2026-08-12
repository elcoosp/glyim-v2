use inkwell::module::Module;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;

/// Run LLVM optimization passes based on the given optimization level and size hint.
///
/// Optimization levels:
/// - 0: No optimizations (O0)
/// - 1: Basic optimizations (O1)
/// - 2: Standard optimizations (O2)
/// - 3: Aggressive optimizations (O3)
/// - 4+: Same as O3 (default)
///
/// When `opt_for_size` is true, size optimizations (Os/Oz) are used instead of speed.
pub(crate) fn run_llvm_passes<'ctx>(
    module: &Module<'ctx>,
    target_machine: &TargetMachine,
    opt_level: u8,
    opt_for_size: bool,
) -> Result<(), String> {
    let pass_str = match (opt_level, opt_for_size) {
        // Level 0: no optimization
        (0, _) => "default<O0>",
        // Level 1: light optimization
        (1, false) => "default<O1>",
        (1, true) => "default<Os>",
        // Level 2: standard optimization
        (2, false) => "default<O2>",
        (2, true) => "default<Oz>",
        // Level 3: aggressive optimization
        (3, false) => "default<O3>",
        (3, true) => "default<Oz>",
        // Level 4+: still O3 (or Oz if size-optimizing)
        (_, false) => "default<O3>",
        (_, true) => "default<Oz>",
    };

    let opts = PassBuilderOptions::create();
    module
        .run_passes(pass_str, target_machine, opts)
        .map_err(|e| format!("Failed to run LLVM passes: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::OptimizationLevel;
    use inkwell::context::Context;
    use inkwell::targets::{Target, TargetMachine};
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_targets() {
        INIT.call_once(|| {
            Target::initialize_all(&inkwell::targets::InitializationConfig::default());
        });
    }
    fn create_test_module(ctx: &Context) -> (Module<'_>, TargetMachine) {
        init_targets();
        let module = ctx.create_module("test");
        let triple = inkwell::targets::TargetTriple::create("x86_64-unknown-linux-gnu");
        module.set_triple(&triple);
        let target = Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();
        (module, target_machine)
    }

    #[test]
    fn test_opt_level_0() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_level_1() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 1, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_level_2() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 2, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_level_3() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 3, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_level_4_defaults_to_o3() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 4, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_for_size_with_level_1() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 1, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_for_size_with_level_2() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 2, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_for_size_with_level_3() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 3, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_opt_for_size_with_high_level() {
        let ctx = Context::create();
        let (module, tm) = create_test_module(&ctx);
        let result = run_llvm_passes(&module, &tm, 5, true);
        assert!(result.is_ok());
    }
}
