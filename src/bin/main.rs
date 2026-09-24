// ============================================================================
// main.rs — apfel CLI executable entry point
// Part of apfel-rs
// ============================================================================

use apfel::cli::{run_cli, CliArgs};
use clap::Parser;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    if args.debug {
        tracing_subscriber::fmt()
            .with_env_filter("apfel=debug,tower_http=debug")
            .init();
    }

    let code = run_cli(args).await;
    std::process::exit(code);
}
