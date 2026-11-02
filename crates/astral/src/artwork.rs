//! Bounded artwork import, immutable bitmap edits and audio-driven mouth keys.
use crate::service::{Request, Service};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Cursor, sync::Arc};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[schemars(extend("type" = "object"))]
pub enum ArtworkRequest {
    Audio {
        project: Id,
        expected_revision: u64,
        path: String,
        start: u32,
    },
    Track {
        project: Id,
        expected_revision: u64,
        layer: Id,
        start: u32,
        end: u32,
        point: Point,
        #[serde(default)]
        second_point: Option<Point>,
        /// Native graphic or raster layer. Omit to drive the camera.
        #[serde(default)]
        target: Option<Id>,
        #[serde(default)]
        apply: bool,
    },
    Import {
        project: Id,
        expected_revision: u64,
        paths: Vec<String>,
        start: u32,
        source_fps: u32,
        #[serde(default)]
        looping: bool,
    },
    Paint {
        project: Id,
        expected_revision: u64,
        layer: Id,
        frame: u32,
        points: Vec<Point>,
        radius: f32,
        color: Color,
        #[serde(default)]
        erase: bool,
    },
    LipSync {
        #[serde(default)]
        engine: crate::speech::Recognizer,
        #[serde(default)]
        mapping: std::collections::BTreeMap<String, MouthShape>,
        project: Id,
        expected_revision: u64,
        clip: Id,
        character: Id,
        #[serde(default)]
        text: String,
        #[serde(default)]
        threshold: Option<f32>,
    },
    Video {
        project: Id,
        expected_revision: u64,
        path: String,
        start: u32,
        frames: u32,
        fps: u32,
        width: u32,
        height: u32,
    },
}
pub fn bitmap(name: String, image: image::RgbaImage) -> Result<Bitmap> {
    ensure!(
        image.width() > 0
            && image.height() > 0
            && image.width() as u64 * image.height() as u64 <= 16_777_216,
        "Bitmap exceeds 16 megapixels"
    );
    let mut bytes = Cursor::new(vec![]);
    image.write_to(&mut bytes, image::ImageFormat::Png)?;
    Ok(Bitmap {
        id: Id::new_v4(),
        name,
        width: image.width(),
        height: image.height(),
        png: Arc::from(STANDARD.encode(bytes.into_inner())),
    })
}
fn decode(bytes: &[u8]) -> Result<image::RgbaImage> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    use image::ImageDecoder;
    let mut decoder = reader.into_decoder()?;
    let profile = decoder.icc_profile()?;
    let mut image = image::DynamicImage::from_decoder(decoder)?.into_rgba8();
    if let Some(profile) = profile {
        astral_render::color::to_srgb(&mut image, &profile)?;
    }
    ensure!(
        image.width() as u64 * image.height() as u64 <= 16_777_216,
        "Bitmap exceeds 16 megapixels"
    );
    Ok(image)
}
pub(crate) fn layer(
    name: String,
    source: &Bitmap,
    start: u32,
    end: u32,
    position: Point,
) -> RasterLayer {
    RasterLayer {
        id: Id::new_v4(),
        name,
        start,
        end,
        sources: vec![source.id],
        source_fps: 24,
        looping: false,
        size: Point::new(source.width as f32, source.height as f32),
        keys: vec![RasterKey {
            frame: start,
            position,
            scale: Point::new(1., 1.),
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Smooth,
        }],
        cels: vec![],
        mesh: None,
        layer: GraphicLayer::Background,
        visible: true,
    }
}
impl Service {
    pub fn artwork(&self, request: ArtworkRequest) -> Result<Value> {
        match request {
            ArtworkRequest::Audio {
                project,
                expected_revision,
                path,
                start,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                ensure!(start < p.duration, "Audio starts outside project");
                let workspace = self.project_workspace(project);
                let service = self.clone();
                self.background_task(project,expected_revision,move|job|{
                    job.check()?;job.update(json!({"state":"decoding_audio","progress":0.1}));
                    let bytes=workspace.read(&path)?;let wav=astral_audio::normalize_wav(&bytes)?;job.check()?;
                    let seconds=astral_audio::wav_duration(&wav)?;let asset=Id::new_v4();let clip=Id::new_v4();let output=format!("assets/audio/{asset}.wav");workspace.atomic_write(&output,&wav)?;
                    let name=std::path::Path::new(&path).file_name().and_then(|s|s.to_str()).unwrap_or("Audio").to_string();
                    let end=start+(seconds*p.fps as f64).ceil() as u32;let mut commands=vec![];if end>p.duration{commands.push(Command::SetProject{name:None,duration:Some(end),background:None});}
                    commands.push(Command::AddAsset{asset:Asset{id:asset,name:name.clone(),path:output.clone(),kind:"audio".into(),tags:vec!["normalized_pcm".into()]}});
                    commands.push(Command::AddAudio{clip:AudioClip{id:clip,name,asset,start,offset_seconds:0.,duration_seconds:seconds,gain:1.,fade_in:0.,fade_out:0.,looping:false,speed:1.}});
                    job.check()?;let receipt=service.request(Request::ApplyCommands{batch:Batch{project,expected_revision,commands}})?;
                    Ok(json!({"project":project,"revision":receipt["revision"],"asset":asset,"clip":clip,"duration_seconds":seconds,"path":output,"receipt":receipt}))
                })
            }
            ArtworkRequest::Track {
                project,
                expected_revision,
                layer,
                start,
                end,
                point,
                second_point,
                target,
                apply,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision
                        && start <= end
                        && end < p.duration
                        && end - start < 120
                        && point.finite(),
                    "Track a valid range of at most 120 frames at the current revision"
                );
                let r = p
                    .rasters
                    .iter()
                    .find(|r| r.id == layer)
                    .context("Raster layer missing")?
                    .clone();
                ensure!(
                    start >= r.start && end <= r.end,
                    "Track inside the artwork range"
                );
                let service = self.clone();
                ensure!(
                    target.is_none_or(|id| p.graphics.iter().any(|g| g.id == id)
                        || p.rasters.iter().any(|r| r.id == id)),
                    "Tracking target must be a graphic or raster layer"
                );
                ensure!(
                    second_point.is_none_or(|q| q.finite() && q.distance(point) >= 12.)
                        && (target.is_some() || second_point.is_none()),
                    "Two-point rotation tracking needs an object target and separated source points"
                );
                let mut jobs = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                jobs.jobs.retain(|_, j| !j.finished());
                ensure!(jobs.jobs.len() < 8, "Too many active jobs");
                let job = crate::export::background(project, p.revision, move |job| {
                    let mut result = track(&p, &r, start, end, point, job)?;
                    let second = second_point
                        .map(|q| track(&p, &r, start, end, q, job))
                        .transpose()?;
                    if let Some(target) = target {
                        result.1 = follow_track(
                            &p,
                            target,
                            start,
                            end,
                            &result.0,
                            second.as_ref().map(|r| r.0.as_slice()),
                        )?;
                    }
                    let mut value = json!({"project":project,"revision":p.revision,"points":result.0,"plan":{"name":"Track artwork camera","commands":result.1},"warnings":["Local template tracking follows visible texture, not semantic objects. Inspect the points before applying; occlusion, blur or similar shapes can cause drift."],"applied":false});
                    if apply {
                        job.check()?;
                        value["receipt"] = service.request(Request::ApplyCommands {
                            batch: Batch {
                                project,
                                expected_revision,
                                commands: result.1,
                            },
                        })?;
                        value["applied"] = true.into();
                    }
                    Ok(value)
                });
                let id = job.id;
                jobs.jobs.insert(id, job);
                Ok(json!({"job":id}))
            }
            ArtworkRequest::Import {
                project,
                expected_revision,
                paths,
                start,
                source_fps,
                looping,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect before importing"
                );
                ensure!(
                    !paths.is_empty()
                        && paths.len() <= 240
                        && start < p.duration
                        && (1..=240).contains(&source_fps),
                    "Import 1–240 images with valid timing"
                );
                let workspace = self.project_workspace(project);
                for path in &paths {
                    let absolute = workspace.path(path)?;
                    ensure!(
                        absolute.is_file()
                            && std::fs::metadata(absolute)?.len() <= 32 * 1024 * 1024,
                        "Image must exist and fit the 32 MiB input limit"
                    );
                }
                let service = self.clone();
                let mut jobs = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                jobs.jobs.retain(|_, j| !j.finished());
                ensure!(jobs.jobs.len() < 8, "Too many active jobs");
                let job = crate::export::background(project, p.revision, move |job| {
                    let mut commands = vec![];
                    let mut warnings = vec![];
                    let mut ids = vec![];
                    if paths.len() == 1
                        && std::path::Path::new(&paths[0])
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("psd"))
                    {
                        let bytes = workspace.read(&paths[0])?;
                        let (native, notes, created) = crate::psd_import::import(
                            &bytes,
                            &service.application_root,
                            &p,
                            start,
                            job,
                        )?;
                        commands.extend(native);
                        warnings.extend(notes);
                        ids.extend(created);
                        commands.push(Command::AddAsset {
                            asset: Asset {
                                id: Id::new_v4(),
                                name: std::path::Path::new(&paths[0])
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into(),
                                path: paths[0].clone(),
                                kind: "source".into(),
                                tags: vec!["psd".into(), "original".into()],
                            },
                        });
                    } else {
                        let mut sources = vec![];
                        let mut first = None;
                        let mut pixels = 0u64;
                        for (i, path) in paths.iter().enumerate() {
                            job.check()?;
                            let bytes = workspace.read(path)?;
                            if path.to_lowercase().ends_with(".gif") {
                                warnings.push("GIF import uses its first image; import a PNG sequence for animated sprites".into());
                            }
                            let image = decode(&bytes)?;
                            pixels += image.width() as u64 * image.height() as u64;
                            ensure!(
                                pixels <= 134_217_728,
                                "Sequence exceeds 128-megapixel project budget"
                            );
                            let b = bitmap(
                                std::path::Path::new(path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into(),
                                image,
                            )?;
                            if first.is_none() {
                                first = Some(b.clone());
                            }
                            sources.push(b.id);
                            commands.push(Command::AddBitmap { bitmap: b });
                            job.update(json!({"state":"importing","progress":(i+1)as f64/paths.len()as f64}));
                        }
                        let first = first.unwrap();
                        let mut r = layer(
                            if paths.len() > 1 {
                                "Image sequence".into()
                            } else {
                                first.name.clone()
                            },
                            &first,
                            start,
                            p.duration - 1,
                            Point::new(p.width as f32 / 2., p.height as f32 / 2.),
                        );
                        r.sources = sources;
                        r.source_fps = source_fps;
                        r.looping = looping;
                        ids.push(r.id);
                        commands.push(Command::PutRasterLayer { layer: r });
                    }
                    job.check()?;
                    let receipt = service.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands,
                        },
                    })?;
                    Ok(json!({"receipt":receipt,"created_layers":ids,"warnings":warnings}))
                });
                let id = job.id;
                jobs.jobs.insert(id, job);
                Ok(json!({"job":id}))
            }
            ArtworkRequest::Paint {
                project,
                expected_revision,
                layer: id,
                frame,
                points,
                radius,
                color,
                erase,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect before painting"
                );
                ensure!(
                    frame < p.duration
                        && !points.is_empty()
                        && points.len() <= 8192
                        && points.iter().all(|p| p.finite())
                        && radius.is_finite()
                        && (0.5..=256.).contains(&radius),
                    "Invalid bitmap brush stroke"
                );
                let mut r = p
                    .rasters
                    .iter()
                    .find(|r| r.id == id)
                    .context("Raster layer missing")?
                    .clone();
                ensure!(
                    frame >= r.start && frame <= r.end,
                    "Paint inside the layer range"
                );
                let source = p
                    .bitmaps
                    .iter()
                    .find(|b| b.id == r.bitmap_at(frame as f64, p.fps))
                    .context("Bitmap missing")?;
                let bytes = STANDARD.decode(source.png.as_bytes())?;
                let mut image = decode(&bytes)?;
                paint(&mut image, &points, radius, color, erase);
                let b = bitmap(format!("{} · frame {}", r.name, frame + 1), image)?;
                let restore = r
                    .cels
                    .iter()
                    .rev()
                    .find(|c| c.frame <= frame.saturating_add(1))
                    .and_then(|c| c.bitmap);
                let old = r
                    .cels
                    .iter()
                    .find(|c| c.frame == frame)
                    .and_then(|c| c.bitmap);
                r.cels.retain(|c| c.frame != frame);
                r.cels.push(BitmapCel {
                    frame,
                    bitmap: Some(b.id),
                });
                if frame + 1 < p.duration && !r.cels.iter().any(|c| c.frame == frame + 1) {
                    r.cels.push(BitmapCel {
                        frame: frame + 1,
                        bitmap: restore,
                    });
                }
                r.cels.sort_by_key(|c| c.frame);
                let removable = old.filter(|old| {
                    !r.sources.contains(old)
                        && !r.cels.iter().any(|c| c.bitmap == Some(*old))
                        && !p
                            .rasters
                            .iter()
                            .filter(|other| other.id != r.id)
                            .any(|other| {
                                other.sources.contains(old)
                                    || other.cels.iter().any(|c| c.bitmap == Some(*old))
                            })
                });
                let mut commands = vec![
                    Command::AddBitmap { bitmap: b },
                    Command::PutRasterLayer { layer: r },
                ];
                if let Some(old) = removable {
                    commands.push(Command::Remove { id: old });
                }
                self.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision,
                        commands,
                    },
                })
            }
            ArtworkRequest::LipSync {
                engine,
                mapping,
                project,
                expected_revision,
                clip,
                character,
                text,
                threshold,
            } => {
                if engine != crate::speech::Recognizer::Envelope {
                    return crate::speech::recognize(
                        self,
                        project,
                        expected_revision,
                        clip,
                        character,
                        text,
                        engine,
                        mapping,
                    );
                }
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision,
                    "Stale revision: inspect before lip sync"
                );
                let a = p
                    .audio
                    .iter()
                    .find(|a| a.id == clip)
                    .context("Audio clip missing")?;
                ensure!(p.character(character).is_some(), "Character missing");
                let asset = p
                    .assets
                    .iter()
                    .find(|v| v.id == a.asset)
                    .context("Audio asset missing")?;
                let bytes = self.project_workspace(project).read(&asset.path)?;
                let track = lip_sync(&p, a, character, &bytes, &text, threshold.unwrap_or(0.02))?;
                let receipt = self.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision,
                        commands: vec![Command::PutMouthTrack { track }],
                    },
                })?;
                Ok(
                    json!({"receipt":receipt,"warnings":["Audio-envelope/text-guided visemes are editable approximations, not phoneme recognition. Review mouth timing after retiming audio."]}),
                )
            }
            ArtworkRequest::Video {
                project,
                expected_revision,
                path,
                start,
                frames,
                fps,
                width,
                height,
            } => {
                let p = self.snapshot(project)?;
                ensure!(
                    p.revision == expected_revision && start < p.duration,
                    "Stale revision or invalid start"
                );
                ensure!(
                    (1..=240).contains(&frames)
                        && (1..=60).contains(&fps)
                        && width > 0
                        && height > 0
                        && width <= 1920
                        && height <= 1920
                        && width as u64 * height as u64 * frames as u64 <= 134_217_728,
                    "Video extraction exceeds frame/pixel budget"
                );
                let workspace = self.project_workspace(project);
                let input = workspace.path(&path)?;
                ensure!(input.is_file(), "Video missing");
                let service = self.clone();
                let mut jobs = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                jobs.jobs.retain(|_, j| !j.finished());
                ensure!(jobs.jobs.len() < 8, "Too many active jobs");
                let job = crate::export::background(project, p.revision, move |job| {
                    let temp = tempfile::tempdir()?;
                    let mut command = std::process::Command::new(crate::export::ffmpeg());
                    command.args(["-hide_banner","-loglevel","error","-nostdin","-y","-protocol_whitelist","file,pipe","-i"]).arg(input).args(["-vf",&format!("fps={fps},scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black@0"),"-frames:v",&frames.to_string()]).arg(temp.path().join("frame_%06d.png"));
                    crate::export::run_encoder(job, command)?;
                    let mut files = std::fs::read_dir(temp.path())?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    files.sort_by_key(|e| e.file_name());
                    ensure!(!files.is_empty(), "Video produced no frames");
                    let mut sources = vec![];
                    let mut commands = vec![];
                    let mut first = None;
                    for file in files {
                        job.check()?;
                        let b = bitmap(
                            file.file_name().to_string_lossy().into(),
                            decode(&std::fs::read(file.path())?)?,
                        )?;
                        if first.is_none() {
                            first = Some(b.clone());
                        }
                        sources.push(b.id);
                        commands.push(Command::AddBitmap { bitmap: b });
                    }
                    let mut r = layer(
                        "Video frames".into(),
                        &first.unwrap(),
                        start,
                        p.duration - 1,
                        Point::new(p.width as f32 / 2., p.height as f32 / 2.),
                    );
                    r.sources = sources;
                    r.source_fps = fps;
                    let id = r.id;
                    commands.push(Command::PutRasterLayer { layer: r });
                    job.check()?;
                    let receipt = service.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands,
                        },
                    })?;
                    Ok(
                        json!({"receipt":receipt,"created_layers":[id],"warnings":["Video is converted to an editable bounded image sequence. Embedded sound is not imported; add a separate audio track."]}),
                    )
                });
                let id = job.id;
                jobs.jobs.insert(id, job);
                Ok(json!({"job":id}))
            }
        }
    }
}

