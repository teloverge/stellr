mod cli;

use std::{io::Write, sync::Arc, time::Duration};

use clap::Parser;
use stellr_app::runtime::{RuntimeOptions, SessionAuth, start};
use stellr_github::{auth::resolve_token, cache::Cache, sync::GithubProvider};
use stellr_server::spaces::SpaceStore;

use crate::cli::{Cli, Command, ServeArgs};

type DynError = Box<dyn std::error::Error + Send + Sync>;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), DynError> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Serve(args)) => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(serve(args)),
        Some(Command::Open(args)) => {
            let cwd = std::env::current_dir()?;
            stellr_app::desktop::run(stellr_app::desktop::DesktopLaunch {
                cwd,
                target: args.target,
                restore_route: false,
            })
            .map_err(Into::into)
        }
        None => {
            let cwd = std::env::current_dir()?;
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
}
