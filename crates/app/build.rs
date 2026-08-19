fn main() {
    #[cfg(feature = "desktop")]
    build_desktop();
}

#[cfg(feature = "desktop")]
fn build_desktop() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "begin_device_authorization",
            "device_authorization_status",
            "cancel_device_authorization",
            "take_route_event",
            "persist_route_state",
            "get_theme_preference",
            "set_theme_preference",
            "choose_repository_directory",
            "open_external_url",
        ]),
    ))
    .expect("Tauri build configuration should be valid");

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