fn follow_track(
    p: &Project,
    target: Id,
    start: u32,
    end: u32,
    points: &[Value],
    second: Option<&[Value]>,
) -> Result<Vec<Command>> {
    let world = |v: &Value| -> Result<Point> { Ok(serde_json::from_value(v["world"].clone())?) };
    let origin = world(&points[0])?;
    let baseline = second
        .map(|s| world(&s[0]).map(|q| q - origin))
        .transpose()?;
    ensure!(
        baseline.is_none_or(|p| p.distance(Point::default()) >= 1.),
        "Tracking needs two distinct initial positions"
    );
    let transform = |index: usize, position: Point| -> Result<(Point, f32, f32)> {
        let center = world(&points[index])?;
        let mut angle = 0.;
        let mut scale = 1.;
        if let (Some(second), Some(base)) = (second, baseline) {
            let delta = world(&second[index])? - center;
            ensure!(
                delta.distance(Point::default()) >= 1.,
                "Tracking points converged; review a shorter range"
            );
            angle = delta.y.atan2(delta.x) - base.y.atan2(base.x);
            scale = delta.distance(Point::default()) / base.distance(Point::default());
        }
        let local = position - origin;
        Ok((
            center
                + Point::new(
                    (local.x * angle.cos() - local.y * angle.sin()) * scale,
                    (local.x * angle.sin() + local.y * angle.cos()) * scale,
                ),
            angle,
            scale,
        ))
    };
    let frames = (start..=end)
        .chain((start > 0).then_some(start.saturating_sub(1)))
        .chain((end + 1 < p.duration).then_some(end + 1));
    let mut out = vec![];
    if let Some(graphic) = p.graphics.iter().find(|g| g.id == target) {
        ensure!(
            !p.bindings.iter().any(|b| b.graphic == target),
            "Detach bone-bound graphics before applying world-space tracking"
        );
        for frame in frames {
            let mut key = graphic.at(frame as f64);
            key.frame = frame;
            if frame >= start && frame <= end {
                let (position, angle, scale) = transform((frame - start) as usize, key.position)?;
                key.position = position;
                key.rotation += angle;
                key.scale *= scale;
                key.easing = Easing::Linear;
            }
            out.push(Command::SetGraphicKey {
                graphic: target,
                key,
            });
        }
    } else if let Some(original) = p.rasters.iter().find(|r| r.id == target) {
        let mut raster = original.clone();
        for frame in frames {
            let mut key = original.at(frame as f64);
            key.frame = frame;
            if frame >= start && frame <= end {
                let (position, angle, scale) = transform((frame - start) as usize, key.position)?;
                key.position = position;
                key.rotation += angle;
                key.scale = Point::new(key.scale.x * scale, key.scale.y * scale);
                key.easing = Easing::Linear;
            }
            raster.keys.retain(|k| k.frame != frame);
            raster.keys.push(key);
        }
        raster.keys.sort_by_key(|k| k.frame);
        out.push(Command::PutRasterLayer { layer: raster });
    }
    Ok(out)
}

