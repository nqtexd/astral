//! One renderer-independent display list for GPU UI and deterministic exports.
use anyhow::{Context, Result, ensure};
use astral_core::*;
mod presentation;
mod raster_images;
mod text;
pub use raster_images::{
    DecodedBitmap, TextureVertex, decode_bitmap, paint_image, validate_artwork,
};

#[derive(Clone, Debug)]
pub enum Primitive {
    Image {
        bitmap: std::sync::Arc<DecodedBitmap>,
        vertices: Vec<TextureVertex>,
        indices: Vec<u32>,
        opacity: f32,
    },
    Path {
        contours: Vec<Vec<Point>>,
        color: Color,
    },
    Line {
        a: Point,
        b: Point,
        width: f32,
        color: Color,
    },
    Circle {
        center: Point,
        radius: f32,
        color: Color,
    },
    Polyline {
        points: Vec<Point>,
        width: f32,
        color: Color,
    },
}
pub struct DisplayList {
    pub width: u32,
    pub height: u32,
    pub background: Color,
    pub primitives: Vec<Primitive>,
}

pub fn display_list(project: &Project, frame: f64) -> DisplayList {
    let scene = evaluate(project, frame);
    let mut out = DisplayList {
        width: project.width,
        height: project.height,
        background: project.background,
        primitives: vec![],
    };
    let transform = |p: Point| {
        Point::new(
            (p.x - scene.center.x) * scene.zoom + project.width as f32 / 2.,
            (p.y - scene.center.y) * scene.zoom + project.height as f32 / 2.,
        )
    };
    raster_images::append(&mut out, project, &scene, frame, GraphicLayer::Background);
    presentation::append_graphics(&mut out, project, &scene, frame, GraphicLayer::Background);
    for d in &project.drawings {
        let learned = project
            .drawing_motions
            .iter()
            .any(|m| m.sources.contains(&d.id) && frame >= m.start as f64 && frame <= m.end as f64);
        if !learned && frame >= d.start as f64 && frame <= d.end as f64 {
            out.primitives.push(Primitive::Polyline {
                points: d.points.iter().map(|p| transform(*p)).collect(),
                width: d.width * scene.zoom,
                color: d.color,
            });
        }
    }
    for m in &project.drawing_motions {
        if frame >= m.start as f64 && frame <= m.end as f64 {
            out.primitives.push(Primitive::Polyline {
                points: sample_drawing(project, m, frame)
                    .iter()
                    .map(|q| transform(*q))
                    .collect(),
                width: m.width * scene.zoom,
                color: m.color,
            });
        }
    }
    for layer in p_layers(project) {
        if let Some(cel) = project.cel_at(layer.id, frame.floor().max(0.) as u32) {
            for stroke in &cel.strokes {
                let mut color = stroke.color;
                color[3] = (color[3] as f32 * layer.opacity).round() as u8;
                out.primitives.push(Primitive::Polyline {
                    points: stroke.points.iter().map(|p| transform(*p)).collect(),
                    width: stroke.width * scene.zoom,
                    color,
                });
            }
        }
    }
    for c in &scene.characters {
        for (i, parent) in c.parents.iter().enumerate() {
            if let Some(p) = parent {
                out.primitives.push(Primitive::Line {
                    a: transform(c.pose.root + c.pose.joints[*p]),
                    b: transform(c.pose.root + c.pose.joints[i]),
                    width: c.thickness * scene.zoom,
                    color: c.color,
                });
            }
        }
        if let Some(head) = project
            .character(c.id)
            .and_then(|v| v.joints.iter().position(|j| j.name == "head"))
        {
            out.primitives.push(Primitive::Circle {
                center: transform(c.pose.root + c.pose.joints[head]),
                radius: c.thickness * 1.8 * scene.zoom,
                color: c.color,
            });
        }
    }
    for e in &project.effects {
        if frame < e.start as f64 || frame > e.end as f64 || e.kind == EffectKind::CameraShake {
            continue;
        }
        let t = ((frame - e.start as f64) / (e.end - e.start + 1) as f64) as f32;
        let position = e
            .character
            .and_then(|id| scene.characters.iter().find(|c| c.id == id))
            .map(|c| {
                c.pose.root
                    + e.joint
                        .and_then(|j| c.pose.joints.get(j).copied())
                        .unwrap_or_default()
                    + e.position
            })
            .unwrap_or(e.position);
        let center = transform(position);
        let mut color = e.color;
        color[3] = (color[3] as f32 * (1. - t)) as u8;
        match e.kind {
            EffectKind::Impact => {
                for i in 0..12 {
                    let angle = i as f32 * std::f32::consts::TAU / 12.;
                    let r = (15. + t * 80.) * e.intensity * scene.zoom;
                    out.primitives.push(Primitive::Line {
                        a: Point::new(
                            center.x + angle.cos() * r * 0.4,
                            center.y + angle.sin() * r * 0.4,
                        ),
                        b: Point::new(center.x + angle.cos() * r, center.y + angle.sin() * r),
                        width: (1. - t) * 5. * scene.zoom,
                        color,
                    });
                }
            }
            EffectKind::Particles | EffectKind::Sparks | EffectKind::Smoke => {
                for i in 0..24 {
                    let r = random(e.seed + i) * std::f32::consts::TAU;
                    let speed = 20. + random(e.seed + i + 41) * 100.;
                    let d = t * speed * e.intensity * scene.zoom;
                    let gravity = if e.kind == EffectKind::Smoke { -1. } else { 1. };
                    out.primitives.push(Primitive::Circle {
                        center: Point::new(
                            center.x + r.cos() * d,
                            center.y + r.sin() * d + gravity * t * t * 45. * scene.zoom,
                        ),
                        radius: if e.kind == EffectKind::Smoke {
                            (8. + t * 18.) * scene.zoom
                        } else {
                            2. * scene.zoom
                        },
                        color,
                    });
                }
            }
            EffectKind::Trail => {
                if let Some(id) = e.character {
                    for age in 1..=8 {
                        let previous = evaluate(project, (frame - age as f64 * 1.5).max(0.));
                        if let Some(c) = previous.characters.iter().find(|c| c.id == id) {
                            let mut ghost = color;
                            ghost[3] = ((9 - age) as f32 * 12. * (1. - t)) as u8;
                            for (i, p) in c.parents.iter().enumerate() {
                                if let Some(p) = p {
                                    out.primitives.push(Primitive::Line {
                                        a: transform(c.pose.root + c.pose.joints[*p]),
                                        b: transform(c.pose.root + c.pose.joints[i]),
                                        width: c.thickness * scene.zoom,
                                        color: ghost,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            // Soft radial overlays are currently stylized presets, not legacy filter parity.
            EffectKind::Glow | EffectKind::Blur => {
                for i in (1..=12).rev() {
                    let mut c = color;
                    c[3] = (color[3] as f32 / 24.) as u8;
                    out.primitives.push(Primitive::Circle {
                        center,
                        radius: i as f32 * 5. * e.intensity * scene.zoom,
                        color: c,
                    });
                }
            }
            EffectKind::Flash => {
                let mut flash = color;
                flash[3] = (color[3] as f32 * e.intensity.min(1.) * (1. - t)) as u8;
                out.primitives.push(Primitive::Path {
                    contours: vec![vec![
                        Point::new(0., 0.),
                        Point::new(project.width as f32, 0.),
                        Point::new(project.width as f32, project.height as f32),
                        Point::new(0., project.height as f32),
                    ]],
                    color: flash,
                });
            }
            EffectKind::Shockwave => {
                let radius = (8. + t * 140.) * e.intensity * scene.zoom;
                out.primitives.push(Primitive::Polyline {
                    points: (0..=48)
                        .map(|i| {
                            let angle = i as f32 * std::f32::consts::TAU / 48.;
                            Point::new(
                                center.x + angle.cos() * radius,
                                center.y + angle.sin() * radius * 0.7,
                            )
                        })
                        .collect(),
                    width: (1. - t) * 5. * scene.zoom,
                    color,
                });
            }
            EffectKind::SpeedLines => {
                for i in 0..14 {
                    let angle = random(e.seed + i) * std::f32::consts::TAU;
                    let near = (30. + t * 60.) * e.intensity * scene.zoom;
                    let far = near + (40. + random(e.seed + i + 37) * 80.) * scene.zoom;
                    out.primitives.push(Primitive::Line {
                        a: Point::new(center.x + angle.cos() * near, center.y + angle.sin() * near),
                        b: Point::new(center.x + angle.cos() * far, center.y + angle.sin() * far),
                        width: (1. - t) * 2. * scene.zoom,
                        color,
                    });
                }
            }
            EffectKind::Dust => {
                for i in 0..18 {
                    let side = if i % 2 == 0 { 1. } else { -1. };
                    let spread = t * (35. + random(e.seed + i) * 80.) * e.intensity * scene.zoom;
                    out.primitives.push(Primitive::Circle {
                        center: Point::new(
                            center.x + side * spread,
                            center.y - spread * 0.25 + random(e.seed + i + 18) * 15. * scene.zoom,
                        ),
                        radius: (3. + t * 12.) * (0.5 + random(e.seed + i + 40)) * scene.zoom,
                        color,
                    });
                }
            }
            EffectKind::CameraShake => {}
        }
    }
    presentation::append_graphics(&mut out, project, &scene, frame, GraphicLayer::Foreground);
    raster_images::append(&mut out, project, &scene, frame, GraphicLayer::Foreground);
    raster_images::mouths(&mut out, project, &scene, frame);
    presentation::append_dialogues(&mut out, project, &scene, frame);
    out
}
fn p_layers(project: &Project) -> impl Iterator<Item = &DrawingLayer> {
    project
        .layers
        .iter()
        .filter(|l| l.visible && l.opacity > 0.)
}
fn random(mut x: u64) -> f32 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((x ^ (x >> 31)) >> 40) as f32 / 16777215.
}

pub fn rasterize(list: &DisplayList, width: u32, height: u32) -> Result<tiny_skia::Pixmap> {
    rasterize_with_framing(list, width, height, None)
}

/// Draw vectors directly at delivery resolution; clip the original canvas before letterboxing.
pub fn rasterize_framed(
    list: &DisplayList,
    width: u32,
    height: u32,
    fill: bool,
) -> Result<tiny_skia::Pixmap> {
    rasterize_with_framing(list, width, height, Some(fill))
}

fn rasterize_with_framing(
    list: &DisplayList,
    width: u32,
    height: u32,
    framing: Option<bool>,
) -> Result<tiny_skia::Pixmap> {
    ensure!(
        width > 0
            && height > 0
            && width <= 8192
            && height <= 8192
            && (width as u64 * height as u64) <= 33_554_432,
        "Output exceeds 32-megapixel render budget"
    );
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).context("Cannot allocate output surface")?;
    let color = |c: Color| tiny_skia::Color::from_rgba8(c[0], c[1], c[2], c[3]);
    let mut scale = tiny_skia::Transform::from_scale(
        width as f32 / list.width as f32,
        height as f32 / list.height as f32,
    );
    let mut mask = None;
    if let Some(fill) = framing {
        let sx = width as f32 / list.width as f32;
        let sy = height as f32 / list.height as f32;
        let factor = if fill { sx.max(sy) } else { sx.min(sy) };
        let x = (width as f32 - list.width as f32 * factor) / 2.;
        let y = (height as f32 - list.height as f32 * factor) / 2.;
        scale = tiny_skia::Transform::from_row(factor, 0., 0., factor, x, y);
        // No mask is needed when the source covers the entire output. Applying
        // a full-resolution mask to every stick/text primitive wasted work.
        let covers_output = fill || (x.abs() < 0.01 && y.abs() < 0.01);
        if covers_output {
            pixmap.fill(color(list.background));
        } else {
            pixmap.fill(tiny_skia::Color::BLACK);
            let rect = tiny_skia::Rect::from_xywh(0., 0., list.width as f32, list.height as f32)
                .context("Invalid source canvas")?;
            let path = tiny_skia::PathBuilder::from_rect(rect);
            let mut clip =
                tiny_skia::Mask::new(width, height).context("Cannot allocate canvas clip")?;
            clip.fill_path(&path, tiny_skia::FillRule::Winding, true, scale);
            let background = tiny_skia::Paint {
                shader: tiny_skia::Shader::SolidColor(color(list.background)),
                blend_mode: tiny_skia::BlendMode::Source,
                ..Default::default()
            };
            pixmap.fill_path(
                &path,
                &background,
                tiny_skia::FillRule::Winding,
                scale,
                None,
            );
            mask = Some(clip);
        }
    } else {
        pixmap.fill(color(list.background));
    }
    for primitive in &list.primitives {
        let mut paint = tiny_skia::Paint {
            anti_alias: true,
            ..Default::default()
        };
        match primitive {
            Primitive::Image {
                bitmap,
                vertices,
                indices,
                opacity,
            } => raster_images::draw(
                &mut pixmap,
                bitmap,
                vertices,
                indices,
                *opacity,
                scale,
                mask.as_ref(),
            ),
            Primitive::Path { contours, color: c } => {
                paint.set_color(color(*c));
                let mut builder = tiny_skia::PathBuilder::new();
                for contour in contours {
                    if let Some(first) = contour.first() {
                        builder.move_to(first.x, first.y);
                        for p in &contour[1..] {
                            builder.line_to(p.x, p.y)
                        }
                        builder.close();
                    }
                }
                if let Some(path) = builder.finish() {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        scale,
                        mask.as_ref(),
                    );
                }
            }
            Primitive::Circle {
                center,
                radius,
                color: c,
            } => {
                paint.set_color(color(*c));
                if let Some(path) = tiny_skia::PathBuilder::from_circle(center.x, center.y, *radius)
                {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        scale,
                        mask.as_ref(),
                    );
                }
            }
            Primitive::Line {
                a,
                b,
                width,
                color: c,
            } => {
                paint.set_color(color(*c));
                let mut path = tiny_skia::PathBuilder::new();
                path.move_to(a.x, a.y);
                path.line_to(b.x, b.y);
                if let Some(path) = path.finish() {
                    let stroke = tiny_skia::Stroke {
                        width: *width,
                        line_cap: tiny_skia::LineCap::Round,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&path, &paint, &stroke, scale, mask.as_ref());
                }
            }
            Primitive::Polyline {
                points,
                width,
                color: c,
            } => {
                paint.set_color(color(*c));
                if let Some(first) = points.first() {
                    let mut path = tiny_skia::PathBuilder::new();
                    path.move_to(first.x, first.y);
                    for p in &points[1..] {
                        path.line_to(p.x, p.y)
                    }
                    if let Some(path) = path.finish() {
                        let stroke = tiny_skia::Stroke {
                            width: *width,
                            line_cap: tiny_skia::LineCap::Round,
                            line_join: tiny_skia::LineJoin::Round,
                            ..Default::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, scale, mask.as_ref());
                    }
                }
            }
        }
    }
    Ok(pixmap)
}
/// Triangulate the same filled contours for the GPU viewport, preserving holes.
pub fn tessellate(contours: &[Vec<Point>]) -> Result<(Vec<Point>, Vec<u32>)> {
    use lyon_tessellation::{
        BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
        math::point, path::Path,
    };
    let mut builder = Path::builder();
    for contour in contours {
        if let Some(first) = contour.first() {
            builder.begin(point(first.x, first.y));
            for p in &contour[1..] {
                builder.line_to(point(p.x, p.y));
            }
            builder.close();
        }
    }
    let path = builder.build();
    let mut buffers: VertexBuffers<Point, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &path,
            &FillOptions::default().with_fill_rule(FillRule::NonZero),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| {
                let p = v.position();
                Point::new(p.x, p.y)
            }),
        )
        .map_err(|e| anyhow::anyhow!("Vector tessellation: {e:?}"))?;
    Ok((buffers.vertices, buffers.indices))
}
pub fn png(project: &Project, frame: f64, width: u32, height: u32) -> Result<Vec<u8>> {
    project.validate()?;
    validate_artwork(project)?;
    ensure!(
        frame.is_finite() && frame >= 0. && frame < project.duration as f64,
        "Frame must be inside the project"
    );
    Ok(rasterize(&display_list(project, frame), width, height)?.encode_png()?)
}
/// Convert premultiplied pixels to straight RGBA for encoders.
pub fn rgba(project: &Project, frame: f64) -> Result<Vec<u8>> {
    let pixmap = rasterize(&display_list(project, frame), project.width, project.height)?;
    Ok(straight_rgba(&pixmap))
}
pub fn rgba_framed(
    project: &Project,
    frame: f64,
    width: u32,
    height: u32,
    fill: bool,
) -> Result<Vec<u8>> {
    let pixmap = rasterize_framed(&display_list(project, frame), width, height, fill)?;
    Ok(straight_rgba(&pixmap))
}
fn straight_rgba(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixmap.data().len());
    for p in pixmap.pixels() {
        let c = p.demultiply();
        bytes.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framed_delivery_clips_source_and_keeps_vector_detail() {
        let list = DisplayList {
            width: 100,
            height: 100,
            background: [255, 255, 255, 255],
            primitives: vec![Primitive::Circle {
                center: Point::new(150., 50.),
                radius: 40.,
                color: [255, 0, 0, 255],
            }],
        };
        let fit = rasterize_framed(&list, 300, 100, false).unwrap();
        assert_eq!(fit.pixel(150, 50).unwrap().demultiply().red(), 255);
        assert_eq!(
            fit.pixel(250, 50).unwrap().demultiply().red(),
            0,
            "Off-canvas artwork must not leak into letterbox bars"
        );
        let fill = rasterize_framed(&list, 300, 100, true).unwrap();
        assert_eq!(fill.pixel(10, 10).unwrap().demultiply().red(), 255);
        let detail = DisplayList {
            width: 10,
            height: 10,
            background: [0, 0, 0, 255],
            primitives: vec![Primitive::Circle {
                center: Point::new(3.5, 3.5),
                radius: 0.15,
                color: [255, 255, 255, 255],
            }],
        };
        let delivery = rasterize_framed(&detail, 1000, 1000, false).unwrap();
        assert_eq!(
            delivery.pixel(350, 350).unwrap().demultiply().red(),
            255,
            "Subpixel source details must be rasterized at delivery resolution"
        );
        assert_eq!(delivery.pixel(370, 350).unwrap().demultiply().red(), 0);
        assert!(rasterize_framed(&detail, 0, 100, false).is_err());
    }
    #[test]
    fn new_impact_effects_are_visible_and_seeded() {
        let mut p = Project::new("Impact", 320, 180, 24, 48);
        p.schema = 5;
        let plain = png(&p, 4., 320, 180).unwrap();
        for kind in [
            EffectKind::Flash,
            EffectKind::SpeedLines,
            EffectKind::Dust,
            EffectKind::Shockwave,
        ] {
            p.effects = vec![Effect {
                id: Id::new_v4(),
                name: "Accent".into(),
                kind,
                start: 0,
                end: 12,
                position: Point::new(160., 90.),
                character: None,
                joint: None,
                color: [70, 110, 160, 220],
                intensity: 1.,
                seed: 77,
            }];
            p.validate().unwrap();
            let frame = png(&p, 4., 320, 180).unwrap();
            assert_ne!(frame, plain, "Invisible effect {kind:?}");
            assert_eq!(frame, png(&p, 4., 320, 180).unwrap());
        }
    }
    #[test]
    fn seeded_preview_is_reproducible() {
        let p = fight_demo().unwrap();
        let a = png(&p, 104., 480, 270).unwrap();
        assert_eq!(a, png(&p, 104., 480, 270).unwrap());
        assert_ne!(a, png(&p, 0., 480, 270).unwrap());
        assert!(png(&p, 0., 9000, 10).is_err());
    }
}

#[cfg(feature = "gpu")]
pub mod gpu;

mod text_review;
pub use text_review::{TextIssue, fit_and_place, review_text};
