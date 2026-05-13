fn main() {
    if let Err(diags) = glyim_cli::run() {
        for diag in diags {
            eprintln!("{}", diag);
        }
        std::process::exit(1);
    }
}
