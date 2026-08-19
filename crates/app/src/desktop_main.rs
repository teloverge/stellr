#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() {
    let result = stellr_app::entrypoints::run_desktop();

    if let Err(_error) = result {
        #[cfg(debug_assertions)]
        eprintln!("{_error}");
        std::process::exit(1);
    }
}
