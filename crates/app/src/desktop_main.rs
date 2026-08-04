#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() {
    if let Err(error) = stellr_app::entrypoints::run_desktop() {
        #[cfg(debug_assertions)]
        eprintln!("{error}");
        std::process::exit(1);
    }
}
