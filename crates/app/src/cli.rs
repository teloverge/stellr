use std::num::NonZeroU64;
#[cfg(debug_assertions)]
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "stellr")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[arg(value_name = "STELLR_LINK", hide = true)]
    pub protocol_target: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    #[cfg(debug_assertions)]
    #[command(hide = true)]
    Acceptance(AcceptanceArgs),
    Serve(ServeArgs),
    Open(OpenArgs),
}

#[cfg(debug_assertions)]
#[derive(Args)]
pub struct AcceptanceArgs {
    #[arg(long)]
    pub github_base: String,
    #[arg(long)]
    pub profile: PathBuf,
}

#[derive(Args)]
pub struct OpenArgs {
    pub target: String,
}

#[derive(Args)]
pub struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:8787")]
    pub addr: String,
    #[arg(long)]
    pub no_token: bool,
    #[arg(long)]
    pub issue: Option<NonZeroU64>,
}
