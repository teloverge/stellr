use thiserror::Error;

use crate::credentials::CredentialStoreError;
#[cfg(feature = "os-credentials")]
use crate::credentials::{CredentialStore, OsCredentialStore};

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("no GitHub token: {0}. Run `gh auth login` or set GITHUB_TOKEN.")]
    NotFound(String),
}

pub fn resolve_token() -> Result<String, AuthError> {
    let env = std::env::var("GITHUB_TOKEN").ok();
    #[cfg(feature = "os-credentials")]
    let stored_token = || OsCredentialStore::default().load();
    #[cfg(not(feature = "os-credentials"))]
    let stored_token = || Ok(None);
    resolve_with(
        env,
        || {
            std::process::Command::new("gh")
                .args(["auth", "token"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        },
        stored_token,
        cfg!(feature = "os-credentials"),
    )
}

fn resolve_with(
    env: Option<String>,
    gh_token: impl FnOnce() -> Option<String>,
    stored_token: impl FnOnce() -> Result<Option<String>, CredentialStoreError>,
    credential_store_enabled: bool,
) -> Result<String, AuthError> {
    if let Some(token) = env.filter(|token| !token.trim().is_empty()) {
        return Ok(token);
    }
    let gh_output = gh_token();
    if let Some(token) = gh_output
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
    {
        return Ok(token);
    }
    if !credential_store_enabled {
        return Err(AuthError::NotFound(
            "neither GITHUB_TOKEN nor `gh auth token` yielded one".into(),
        ));
    }
    match stored_token() {
        Ok(Some(token)) if !token.trim().is_empty() => return Ok(token),
        Ok(_) => {}
        Err(error) => {
            return Err(AuthError::NotFound(format!(
                "neither GITHUB_TOKEN nor `gh auth token` yielded one, and {error}"
            )));
        }
    }
    Err(AuthError::NotFound(
        "neither GITHUB_TOKEN, `gh auth token`, nor the operating-system credential store yielded one"
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn env_var_wins() {
        assert_eq!(
            resolve_with(
                Some("tok_env".into()),
                || Some("tok_gh".into()),
                || Ok(Some("tok_store".into())),
                true,
            )
            .unwrap(),
            "tok_env"
        );
    }

    #[test]
    fn nonblank_env_does_not_invoke_gh_provider() {
        let gh_calls = Cell::new(0);

        let token = resolve_with(
            Some("tok_env".into()),
            || {
                gh_calls.set(gh_calls.get() + 1);
                Some("tok_gh".into())
            },
            || panic!("credential store must not be read"),
            true,
        )
        .unwrap();

        assert_eq!(token, "tok_env");
        assert_eq!(gh_calls.get(), 0);
    }

    #[test]
    fn falls_back_to_gh_and_trims() {
        assert_eq!(
            resolve_with(
                None,
                || Some("tok_gh\n".into()),
                || Ok(Some("tok_store".into())),
                true,
            )
            .unwrap(),
            "tok_gh"
        );
    }

    #[test]
    fn falls_back_to_the_operating_system_store_after_github_cli() {
        assert_eq!(
            resolve_with(None, || None, || Ok(Some("tok_store".into())), true).unwrap(),
            "tok_store"
        );
    }

    #[test]
    fn empty_everything_is_a_helpful_error() {
        let err = resolve_with(Some("".into()), || None, || Ok(None), true).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("gh auth login"));
        assert!(message.contains("GITHUB_TOKEN"));
        assert!(message.contains("operating-system credential store"));
    }

    #[test]
    fn headless_auth_does_not_read_or_advertise_an_os_store() {
        let err = resolve_with(
            None,
            || None,
            || panic!("credential store must not be read"),
            false,
        )
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("GITHUB_TOKEN"));
        assert!(message.contains("gh auth login"));
        assert!(!message.contains("operating-system credential store"));
    }
}
