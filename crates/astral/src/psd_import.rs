//! Advanced PSD bridge. Native graph, masks, groups and basic editable text.
use crate::artwork::{bitmap, layer};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::{
    io::Write,
    path::Path,
    process::{Command as Process, Stdio},
};

pub fn import(
    bytes: &[u8],
    root: &Path,
    p: &Project,
    start: u32,
    job: &crate::export::Job,
) -> Result<(Vec<Command>, Vec<String>, Vec<Id>)> {
    ensure!(bytes.len() <= 32 * 1024 * 1024, "PSD input exceeds 32 MiB");
    job.check()?;
    let directory = tempfile::tempdir()?;
    let mut input = tempfile::NamedTempFile::new_in(directory.path())?;
    input.write_all(bytes)?;
    let output = tempfile::NamedTempFile::new_in(directory.path())?;
    let errors = tempfile::NamedTempFile::new_in(directory.path())?;
    let mut process = Process::new(astral_storage::node_executable(root));
    process
        .args(["--max-old-space-size=384"])
        .arg(root.join("scripts/astral-psd.mjs"))
        .args(["snapshot.psd", "--stdin"])
        .stdin(Stdio::from(input.reopen()?))
        .stdout(Stdio::from(output.reopen()?))
        .stderr(Stdio::from(errors.reopen()?));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        process.creation_flags(0x08000000);
    }
    *job.child.lock().unwrap_or_else(|e| e.into_inner()) = Some(
        process
            .spawn()
            .context("Packaged PSD parser runtime is missing")?,
    );
    let started = std::time::Instant::now();
    let status = (|| -> Result<_> {
        loop {
            job.check()?;
            ensure!(
                started.elapsed() < std::time::Duration::from_secs(60),
                "PSD decoder timed out"
            );
            ensure!(
                output.as_file().metadata()?.len() <= 96 * 1024 * 1024
                    && errors.as_file().metadata()?.len() <= 1024 * 1024,
                "PSD decoder output exceeds budget"
            );
            let value = job
                .child
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_mut()
                .context("PSD process unavailable")?
                .try_wait()?;
            if let Some(status) = value {
                break Ok(status);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    })();
    if let Some(mut child) = job.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
        if status.is_err() {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
    let status = status?;
    ensure!(
        status.success(),
        "PSD decoder rejected unsupported or malformed layer data: {}",
        String::from_utf8_lossy(&std::fs::read(errors.path())?)
            .chars()
            .take(1500)
            .collect::<String>()
    );
    job.check()?;
    ensure!(
        output.as_file().metadata()?.len() <= 96 * 1024 * 1024,
        "PSD report exceeds budget"
    );
    let output = std::fs::read(output.path())?;
    let report: Value = serde_json::from_slice(&output)?;
    ensure!(
        report["layers"].as_array().is_some_and(|a| !a.is_empty()),
        "PSD has no separate layers"
    );
    let icc = report["icc"]
        .as_str()
        .map(|v| STANDARD.decode(v))
        .transpose()?;
    ensure!(
        icc.as_ref().is_none_or(|v| v.len() <= 4 * 1024 * 1024),
        "PSD ICC profile exceeds 4 MiB"
    );
    let mut builder = Builder {
        icc,
        job,
        commands: vec![],
        nodes: vec![],
        ids: vec![],
        warnings: report["warnings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect(),
        start,
        end: p.duration - 1,
    };
    let output = builder.group(&report["layers"])?;
    let graph = Composition {
        id: Id::new_v4(),
        name: "PSD composition".into(),
        enabled: true,
        layer: GraphicLayer::Background,
        output,
        nodes: builder.nodes,
    };
    let mut graphs = p.compositions.clone();
    graphs.push(graph);
    builder.commands.push(Command::SetCompositions {
        compositions: graphs,
    });
    Ok((builder.commands, builder.warnings, builder.ids))
}
struct Builder<'a> {
    icc: Option<Vec<u8>>,
    job: &'a crate::export::Job,
    commands: Vec<Command>,
    nodes: Vec<CompositeNode>,
    ids: Vec<Id>,
    warnings: Vec<String>,
    start: u32,
    end: u32,
}
impl Builder<'_> {
    fn node(&mut self, name: impl Into<String>, op: CompositeOp) -> Id {
        let id = Id::new_v4();
        self.nodes.push(CompositeNode {
            id,
            name: name.into(),
            op,
        });
        id
    }
    fn image(&mut self, v: &Value, name: &str, manage_color: bool) -> Result<Id> {
        self.job.check()?;
        let png = STANDARD.decode(v["png"].as_str().context("PSD image missing")?)?;
        let mut image =
            image::load_from_memory_with_format(&png, image::ImageFormat::Png)?.into_rgba8();
        if manage_color && let Some(icc) = &self.icc {
            astral_render::color::to_srgb(&mut image, icc)?;
        }
        let b = bitmap(name.into(), image)?;
        let r = layer(
            name.into(),
            &b,
            self.start,
            self.end,
            Point::new(
                v["left"].as_f64().unwrap_or(0.) as f32 + b.width as f32 / 2.,
                v["top"].as_f64().unwrap_or(0.) as f32 + b.height as f32 / 2.,
            ),
        );
        let id = r.id;
        self.ids.push(id);
        self.commands.push(Command::AddBitmap { bitmap: b });
        self.commands.push(Command::PutRasterLayer { layer: r });
        Ok(self.node(name, CompositeOp::Raster { layer: id }))
    }
    fn group(&mut self, children: &Value) -> Result<Id> {
        let mut below = self.node(
            "Transparent",
            CompositeOp::Solid {
                color: [0, 0, 0, 0],
            },
        );
        for child in children.as_array().context("Invalid PSD group")? {
            self.job.check()?;
            let name = child["name"].as_str().unwrap_or("PSD layer");
            let mut above = if child["children"].is_array() {
                self.group(&child["children"])?
            } else if child["text"].is_object() {
                self.text(child)?
            } else if child["png"].is_string() {
                self.image(child, name, true)?
            } else {
                self.warnings.push(format!(
                    "{name}: no editable pixels/text; retained in PSD original"
                ));
                continue;
            };
            if child["mask"].is_object() {
                let m = &child["mask"];
                let mask = self.image(m, &format!("{name} mask"), false)?;
                let base = m["default_color"].as_u64().unwrap_or(0).min(255) as u8;
                let bg = self.node(
                    "Mask outside",
                    CompositeOp::Solid {
                        color: [base, base, base, 255],
                    },
                );
                let mask = self.node(
                    "Mask bounds",
                    CompositeOp::Blend {
                        below: bg,
                        above: mask,
                        mode: BlendMode::Normal,
                        opacity: 1.,
                    },
                );
                above = self.node(
                    format!("{name} mask"),
                    CompositeOp::Mask {
                        source: above,
                        mask,
                        invert: false,
                        luminance: true,
                    },
                );
            }
            let mode = match child["blend"].as_str().unwrap_or("normal") {
                "normal" => BlendMode::Normal,
                "multiply" => BlendMode::Multiply,
                "screen" => BlendMode::Screen,
                "overlay" => BlendMode::Overlay,
                "darken" => BlendMode::Darken,
                "lighten" => BlendMode::Lighten,
                "difference" => BlendMode::Difference,
                "linear dodge" => BlendMode::Add,
                other => {
                    self.warnings
                        .push(format!("{name}: blend {other} replaced with normal"));
                    BlendMode::Normal
                }
            };
            let opacity = if child["visible"].as_bool().unwrap_or(true) {
                child["opacity"].as_f64().unwrap_or(1.).clamp(0., 1.) as f32
            } else {
                0.
            };
            below = self.node(
                format!("Blend {name}"),
                CompositeOp::Blend {
                    below,
                    above,
                    mode,
                    opacity,
                },
            );
        }
        Ok(below)
    }
    fn text(&mut self, v: &Value) -> Result<Id> {
        let text = &v["text"];
        let name = v["name"].as_str().unwrap_or("PSD text").to_string();
        let c = &text["color"];
        let channel = |name: &str| c[name].as_f64().unwrap_or(0.).clamp(0., 255.) as u8;
        let id = Id::new_v4();
        let size = Point::new(
            v["width"].as_f64().unwrap_or(240.).max(8.) as f32,
            v["height"].as_f64().unwrap_or(80.).max(8.) as f32,
        );
        let font = text["font"].as_str().unwrap_or("").to_lowercase();
        let formatting = TextFormatting {
            font: if font.contains("bold") {
                TextFont::Bold
            } else {
                TextFont::Regular
            },
            align: match text["align"].as_str() {
                Some("center") => TextAlign::Center,
                Some("right") => TextAlign::Right,
                _ => TextAlign::Left,
            },
            line_height: 1.2,
            padding: 0.,
        };
        let dialogue = Dialogue {
            id,
            name: name.clone(),
            text: text["value"].as_str().unwrap_or("").replace('\r', "\n"),
            speaker: String::new(),
            start: self.start,
            end: self.end,
            position: Point::new(
                v["left"].as_f64().unwrap_or(0.) as f32 + size.x / 2.,
                v["top"].as_f64().unwrap_or(0.) as f32 + size.y / 2.,
            ),
            size,
            font_size: text["size"].as_f64().unwrap_or(24.).clamp(8., 200.) as f32,
            color: [0, 0, 0, 0],
            text_color: [channel("r"), channel("g"), channel("b"), 255],
            appearance: DialogueAppearance {
                border_width: 0.,
                ..Default::default()
            },
            style: DialogueStyle::Caption,
            character: None,
            typewriter: 0.,
            formatting,
        };
        self.commands.push(Command::AddDialogue { dialogue });
        Ok(self.node(name, CompositeOp::Dialogue { dialogue: id }))
    }
}
