fn main() {
    tauri_build::build();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo should set CARGO_MANIFEST_DIR"),
        )
        .join("tests/windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
