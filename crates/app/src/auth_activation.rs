//! Activates a newly authorized provider and persists its credential.

use std::sync::Arc;

use stellr_core::Provider;
use stellr_github::{credentials::CredentialStore, device_flow::AccessToken};
use tokio::sync::Notify;

use crate::runtime::ProviderSlot;

/// Installs the provider for the current process before attempting durable storage.
///
/// A returned warning means synchronization is active, but the credential must be
/// acquired again on the next launch.
pub async fn activate_provider_and_store(
    provider_slot: &ProviderSlot,
    provider: Arc<dyn Provider + Send + Sync>,
    refresh: Arc<Notify>,
    credential_store: Arc<dyn CredentialStore>,
    credential: AccessToken,
) -> Option<String> {
    provider_slot.replace(provider).await;
    refresh.notify_one();

    match tokio::task::spawn_blocking(move || credential_store.store(credential.expose())).await {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(format!(
            "GitHub is connected for this run, but the credential could not be saved: {error}"
        )),
        Err(error) => Some(format!(
            "GitHub is connected for this run, but credential storage did not finish: {error}"
        )),
    }
}
