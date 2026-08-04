mod cli;

use std::{ffi::OsString, io::Write, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use stellr_app::runtime::{RuntimeOptions, SessionAuth, start};
use stellr_github::{auth::resolve_token, cache::Cache, sync::GithubProvider};
use stellr_server::spaces::SpaceStore;

use crate::cli::{Cli, Command, ServeArgs};

type DynError = Box<dyn std::error::Error + Send + Sync>;

fn effective_launch_dir(
    current: PathBuf,
    appimage: Option<OsString>,
    original_working_dir: Option<OsString>,
) -> PathBuf {
    if appimage.is_some()
        && let Some(original) = original_working_dir.map(PathBuf::from)
        && original.is_absolute()
    {
        return original;
    }

    current
}

fn launch_current_dir() -> std::io::Result<PathBuf> {
    Ok(effective_launch_dir(
        std::env::current_dir()?,
        std::env::var_os("APPIMAGE"),
        std::env::var_os("OWD"),
    ))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), DynError> {
    let cli = Cli::parse();
    match cli.command {
        #[cfg(debug_assertions)]
        Some(Command::Acceptance(args)) => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(stellr_app::acceptance::run(args.github_base, args.profile)),
        Some(Command::Serve(args)) => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(serve(args)),
        Some(Command::Open(args)) => {
            let cwd = launch_current_dir()?;
            stellr_app::desktop::run(stellr_app::desktop::DesktopLaunch {
                cwd,
                target: args.target,
                restore_route: false,
            })
            .map_err(Into::into)
        }
        None => {
            let cwd = launch_current_dir()?;
            let restore_route = cli.protocol_target.is_none();
            let target = cli
                .protocol_target
                .unwrap_or_else(|| cwd.to_string_lossy().into_owned());
            stellr_app::desktop::run(stellr_app::desktop::DesktopLaunch {
                target,
                cwd,
                restore_route,
            })
            .map_err(Into::into)
        }
    }
}

async fn serve(args: ServeArgs) -> Result<(), DynError> {
    let provider_token = resolve_token()?;
    let provider = Arc::new(GithubProvider::new(provider_token)?);
    let runtime = start(
        RuntimeOptions {
            address: args.addr,
            session_auth: if args.no_token {
                SessionAuth::Disabled
            } else {
                SessionAuth::Required
            },
            issue: args.issue,
            spaces_file: SpaceStore::default_file(),
            cache_root: Cache::default_root(),
            poll_interval: Duration::from_secs(30),
        },
        provider,
    )
    .await?;
    println!("stellr cockpit: {}", runtime.cockpit_url());
    std::io::stdout().flush()?;

    runtime.wait().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use clap::Parser;

    use crate::cli::{Cli, Command};

    #[test]
    fn bare_launch_selects_desktop_mode() {
        let parsed = Cli::try_parse_from(["stellr"]).unwrap();

        assert!(parsed.command.is_none());
    }

    #[test]
    fn serve_defaults_to_loopback_port_8787_with_session_auth() {
        let parsed = Cli::try_parse_from(["stellr", "serve"]).unwrap();
        let Some(Command::Serve(args)) = parsed.command else {
            panic!("serve should select the serve command")
        };

        assert_eq!(args.addr, "127.0.0.1:8787");
        assert!(!args.no_token);
        assert_eq!(args.issue, None);
    }

    #[test]
    fn serve_accepts_a_custom_address_and_open_server_flag() {
        let parsed =
            Cli::try_parse_from(["stellr", "serve", "--addr", "127.0.0.1:0", "--no-token"])
                .unwrap();
        let Some(Command::Serve(args)) = parsed.command else {
            panic!("serve should select the serve command")
        };

        assert_eq!(args.addr, "127.0.0.1:0");
        assert!(args.no_token);
    }

    #[test]
    fn serve_accepts_a_positive_conversation_issue() {
        let parsed =
            Cli::try_parse_from(["stellr", "serve", "--addr", "127.0.0.1:0", "--issue", "14"])
                .unwrap();
        let Some(Command::Serve(args)) = parsed.command else {
            panic!("serve should select the serve command")
        };

        assert_eq!(args.issue.map(NonZeroU64::get), Some(14));
        assert!(Cli::try_parse_from(["stellr", "serve", "--issue", "0"]).is_err());
    }

    #[test]
    fn open_accepts_one_path_url_slug_or_stellr_target() {
        for target in [
            r"D:\dev\stellr",
            "teloverge/stellr",
            "https://github.com/teloverge/stellr/issues/62",
            "stellr://space?repo=teloverge%2Fstellr&issue=62",
        ] {
            let parsed = Cli::try_parse_from(["stellr", "open", target]).unwrap();
            let Some(Command::Open(args)) = parsed.command else {
                panic!("open should select the desktop open command")
            };
            assert_eq!(args.target, target);
        }
        assert!(Cli::try_parse_from(["stellr", "open"]).is_err());
        assert!(Cli::try_parse_from(["stellr", "open", "one", "two"]).is_err());
    }

    #[test]
    fn a_registered_stellr_protocol_link_is_accepted_as_the_hidden_root_target() {
        let link = "stellr://space?repo=teloverge%2Fstellr&issue=62";
        let parsed = Cli::try_parse_from(["stellr", link]).unwrap();

        assert!(parsed.command.is_none());
        assert_eq!(parsed.protocol_target.as_deref(), Some(link));
    }

    #[test]
    fn appimage_launch_uses_the_callers_original_working_directory() {
        let mounted = std::env::temp_dir().join("mounted-appimage").join("usr");
        let original = std::env::temp_dir().join("stellr-repository");

        assert_eq!(
            super::effective_launch_dir(
                mounted,
                Some("/tmp/Stellr.AppImage".into()),
                Some(original.clone().into_os_string()),
            ),
            original
        );
    }

    #[test]
    fn unpackaged_launch_ignores_an_unpaired_original_working_directory() {
        let current = std::env::temp_dir().join("stellr-repository");
        let unrelated = std::env::temp_dir().join("unrelated");

        assert_eq!(
            super::effective_launch_dir(current.clone(), None, Some(unrelated.into_os_string()),),
            current
        );
    }

    #[test]
    fn appimage_launch_rejects_a_relative_original_working_directory() {
        let mounted = std::env::temp_dir().join("mounted-appimage").join("usr");

        assert_eq!(
            super::effective_launch_dir(
                mounted.clone(),
                Some("/tmp/Stellr.AppImage".into()),
                Some("relative-repository".into()),
            ),
            mounted
        );
    }
}