pub fn paint(
    image: &mut image::RgbaImage,
    points: &[Point],
    radius: f32,
    color: Color,
    erase: bool,
) {
    let mut stamp = |q: Point| {
        let x0 = (q.x - radius - 1.).max(0.) as u32;
        let x1 = (q.x + radius + 1.).ceil().min(image.width() as f32) as u32;
        let y0 = (q.y - radius - 1.).max(0.) as u32;
        let y1 = (q.y + radius + 1.).ceil().min(image.height() as f32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let coverage = (radius + 0.5
                    - Point::new(x as f32 + 0.5, y as f32 + 0.5).distance(q))
                .clamp(0., 1.);
                let pixel = image.get_pixel_mut(x, y);
                let sa = color[3] as f32 / 255. * coverage;
                if erase {
                    pixel.0[3] = (pixel.0[3] as f32 * (1. - coverage)).round() as u8;
                } else {
                    let da = pixel.0[3] as f32 / 255.;
                    let alpha = sa + da * (1. - sa);
                    if alpha > 0. {
                        for (c, channel) in color.iter().enumerate().take(3) {
                            pixel.0[c] = ((*channel as f32 * sa
                                + pixel.0[c] as f32 * da * (1. - sa))
                                / alpha)
                                .round() as u8;
                        }
                        pixel.0[3] = (alpha * 255.).round() as u8;
                    }
                }
            }
        }
    };
    stamp(points[0]);
    for pair in points.windows(2) {
        let steps = (pair[0].distance(pair[1]) / (radius * 0.3).max(0.5))
            .ceil()
            .min(32768.) as u32;
        for i in 1..=steps {
            stamp(pair[0].lerp(pair[1], i as f32 / steps as f32));
        }
    }
}
fn lip_sync(
    p: &Project,
    clip: &AudioClip,
    character: Id,
    bytes: &[u8],
    text: &str,
    threshold: f32,
) -> Result<MouthTrack> {
    ensure!(
        threshold.is_finite() && (0.001..=0.5).contains(&threshold) && text.len() <= 4096,
        "Invalid lip-sync input"
    );
    let pcm = astral_audio::decode(bytes)?;
    ensure!(
        clip.duration_seconds <= 600.,
        "Lip sync is limited to ten minutes"
    );
    let samples = pcm.samples;
    let end =
        (clip.start + (clip.duration_seconds * p.fps as f64).ceil() as u32).min(p.duration) - 1;
    let letters: Vec<char> = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect();
    let mut keys = vec![];
    let frames = (end - clip.start + 1).max(1);
    for frame in clip.start..=end {
        let time = (frame - clip.start) as f64 / p.fps as f64;
        let offset = ((clip.offset_seconds + time * clip.speed as f64) * pcm.rate as f64) as usize
            * pcm.channels;
        let count = (pcm.rate as f64 / p.fps as f64 * clip.speed as f64)
            .ceil()
            .max(1.) as usize
            * pcm.channels;
        let mut energy = 0.;
        let mut n = 0;
        for i in offset..offset.saturating_add(count) {
            let index = if clip.looping && !samples.is_empty() {
                i % samples.len()
            } else {
                i
            };
            if let Some(v) = samples.get(index) {
                energy += v * v;
                n += 1;
            }
        }
        let rms = if n > 0 {
            (energy / n as f32).sqrt() * clip.gain
        } else {
            0.
        };
        let shape = if rms < threshold {
            MouthShape::Rest
        } else if letters.is_empty() {
            if rms > threshold * 3. {
                MouthShape::A
            } else {
                MouthShape::O
            }
        } else {
            match letters[((frame - clip.start) as usize * letters.len() / frames as usize)
                .min(letters.len() - 1)]
            {
                'a' | 'h' => MouthShape::A,
                'e' | 'i' | 'y' => MouthShape::E,
                'o' | 'u' | 'w' => MouthShape::O,
                'f' | 'v' => MouthShape::F,
                'm' | 'b' | 'p' => MouthShape::M,
                _ => MouthShape::E,
            }
        };
        if keys.last().is_none_or(|k: &MouthKey| k.shape != shape) {
            keys.push(MouthKey { frame, shape });
        }
    }
    Ok(MouthTrack {
        sprites: Default::default(),
        id: Id::new_v4(),
        name: "Audio mouth cues".into(),
        character,
        start: clip.start,
        end,
        size: Point::new(12., 10.),
        offset: Point::new(0., 5.),
        color: [245, 240, 235, 255],
        keys,
    })
}

