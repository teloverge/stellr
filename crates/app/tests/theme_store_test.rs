use stellr_app::theme::{ThemePreference, ThemeStore};

#[test]
fn first_launch_defaults_to_system_and_a_choice_survives_relaunch() {
    let profile = tempfile::tempdir().unwrap();
    let file = profile.path().join("theme.json");

    assert_eq!(
        ThemeStore::new(file.clone()).load(),
        ThemePreference::System
    );
    ThemeStore::new(file.clone())
        .save(ThemePreference::Dark)
        .unwrap();
    assert_eq!(ThemeStore::new(file).load(), ThemePreference::Dark);
}

#[test]
fn corrupt_preferences_fall_back_to_system() {
    let profile = tempfile::tempdir().unwrap();
    let file = profile.path().join("theme.json");
    std::fs::write(&file, br#"{"preference":"ultraviolet"}"#).unwrap();

    assert_eq!(ThemeStore::new(file).load(), ThemePreference::System);
}
