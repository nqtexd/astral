//! Transitional codec adapter. Legacy source data is preserved losslessly;
//! conversion coverage is explicit rather than silently discarding features.
use anyhow::{Context, Result, ensure};
use astral_core::*;
use astral_storage::Workspace;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Stdio},
};

pub fn import_legacy(
    workspace: &Workspace,
    path: &str,
    application_root: &Path,
) -> Result<(Project, Vec<String>)> {
    let input = workspace.path(path)?;
    let extension = input
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    ensure!(
        [
            "styk", "stkz", "stykmov", "piv", "stk", "nodes", "nodemc", "stknds"
        ]
        .contains(&extension.as_str()),
        "Unsupported legacy extension"
    );
    let bytes = workspace.read(path)?;
    let (program, script) = if ["nodes", "nodemc", "stknds"].contains(&extension.as_str()) {
        (
            "node",
            application_root.join("scripts/astral-sticknodes.mjs"),
        )
    } else {
        ("python", application_root.join("scripts/astral-codec.py"))
    };
    ensure!(
        script.is_file(),
        "Legacy bridge is unavailable; keep the original application installed"
    );
    let mut command = Command::new(program);
    command
        .arg(script)
        .arg(&input)
        .current_dir(application_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .context("Start legacy codec (requires development Python/Node)")?;
    ensure!(
        output.status.success(),
        "Legacy decoder: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        output.stdout.len() <= astral_storage::MAX_PROJECT_BYTES,
        "Decoded document exceeds budget"
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let mut project = Project::new(
        input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported"),
        report["canvas_width"]
            .as_u64()
            .unwrap_or(960)
            .clamp(1, 8192) as u32,
        report["canvas_height"]
            .as_u64()
            .unwrap_or(540)
            .clamp(1, 8192) as u32,
        report["fps"].as_u64().unwrap_or(24).clamp(1, 240) as u32,
        300,
    );
    // Archive complete source and canonical document before any future conversion.
    project.metadata.insert("legacy".into(),json!({"filename":path,"source_base64":STANDARD.encode(&bytes),"document":report,"conversion":"archive-only"}));
    let warnings=vec!["Legacy file decoded and archived losslessly. Native conversion of legacy geometry, animation, media and effects is not implemented yet. Continue editing this file in the preserved legacy app; Astral will not display an approximate conversion.".into()];
    project.validate()?;
    Ok((project, warnings))
}

pub fn export_original(project: &Project) -> Result<Vec<u8>> {
    let source = project
        .metadata
        .get("legacy")
        .and_then(|v| v.get("source_base64"))
        .and_then(Value::as_str)
        .context("Project has no archived original")?;
    Ok(STANDARD.decode(source)?)
}