fn track(
    p: &Project,
    r: &RasterLayer,
    start: u32,
    end: u32,
    point: Point,
    job: &crate::export::Job,
) -> Result<(Vec<Value>, Vec<Command>)> {
    let source = p
        .bitmaps
        .iter()
        .find(|b| b.id == r.bitmap_at(start as f64, p.fps))
        .context("Bitmap missing")?;
    let first = astral_render::decode_bitmap(source)?;
    let mut x = point.x.round() as i32;
    let mut y = point.y.round() as i32;
    ensure!(
        x >= 6 && y >= 6 && x + 6 < first.width as i32 && y + 6 < first.height as i32,
        "Choose a textured point at least six pixels inside the source image"
    );
    let patch: Vec<[f32; 4]> = (-6..=6)
        .step_by(2)
        .flat_map(|dy| (-6..=6).step_by(2).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| {
            let offset = (((y + dy) as u32 * first.width + (x + dx) as u32) * 4) as usize;
            std::array::from_fn(|c| first.pixels[offset + c] as f32)
        })
        .collect();
    let means: [f32; 4] =
        std::array::from_fn(|c| patch.iter().map(|p| p[c]).sum::<f32>() / patch.len() as f32);
    let variance = patch
        .iter()
        .map(|p| {
            p.iter()
                .zip(means)
                .map(|(value, mean)| (*value - mean).powi(2))
                .sum::<f32>()
        })
        .sum::<f32>()
        / patch.len() as f32;
    ensure!(
        variance > 40.,
        "Choose a distinctive edge or texture; this patch is too uniform to track"
    );
    let mut points = vec![];
    let mut commands = vec![];
    for frame in start..=end {
        job.check()?;
        let source = p
            .bitmaps
            .iter()
            .find(|b| b.id == r.bitmap_at(frame as f64, p.fps))
            .context("Bitmap missing")?;
        let image = astral_render::decode_bitmap(source)?;
        ensure!(
            image.width == first.width && image.height == first.height,
            "Tracking requires equal source dimensions"
        );
        let mut best = (f32::INFINITY, x, y);
        for cy in (y - 24).max(6)..=(y + 24).min(image.height as i32 - 7) {
            for cx in (x - 24).max(6)..=(x + 24).min(image.width as i32 - 7) {
                let mut score = 0.;
                let mut index = 0;
                for dy in (-6..=6).step_by(2) {
                    for dx in (-6..=6).step_by(2) {
                        let offset =
                            (((cy + dy) as u32 * image.width + (cx + dx) as u32) * 4) as usize;
                        for (c, channel) in patch[index].iter().enumerate() {
                            score += (image.pixels[offset + c] as f32 - *channel).powi(2);
                        }
                        index += 1;
                    }
                }
                score /= patch.len() as f32 * 4.;
                if score < best.0
                    || (score == best.0
                        && (cx - x).pow(2) + (cy - y).pow(2)
                            < (best.1 - x).pow(2) + (best.2 - y).pow(2))
                {
                    best = (score, cx, cy);
                }
            }
        }
        ensure!(
            best.0 < 6400.,
            "Tracking lost the template at frame {}; no camera keys were committed",
            frame + 1
        );
        x = best.1;
        y = best.2;
        let uv = Point::new(
            (x as f32 + 0.5) / image.width as f32,
            (y as f32 + 0.5) / image.height as f32,
        );
        let scene = evaluate(p, frame as f64);
        let center = r.resolve_correctives(&scene).world_point(uv, frame as f64);
        let zoom = scene.zoom;
        points.push(json!({"frame":frame,"source":Point::new(x as f32,y as f32),"world":center,"rms_error":best.0.sqrt()}));
        commands.push(Command::SetCamera {
            key: CameraKey {
                frame,
                center,
                zoom,
            },
        });
        job.update(
            json!({"state":"tracking","progress":(frame-start+1)as f64/(end-start+1)as f64}),
        );
    }
    Ok((points, commands))
}

