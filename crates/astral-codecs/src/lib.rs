//! Transitional codec adapter. Legacy source data is preserved losslessly;
//! conversion coverage is explicit rather than silently discarding features.
use anyhow::{Context, Result, ensure};
use astral_core::*;
use astral_storage::Workspace;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

mod native;
mod sampled;

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
    ensure!(
        bytes.len() <= 128 * 1024 * 1024,
        "Legacy source exceeds 128 MiB"
    );
    let (program, script) = if ["styk", "stkz", "stykmov"].contains(&extension.as_str()) {
        (
            "node",
            application_root.join("scripts/astral-decode-container.mjs"),
        )
    } else if ["nodes", "nodemc", "stknds"].contains(&extension.as_str()) {
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
    let executable = if program == "node" {
        astral_storage::node_executable(application_root)
    } else {
        std::path::PathBuf::from(program)
    };
    let mut command = Command::new(executable);
    command
        .arg(script)
        .arg(&input)
        .arg("--stdin")
        .current_dir(application_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .context("Start legacy codec (requires development Python/Node)")?;
    let sent = {
        let mut input = child.stdin.take().context("Decoder input unavailable")?;
        input.write_all(&bytes)
    };
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "Legacy decoder: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    sent.context("Send immutable source snapshot to decoder")?;
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
    let original_project = project.clone();
    let evaluation = evaluate_source(&report, application_root);
    let (mut warnings, conversion) = match evaluation {
        Ok(samples) => match sampled::convert(&mut project, &samples) {
            Ok(warnings) => (warnings, "source-evaluated-native-cels"),
            Err(error) => {
                project = original_project;
                let mut warnings = native::convert(&mut project, &report)?;
                warnings.push(format!(
                    "Evaluated conversion could not be represented natively: {error}"
                ));
                (warnings, "authored-pose-cels")
            }
        },
        Err(error) => {
            let mut warnings = native::convert(&mut project, &report)?;
            warnings.push(format!("Evaluated conversion unavailable: {error}"));
            (warnings, "authored-pose-cels")
        }
    };
    warnings.extend(sampled::audio(&mut project, &report, workspace)?);
    project.schema = project.schema.max(9);
    project.original_source = Some(std::sync::Arc::new(OriginalSource {
        filename: path.into(),
        source_base64: STANDARD.encode(&bytes).into(),
        canonical_json: serde_json::to_string(&report)?.into(),
    }));
    project.metadata.insert("legacy".into(),json!({"filename":path,"conversion":if project.cels.is_empty(){"archive-only"}else{conversion},"native_rigs":project.characters.len(),"native_cels":project.cels.len(),"original_bytes":bytes.len(),"warnings":warnings}));
    project.validate()?;
    Ok((project, warnings))
}

fn evaluate_source(report: &Value, root: &Path) -> Result<Value> {
    use std::io::Write;
    let script = root.join("scripts/astral-evaluate-legacy.mjs");
    ensure!(script.is_file(), "Source evaluator is unavailable");
    let mut command = Command::new(astral_storage::node_executable(root));
    command
        .arg(script)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .context("Start source evaluator (Node required for import only)")?;
    let mut input = child
        .stdin
        .take()
        .context("Source evaluator input unavailable")?;
    serde_json::to_writer(&mut input, report)?;
    input.flush()?;
    drop(input);
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "Source evaluation: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        output.stdout.len() <= astral_storage::MAX_PROJECT_BYTES,
        "Source evaluation exceeds project size budget"
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn export_original(project: &Project) -> Result<Vec<u8>> {
    if let Some(source) = &project.original_source {
        return Ok(STANDARD.decode(source.source_base64.as_ref())?);
    }
    let source = project
        .metadata
        .get("legacy")
        .and_then(|v| v.get("source_base64"))
        .and_then(Value::as_str)
        .context("Project has no archived original")?;
    Ok(STANDARD.decode(source)?)
}
