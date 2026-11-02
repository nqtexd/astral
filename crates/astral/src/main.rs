#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "console")]
use anyhow::{Context, Result};
use astral::service::{Request, Service};
use astral_core::*;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "astral",
    version,
    about = "Astral native animation studio and agent tools"
)]
struct Args {
    /// Confines project and asset I/O to this directory.
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Mode>,
}
#[derive(Subcommand)]
enum Mode {
    /// Launch the native desktop studio.
    Studio,
    /// Serve MCP over stdio; by default attaches to the running desktop.
    Mcp {
        #[arg(long)]
        headless: bool,
        #[arg(long, value_enum, default_value = "all")]
        profile: astral::mcp::Profile,
    },
    /// Send a typed JSON service request from a file (or - for stdin).
    Request {
        file: String,
        #[arg(long)]
        live: bool,
    },
    /// Inspect a saved project without opening the UI.
    Inspect { path: String },
    /// Validate a saved .astral project.
    Validate { path: String },
    /// Apply a transaction JSON file to a saved project.
    Apply { path: String, batch: String },
    /// Render a PNG preview.
    Render {
        path: String,
        output: String,
        #[arg(long, default_value_t = 0.)]
        frame: f64,
        #[arg(long, default_value_t = 960)]
        width: u32,
        #[arg(long, default_value_t = 540)]
        height: u32,
    },
    /// Create the editable Motion lab example with learned frames and graphics.
    Demo {
        #[arg(default_value = "motion-lab.astral")]
        output: String,
    },
    /// List social-media export presets and file formats.
    ExportPresets,
    /// Export media; the output extension chooses the format.
    Export {
        path: String,
        output: String,
        #[arg(long)]
        hardware: bool,
        #[arg(long, value_enum, default_value = "auto")]
        renderer: astral::export::RenderBackend,
        #[arg(long, value_enum, default_value = "original")]
        preset: astral::export::Preset,
        #[arg(long, value_enum, default_value = "fit")]
        framing: astral::export::Framing,
        #[arg(long, value_enum, default_value = "standard")]
        quality: astral::export::Quality,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long)]
        start: Option<u32>,
        #[arg(long)]
        end: Option<u32>,
    },
    /// Print JSON schemas for service requests and scene command batches.
    Schema,
}
#[tokio::main]
async fn main() -> Result<()> {
    astral::diagnostics::install();
    let args = Args::parse();
    let service = Service::new(args.workspace.unwrap_or_else(astral::default_workspace))?;
    let result = match args.command.unwrap_or(Mode::Studio) {
        Mode::Studio => {
            #[cfg(feature = "desktop")]
            {
                astral::desktop::run(service)?;
                return Ok(());
            }
            #[cfg(not(feature = "desktop"))]
            {
                anyhow::bail!("Rebuild with the desktop feature to launch the studio");
            }
        }
        Mode::Mcp { headless, profile } => {
            astral::mcp::run(service, !headless, profile).await?;
            return Ok(());
        }
        Mode::Request { file, live } => {
            let bytes = if file == "-" {
                use std::io::Read;
                let mut bytes = vec![];
                std::io::stdin()
                    .take(4 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                bytes
            } else {
                std::fs::read(file)?
            };
            anyhow::ensure!(bytes.len() <= 4 * 1024 * 1024, "Request exceeds 4 MiB");
            let request = serde_json::from_slice(&bytes)?;
            if live {
                astral::ipc::call(service.workspace.root(), request).await?
            } else {
                service.request(request)?
            }
        }
        Mode::Inspect { path } => serde_json::to_value(service.workspace.load(path)?)?,
        Mode::Validate { path } => {
            let p = service.workspace.load(path)?;
            {
                let step = (p.duration / 1200).max(1);
                let layout = match astral_core::composition::review(&p, 0, p.duration - 1, step) {
                    Ok(issues) => serde_json::json!({"issues":issues,"step":step}),
                    Err(e) => serde_json::json!({"error":e.to_string()}),
                };
                serde_json::json!({"valid":true,"structural_valid":true,"visual_quality_certified":false,"layout_review":layout,"project":p.id,"revision":p.revision})
            }
        }
        Mode::Apply { path, batch } => {
            let p = service.workspace.load(&path)?;
            service.insert(p)?;
            let batch: Batch = serde_json::from_slice(&std::fs::read(batch)?)?;
            let id = batch.project;
            let result = service.request(Request::ApplyCommands { batch })?;
            service.workspace.save(path, &service.snapshot(id)?)?;
            result
        }
        Mode::Render {
            path,
            output,
            frame,
            width,
            height,
        } => {
            let p = service.workspace.load(path)?;
            let png = astral_render::png(&p, frame, width, height)?;
            service.workspace.atomic_write(&output, &png)?;
            serde_json::json!({"path":output,"bytes":png.len()})
        }
        Mode::Demo { output } => {
            let id = service.demo()?;
            service.request(Request::SaveProject {
                project: id,
                path: output,
            })?
        }
        Mode::ExportPresets => astral::export::catalog(),
        Mode::Export {
            path,
            output,
            hardware,
            renderer,
            preset,
            framing,
            quality,
            width,
            height,
            fps,
            start,
            end,
        } => {
            let id = service.open_local_project(&service.workspace.path(path)?)?;
            let result = service.request(Request::ExportMedia {
                project: id,
                spec: astral::export::ExportSpec {
                    path: output,
                    hardware,
                    renderer,
                    start,
                    end,
                    preset,
                    framing,
                    quality,
                    width,
                    height,
                    fps,
                },
            })?;
            let job: Id = serde_json::from_value(result["job"].clone())?;
            loop {
                let value = service.request(Request::JobStatus { job })?;
                match value["state"].as_str() {
                    Some("completed") => break value,
                    Some("failed" | "cancelled") => anyhow::bail!("{}", value["error"]),
                    _ => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
                }
            }
        }
        Mode::Schema => {
            serde_json::json!({"requests":schemars::schema_for!(Request),"batches":schemars::schema_for!(Batch)})
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&result).context("Serialize response")?
    );
    Ok(())
}
