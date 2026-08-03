use std::num::NonZeroU64;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "stellr")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Serve(ServeArgs),
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
