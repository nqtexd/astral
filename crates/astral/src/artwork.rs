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
    Track {
        project: Id,
        expected_revision: u64,
        layer: Id,
        start: u32,
        end: u32,
        point: Point,
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
    let image = reader.decode()?.into_rgba8();
    ensure!(
        image.width() as u64 * image.height() as u64 <= 16_777_216,
        "Bitmap exceeds 16 megapixels"
    );
    Ok(image)
}
fn layer(name: String, source: &Bitmap, start: u32, end: u32, position: Point) -> RasterLayer {
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
            ArtworkRequest::Track {
                project,
                expected_revision,
                layer,
                start,
                end,
                point,
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
                let mut jobs = self
                    .store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
                jobs.jobs.retain(|_, j| !j.finished());
                ensure!(jobs.jobs.len() < 8, "Too many active jobs");
                let job = crate::export::background(project, p.revision, move |job| {
                    let result = track(&p, &r, start, end, point, job)?;
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
                        let (parts, notes) = import_psd(&bytes, start, p.duration - 1)?;
                        warnings.extend(notes);
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
                        for (bitmap, raster) in parts {
                            ids.push(raster.id);
                            commands.push(Command::AddBitmap { bitmap });
                            commands.push(Command::PutRasterLayer { layer: raster });
                        }
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
                project,
                expected_revision,
                clip,
                character,
                text,
                threshold,
            } => {
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
type ImportedArtwork = (Vec<(Bitmap, RasterLayer)>, Vec<String>);
fn import_psd(bytes: &[u8], start: u32, end: u32) -> Result<ImportedArtwork> {
    ensure!(
        bytes.len() >= 26
            && &bytes[..4] == b"8BPS"
            && u16::from_be_bytes([bytes[4], bytes[5]]) == 1
            && u16::from_be_bytes([bytes[22], bytes[23]]) == 8
            && u16::from_be_bytes([bytes[24], bytes[25]]) == 3,
        "Import supports 8-bit RGB PSD files; PSB, CMYK and high-bit-depth PSD are unsupported"
    );
    let height = u32::from_be_bytes(bytes[14..18].try_into()?);
    let width = u32::from_be_bytes(bytes[18..22].try_into()?);
    ensure!(
        width > 0 && height > 0 && width as u64 * height as u64 <= 16_777_216,
        "PSD exceeds 16 megapixels"
    );
    std::panic::catch_unwind(||->Result<_>{let document=psd::Psd::from_bytes(bytes).map_err(|e|anyhow::anyhow!("PSD: {e:?}"))?;ensure!(document.layers().len()<=64,"PSD exceeds 64 layers");let mut out=vec![];let mut pixels=0u64;let mut warnings=vec!["PSD layers are rasterized. Text, smart objects, masks, adjustment layers, groups and non-normal blend modes are not editable equivalents; inspect the imported composition.".into()];
        for source in document.layers().iter().rev(){let x=source.layer_left().max(0)as u32;let y=source.layer_top().max(0)as u32;let right=(source.layer_right()+1).clamp(0,width as i32)as u32;let bottom=(source.layer_bottom()+1).clamp(0,height as i32)as u32;if x>=right||y>=bottom{warnings.push(format!("Skipped empty/off-canvas layer {}",source.name()));continue;}pixels+=(right-x)as u64*(bottom-y)as u64;ensure!(pixels<=134_217_728,"PSD layers exceed bitmap budget");let data=source.rgba();let canvas=image::RgbaImage::from_raw(width,height,data).context("Invalid PSD layer pixels")?;let cropped=image::imageops::crop_imm(&canvas,x,y,right-x,bottom-y).to_image();let b=bitmap(source.name().into(),cropped)?;let mut r=layer(source.name().into(),&b,start,end,Point::new(x as f32+(right-x)as f32/2.,y as f32+(bottom-y)as f32/2.));r.visible=source.visible();r.keys[0].opacity=source.opacity()as f32/255.;out.push((b,r));}
        if out.is_empty(){let b=bitmap("PSD composite".into(),image::RgbaImage::from_raw(width,height,document.rgba()).context("Invalid PSD composite")?)?;let r=layer("PSD composite".into(),&b,start,end,Point::new(width as f32/2.,height as f32/2.));out.push((b,r));warnings.push("No editable raster layers found; imported the composite only".into());}Ok((out,warnings))
    }).map_err(|_|anyhow::anyhow!("PSD decoder rejected unsupported compression or malformed layer data; the original file was not changed"))?
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
    let mut reader = hound::WavReader::new(Cursor::new(bytes))?;
    let spec = reader.spec();
    ensure!(
        (1..=2).contains(&spec.channels) && spec.sample_rate > 0 && clip.duration_seconds <= 600.,
        "Lip sync requires bounded mono/stereo WAV"
    );
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 2f32.powi(spec.bits_per_sample as i32 - 1);
            reader
                .samples::<i32>()
                .map(|v| v.map(|v| v as f32 / scale))
                .collect::<std::result::Result<_, _>>()?
        }
    };
    ensure!(samples.iter().all(|v| v.is_finite()), "Non-finite audio");
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
        let offset = ((clip.offset_seconds + time) * spec.sample_rate as f64) as usize
            * spec.channels as usize;
        let count = (spec.sample_rate as usize / p.fps as usize).max(1) * spec.channels as usize;
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
    let mean = patch.iter().map(|p| p[0] + p[1] + p[2]).sum::<f32>() / patch.len() as f32;
    let variance = patch
        .iter()
        .map(|p| (p[0] + p[1] + p[2] - mean).powi(2))
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
        let center = r.world_point(uv, frame as f64);
        let zoom = evaluate(p, frame as f64).zoom;
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
