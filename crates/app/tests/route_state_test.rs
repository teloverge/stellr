use std::process::Command;
use stellr_app::route_state::{PersistedRoute, RouteStateStore};

#[test]
fn clean_exit_and_relaunch_restore_the_selected_space_and_issue() {
    if let (Ok(mode), Ok(file)) = (
        std::env::var("STELLR_ROUTE_STATE_CHILD"),
        std::env::var("STELLR_ROUTE_STATE_FILE"),
    ) {
        let store = RouteStateStore::new(file.into());
        if mode == "save" {
            store
                .save(Some(
                    PersistedRoute::new("teloverge-stellr", Some(64)).unwrap(),
                ))
                .unwrap();
        } else {
            assert_eq!(
                store.load(),
                Some(PersistedRoute::new("teloverge-stellr", Some(64)).unwrap())
            );
        }
        return;
    }

    let profile = tempfile::tempdir().unwrap();
    let file = profile.path().join("route.json");
    for mode in ["save", "restore"] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "clean_exit_and_relaunch_restore_the_selected_space_and_issue",
                "--nocapture",
            ])
            .env("STELLR_ROUTE_STATE_CHILD", mode)
            .env("STELLR_ROUTE_STATE_FILE", &file)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode} process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn corrupt_or_invalid_state_is_rejected_instead_of_becoming_a_route() {
    let profile = tempfile::tempdir().unwrap();
    let file = profile.path().join("route.json");
    std::fs::write(&file, r#"{"space":"","issue":0}"#).unwrap();

    assert_eq!(RouteStateStore::new(file).load(), None);
}
