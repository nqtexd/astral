#![cfg_attr(windows, windows_subsystem = "windows")]
//! Desktop launcher without a console window. The `astral` binary remains a
//! normal console executable for CLI and MCP stdio clients.
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "Astral Studio", version)]
struct Args {
    #[arg(long)]
    workspace: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let root = args.workspace.unwrap_or_else(astral::default_workspace);
    let result = astral::service::Service::new(root).and_then(astral::desktop::run);
    if let Err(error) = result {
        rfd::MessageDialog::new()
            .set_title("Astral could not start")
            .set_description(format!("{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
