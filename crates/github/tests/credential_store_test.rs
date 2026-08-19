#![cfg(feature = "os-credentials")]

use stellr_github::credentials::{DEFAULT_ACCOUNT, OsCredentialStore, SERVICE_NAME};

#[test]
fn operating_system_store_uses_the_approved_stellr_identity() {
    let store = OsCredentialStore::default();

    assert_eq!(store.service(), SERVICE_NAME);
    assert_eq!(store.account(), DEFAULT_ACCOUNT);
    assert_eq!(store.service(), "stellr.github");
    assert_eq!(store.account(), "default");
}
