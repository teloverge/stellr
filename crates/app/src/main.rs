fn main() {
    if let Err(error) = stellr_app::entrypoints::run_cli() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
