#![cfg(feature = "desktop")]

use stellr_app::desktop::create_main_window;

#[test]
fn native_main_window_uses_the_authenticated_loopback_url() {
    let app = tauri::test::mock_app();
    let expected_url = "http://127.0.0.1:49152/?token=session-token";

    let window = create_main_window(&app, expected_url.parse().unwrap()).unwrap();

    assert_eq!(window.label(), "main");
    assert_eq!(window.url().unwrap().as_str(), expected_url);
}
