#[cfg(feature = "desktop")]
use std::{ffi::OsString, path::PathBuf};
use std::{io::Write, sync::Arc, time::Duration};

use clap::Parser;
use stellr_github::{auth::resolve_token, cache::Cache, sync::GithubProvider};
use stellr_server::spaces::SpaceStore;

#[cfg(feature = "desktop")]
use crate::desktop::{self, DesktopLaunch};
use crate::{
    cli::{Cli, Command, ServeArgs},
    runtime::{RuntimeOptions, SessionAuth, start},
};

pub type DynError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(feature = "desktop")]
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

#[cfg(feature = "desktop")]
fn launch_current_dir() -> std::io::Result<PathBuf> {
    Ok(effective_launch_dir(
        std::env::current_dir()?,
        std::env::var_os("APPIMAGE"),
        std::env::var_os("OWD"),
    ))
}

#[cfg(feature = "desktop")]
fn desktop_launch_from_cli(cli: Cli, cwd: PathBuf) -> Result<DesktopLaunch, DynError> {
    match cli.command {
        Some(Command::Open(args)) => Ok(DesktopLaunch {
            cwd,
            target: Some(args.target),
            restore_route: false,
        }),
        None => Ok(DesktopLaunch {
            cwd,
            restore_route: cli.protocol_target.is_none(),
            target: cli.protocol_target,
        }),
        Some(Command::Serve(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "serve is available through the stellr CLI executable",
        )
        .into()),
        #[cfg(all(debug_assertions, feature = "desktop"))]
        Some(Command::Acceptance(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "acceptance is available through stellr.exe",
        )
        .into()),
    }
}

#[cfg(feature = "desktop")]
pub fn desktop_launch_from<I, T>(args: I, cwd: PathBuf) -> Result<DesktopLaunch, DynError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    desktop_launch_from_cli(Cli::try_parse_from(args)?, cwd)
}

#[cfg(feature = "desktop")]
pub fn run_desktop() -> Result<(), DynError> {
    let launch = desktop_launch_from(std::env::args_os(), launch_current_dir()?)?;
    desktop::run(launch).map_err(Into::into)
}

pub fn run_cli() -> Result<(), DynError> {
    let cli = Cli::parse();
    match cli {
        #[cfg(all(debug_assertions, feature = "desktop"))]
        Cli {
            command: Some(Command::Acceptance(args)),
            ..
        } => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(crate::acceptance::run(args.github_base, args.profile)),
        Cli {
            command: Some(Command::Serve(args)),
            ..
        } => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(serve(args)),
        #[cfg(feature = "desktop")]
        desktop_cli => {
            let launch = desktop_launch_from_cli(desktop_cli, launch_current_dir()?)?;
            desktop::run(launch).map_err(Into::into)
        }
        #[cfg(not(feature = "desktop"))]
        Cli { command: None } => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the headless build requires the `serve` command",
        )
        .into()),
    }
}

async fn serve(args: ServeArgs) -> Result<(), DynError> {
    let provider_token = resolve_token()?;
    let provider = Arc::new(GithubProvider::new(provider_token)?);
    let session_auth = if args.no_token {
        SessionAuth::Disabled
    } else if let Some(token) = std::env::var_os("STELLR_SESSION_TOKEN") {
        let token = token.into_string().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "STELLR_SESSION_TOKEN must be valid UTF-8",
            )
        })?;
        if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "STELLR_SESSION_TOKEN must be exactly 32 hexadecimal characters",
            )
            .into());
        }
        SessionAuth::Fixed(token.to_ascii_lowercase())
    } else {
        SessionAuth::Required
    };
    let runtime = start(
        RuntimeOptions {
            address: args.addr,
            session_auth,
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
    #[cfg(feature = "desktop")]
    use std::path::PathBuf;

    use clap::Parser;

    #[cfg(feature = "desktop")]
    use super::{desktop_launch_from, effective_launch_dir};
    use crate::cli::{Cli, Command};

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_entry_accepts_bare_open_and_protocol_launches() {
        let cwd = PathBuf::from(r"D:\Apps\Stellr");

        let bare = desktop_launch_from(["stellr-desktop"], cwd.clone()).unwrap();
        assert!(bare.target.is_none());
        assert!(bare.restore_route);

        let open = desktop_launch_from(["stellr-desktop", "open", "teloverge/stellr"], cwd.clone())
            .unwrap();
        assert_eq!(open.target.as_deref(), Some("teloverge/stellr"));
        assert!(!open.restore_route);

        let protocol = desktop_launch_from(
            ["stellr-desktop", "stellr://space?repo=teloverge%2Fstellr"],
            cwd,
        )
        .unwrap();
        assert_eq!(
            protocol.target.as_deref(),
            Some("stellr://space?repo=teloverge%2Fstellr")
        );
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_entry_rejects_console_only_serve() {
        let error = desktop_launch_from(
            ["stellr-desktop", "serve"],
            PathBuf::from(r"D:\Apps\Stellr"),
        )
        .err()
        .expect("the desktop entry must reject console-only serve");

        assert!(error.to_string().contains("stellr CLI"));
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn bare_launch_selects_desktop_mode_without_a_repository_target() {
        let launch =
            desktop_launch_from(["stellr"], std::path::PathBuf::from(r"D:\Apps\Stellr")).unwrap();

        assert!(launch.target.is_none());
        assert!(launch.restore_route);
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

    #[cfg(feature = "desktop")]
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

    #[cfg(feature = "desktop")]
    #[test]
    fn a_registered_stellr_protocol_link_is_accepted_as_the_hidden_root_target() {
        let link = "stellr://space?repo=teloverge%2Fstellr&issue=62";
        let parsed = Cli::try_parse_from(["stellr", link]).unwrap();

        assert!(parsed.command.is_none());
        assert_eq!(parsed.protocol_target.as_deref(), Some(link));
    }

    #[test]
    fn an_arbitrary_root_argument_is_an_invalid_console_command() {
        assert!(Cli::try_parse_from(["stellr", "not-a-command"]).is_err());
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn appimage_launch_uses_the_callers_original_working_directory() {
        let mounted = std::env::temp_dir().join("mounted-appimage").join("usr");
        let original = std::env::temp_dir().join("stellr-repository");

        assert_eq!(
            effective_launch_dir(
                mounted,
                Some("/tmp/Stellr.AppImage".into()),
                Some(original.clone().into_os_string()),
            ),
            original
        );
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn unpackaged_launch_ignores_an_unpaired_original_working_directory() {
        let current = std::env::temp_dir().join("stellr-repository");
        let unrelated = std::env::temp_dir().join("unrelated");

        assert_eq!(
            effective_launch_dir(current.clone(), None, Some(unrelated.into_os_string()),),
            current
        );
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn appimage_launch_rejects_a_relative_original_working_directory() {
        let mounted = std::env::temp_dir().join("mounted-appimage").join("usr");

        assert_eq!(
            effective_launch_dir(
                mounted.clone(),
                Some("/tmp/Stellr.AppImage".into()),
                Some("relative-repository".into()),
            ),
            mounted
        );
    }
}