#[cfg(test)]
mod tracking_tests {
    use super::*;
    #[test]
    fn tracking_does_not_accumulate_offsets_and_preserves_adjacent_frames() {
        let mut p = Project::new("Track", 64, 64, 24, 24);
        p.schema = 8;
        let bitmap = Bitmap {
            id: Id::new_v4(),
            name: "Source".into(),
            width: 1,
            height: 1,
            png: "AA==".into(),
        };
        let r = layer("Target".into(), &bitmap, 0, 23, Point::new(10., 10.));
        let id = r.id;
        p.bitmaps.push(bitmap);
        p.rasters.push(r);
        let points = vec![
            json!({"world":{"x":0,"y":0}}),
            json!({"world":{"x":10,"y":0}}),
            json!({"world":{"x":20,"y":0}}),
        ];
        let commands = follow_track(&p, id, 3, 5, &points, None).unwrap();
        let mut s = Session::new(p).unwrap();
        s.apply(Batch {
            project: s.project.id,
            expected_revision: 0,
            commands,
        })
        .unwrap();
        let r = &s.project.rasters[0];
        assert_eq!(r.at(3.).position, Point::new(10., 10.));
        assert_eq!(r.at(4.).position, Point::new(20., 10.));
        assert_eq!(r.at(5.).position, Point::new(30., 10.));
        assert_eq!(r.at(2.).position, Point::new(10., 10.));
        assert_eq!(r.at(6.).position, Point::new(10., 10.));
        s.undo(s.project.id, 1).unwrap();
        assert_eq!(s.project.rasters[0].keys.len(), 1);
    }
}
