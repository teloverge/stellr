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
        None => stellr_app::desktop::run(std::env::current_dir()?).map_err(Into::into),
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
}
