mod cli;

use std::{io::Write, sync::Arc, time::Duration};

use clap::Parser;
use stellr_core::Model;
use stellr_github::{auth::resolve_token, cache::Cache, sync::GithubProvider};
use stellr_server::{poll::spawn_poller, routes::router, spaces::SpaceStore, state::AppState};

use crate::cli::{Cli, Command, ServeArgs};

type DynError = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), DynError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServeArgs) -> Result<(), DynError> {
    let provider_token = resolve_token()?;
    let provider = Arc::new(GithubProvider::new(provider_token)?);
    let session_token = if args.no_token {
        None
    } else {
        Some(session_token().map_err(|error| {
            std::io::Error::other(format!("could not generate session token: {error}"))
        })?)
    };
    let spaces = SpaceStore::load(SpaceStore::default_file());
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: session_token.clone(),
        spaces: tokio::sync::Mutex::new(spaces),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    spawn_poller(
        state.clone(),
        provider,
        Cache::new(Cache::default_root()),
        Duration::from_secs(30),
    );

    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    let address = listener.local_addr()?;
    let url = cockpit_url(address, session_token.as_deref(), args.issue);
    println!("stellr cockpit: {url}");
    std::io::stdout().flush()?;

    axum::serve(listener, router(state)).await?;
    Ok(())
}

fn cockpit_url(
    address: std::net::SocketAddr,
    token: Option<&str>,
    issue: Option<std::num::NonZeroU64>,
) -> String {
    let mut query = Vec::new();
    if let Some(token) = token {
        query.push(format!("token={token}"));
    }
    if let Some(issue) = issue {
        query.push(format!("issue={issue}"));
    }
    let suffix = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.join("&"))
    };
    format!("http://{address}/{suffix}")
}

fn session_token() -> Result<String, getrandom::Error> {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)?;
    let mut token = String::with_capacity(32);
    for byte in bytes {
        token.push(HEX[(byte >> 4) as usize] as char);
        token.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use clap::Parser;

    use crate::{
        cli::{Cli, Command},
        session_token,
    };

    #[test]
    fn serve_defaults_to_loopback_port_8787_with_session_auth() {
        let parsed = Cli::try_parse_from(["stellr", "serve"]).unwrap();
        let Command::Serve(args) = parsed.command;

        assert_eq!(args.addr, "127.0.0.1:8787");
        assert!(!args.no_token);
        assert_eq!(args.issue, None);
    }

    #[test]
    fn serve_accepts_a_custom_address_and_open_server_flag() {
        let parsed =
            Cli::try_parse_from(["stellr", "serve", "--addr", "127.0.0.1:0", "--no-token"])
                .unwrap();
        let Command::Serve(args) = parsed.command;

        assert_eq!(args.addr, "127.0.0.1:0");
        assert!(args.no_token);
    }

    #[test]
    fn serve_accepts_a_positive_conversation_issue() {
        let parsed =
            Cli::try_parse_from(["stellr", "serve", "--addr", "127.0.0.1:0", "--issue", "14"])
                .unwrap();
        let Command::Serve(args) = parsed.command;

        assert_eq!(args.issue.map(NonZeroU64::get), Some(14));
        assert!(Cli::try_parse_from(["stellr", "serve", "--issue", "0"]).is_err());
    }

    #[test]
    fn session_token_is_32_lowercase_hex_characters() {
        let token = session_token().unwrap();

        assert_eq!(token.len(), 32);
        assert!(
            token
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
    }
}
