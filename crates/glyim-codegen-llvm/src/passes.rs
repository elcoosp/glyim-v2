use inkwell::module::Module;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;

pub(crate) fn run_llvm_passes<'ctx>(
    module: &Module<'ctx>,
    target_machine: &TargetMachine,
    opt_level: u8,
    opt_for_size: bool,
) -> Result<(), String> {
    let pass_str = match (opt_level, opt_for_size) {
        (0, _) => "default<O0>",
        (1, false) => "default<O1>",
        (1, true) => "default<Os>",
        (2, false) => "default<O2>",
        (2, true) => "default<Oz>",
        (3, false) => "default<O3>",
        (3, true) => "default<Oz>",
        _ => "default<O2>",
    };
    eprintln!("DEBUG: run_llvm_passes: opt_level={}, opt_for_size={}, pass_str={}", opt_level, opt_for_size, pass_str);
    let opts = PassBuilderOptions::create();
    module
        .run_passes(pass_str, target_machine, opts)
        .map_err(|e| format!("Failed to run LLVM passes: {}", e))
}
