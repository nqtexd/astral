use crate::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
#[derive(Clone, Debug, PartialEq)]
pub struct TextureVertex {
    pub position: Point,
    pub uv: Point,
}
#[derive(Debug, PartialEq, Eq)]
pub struct DecodedBitmap {
    pub id: Id,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
type CacheKey = (Id, usize);
#[derive(Default)]
struct Cache {
    entries: HashMap<CacheKey, (Arc<str>, Arc<DecodedBitmap>)>,
    bytes: usize,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
pub fn decode_bitmap(bitmap: &Bitmap) -> Result<Arc<DecodedBitmap>> {
    let key = (bitmap.id, Arc::as_ptr(&bitmap.png) as *const u8 as usize);
    let cache = CACHE.get_or_init(Default::default);
    if let Some((_, image)) = cache
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .entries
        .get(&key)
    {
        return Ok(image.clone());
    }
    let bytes = STANDARD
        .decode(bitmap.png.as_bytes())
        .context("Invalid embedded PNG")?;
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
    let (width, height) = reader.into_dimensions()?;
    ensure!(
        width == bitmap.width
            && height == bitmap.height
            && width as u64 * height as u64 <= 16_777_216,
        "Bitmap header does not match its bounded dimensions"
    );
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut pixels = reader.decode()?.into_rgba8().into_raw();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        for channel in 0..3 {
            pixel[channel] = ((pixel[channel] as u16 * pixel[3] as u16 + 127) / 255) as u8;
        }
    }
    let image = Arc::new(DecodedBitmap {
        id: bitmap.id,
        width,
        height,
        pixels,
    });
    let bytes = image.pixels.len() + bitmap.png.len();
    let mut cache = cache.lock().unwrap_or_else(|p| p.into_inner());
    if cache.bytes + bytes > 128 * 1024 * 1024 {
        cache.entries.clear();
        cache.bytes = 0;
    }
    if bytes <= 128 * 1024 * 1024 {
        cache.bytes += bytes;
        cache
            .entries
            .insert(key, (bitmap.png.clone(), image.clone()));
    }
    Ok(image)
}
pub fn validate_artwork(p: &Project) -> Result<()> {
    for bitmap in &p.bitmaps {
        decode_bitmap(bitmap)?;
    }
    Ok(())
}
pub(crate) fn append(
    list: &mut DisplayList,
    p: &Project,
    s: &EvaluatedScene,
    frame: f64,
    layer: GraphicLayer,
) {
    for r in p.rasters.iter().filter(|r| {
        r.visible && r.layer == layer && frame >= r.start as f64 && frame <= r.end as f64
    }) {
        let Some(source) = p.bitmaps.iter().find(|b| b.id == r.bitmap_at(frame, p.fps)) else {
            continue;
        };
        let Ok(bitmap) = decode_bitmap(source) else {
            continue;
        };
        let resolved = r.resolve_correctives(s);
        let r = &resolved;
        let key = r.at(frame);
        let (columns, rows) = r.mesh.as_ref().map_or((1, 1), |m| (m.columns, m.rows));
        let mut vertices = vec![];
        let mut indices = vec![];
        for y in 0..=rows {
            for x in 0..=columns {
                let uv = Point::new(x as f32 / columns as f32, y as f32 / rows as f32);
                let q = r.world_point(uv, frame);
                vertices.push(TextureVertex {
                    position: Point::new(
                        (q.x - s.center.x) * s.zoom + p.width as f32 / 2.,
                        (q.y - s.center.y) * s.zoom + p.height as f32 / 2.,
                    ),
                    uv,
                });
            }
        }
        for y in 0..rows {
            for x in 0..columns {
                let i = y * (columns + 1) + x;
                indices.extend_from_slice(&[
                    i,
                    i + 1,
                    i + columns + 2,
                    i,
                    i + columns + 2,
                    i + columns + 1,
                ]);
            }
        }
        list.primitives.push(Primitive::Image {
            bitmap,
            vertices,
            indices,
            opacity: key.opacity,
        });
    }
}
pub(crate) fn mouths(list: &mut DisplayList, p: &Project, s: &EvaluatedScene, frame: f64) {
    for m in p
        .mouths
        .iter()
        .filter(|m| frame >= m.start as f64 && frame <= m.end as f64)
    {
        let Some(c) = s.characters.iter().find(|c| c.id == m.character) else {
            continue;
        };
        let def = p.character(c.id).unwrap();
        let Some(head) = def.joints.iter().position(|j| j.name == "head") else {
            continue;
        };
        let q = c.pose.root + c.pose.joints[head] + m.offset;
        let center = Point::new(
            (q.x - s.center.x) * s.zoom + p.width as f32 / 2.,
            (q.y - s.center.y) * s.zoom + p.height as f32 / 2.,
        );
        let shape = m
            .keys
            .iter()
            .rev()
            .find(|k| k.frame as f64 <= frame)
            .map_or(MouthShape::Rest, |k| k.shape);
        let (rx, ry) = match shape {
            MouthShape::Rest | MouthShape::M => (m.size.x * 0.4, 0.7),
            MouthShape::A => (m.size.x * 0.45, m.size.y * 0.5),
            MouthShape::E => (m.size.x * 0.5, m.size.y * 0.2),
            MouthShape::O => (m.size.x * 0.25, m.size.y * 0.45),
            MouthShape::F => (m.size.x * 0.4, m.size.y * 0.12),
        };
        let contour = (0..24)
            .map(|i| {
                let a = i as f32 / 24. * std::f32::consts::TAU;
                center + Point::new(a.cos() * rx * s.zoom, a.sin() * ry * s.zoom)
            })
            .collect();
        list.primitives.push(Primitive::Path {
            contours: vec![contour],
            color: m.color,
        });
    }
}
fn edge(a: Point, b: Point, p: Point) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}
fn top_left(a: Point, b: Point) -> bool {
    b.y < a.y || (b.y == a.y && b.x > a.x)
}
pub(crate) fn draw(
    pixmap: &mut tiny_skia::Pixmap,
    bitmap: &DecodedBitmap,
    vertices: &[TextureVertex],
    indices: &[u32],
    opacity: f32,
    transform: tiny_skia::Transform,
    mask: Option<&tiny_skia::Mask>,
) {
    let width = pixmap.width();
    let height = pixmap.height();
    let positions: Vec<_> = vertices
        .iter()
        .map(|v| {
            Point::new(
                transform.sx * v.position.x + transform.kx * v.position.y + transform.tx,
                transform.ky * v.position.x + transform.sy * v.position.y + transform.ty,
            )
        })
        .collect();
    let output = pixmap.data_mut();
    for triangle in indices.as_chunks::<3>().0 {
        let mut ids = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        let mut area = edge(positions[ids[0]], positions[ids[1]], positions[ids[2]]);
        if area.abs() < 0.00001 {
            continue;
        }
        if area < 0. {
            ids.swap(1, 2);
            area = -area;
        }
        let [a, b, c] = ids.map(|i| positions[i]);
        let x0 = a.x.min(b.x).min(c.x).floor().clamp(0., width as f32) as u32;
        let x1 = a.x.max(b.x).max(c.x).ceil().clamp(0., width as f32) as u32;
        let y0 = a.y.min(b.y).min(c.y).floor().clamp(0., height as f32) as u32;
        let y1 = a.y.max(b.y).max(c.y).ceil().clamp(0., height as f32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let q = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                let e = [edge(b, c, q), edge(c, a, q), edge(a, b, q)];
                if e.iter()
                    .zip([top_left(b, c), top_left(c, a), top_left(a, b)])
                    .any(|(e, top)| *e < 0. || (*e == 0. && !top))
                {
                    continue;
                }
                let mut uv = Point::default();
                for j in 0..3 {
                    uv.x += vertices[ids[j]].uv.x * e[j] / area;
                    uv.y += vertices[ids[j]].uv.y * e[j] / area;
                }
                let u = (uv.x * bitmap.width as f32 - 0.5).clamp(0., bitmap.width as f32 - 1.);
                let v = (uv.y * bitmap.height as f32 - 0.5).clamp(0., bitmap.height as f32 - 1.);
                let ix = u.floor() as u32;
                let iy = v.floor() as u32;
                let tx = u - ix as f32;
                let ty = v - iy as f32;
                let coords = [
                    (ix, iy),
                    (ix.saturating_add(1).min(bitmap.width - 1), iy),
                    (ix, iy.saturating_add(1).min(bitmap.height - 1)),
                    (
                        ix.saturating_add(1).min(bitmap.width - 1),
                        iy.saturating_add(1).min(bitmap.height - 1),
                    ),
                ];
                let weights = [
                    (1. - tx) * (1. - ty),
                    tx * (1. - ty),
                    (1. - tx) * ty,
                    tx * ty,
                ];
                let mut source = [0f32; 4];
                for j in 0..4 {
                    let offset = ((coords[j].1 * bitmap.width + coords[j].0) * 4) as usize;
                    for (channel, value) in source.iter_mut().enumerate() {
                        *value += bitmap.pixels[offset + channel] as f32 * weights[j];
                    }
                }
                let coverage =
                    mask.map_or(1., |m| m.data()[(y * width + x) as usize] as f32 / 255.);
                for c in &mut source {
                    *c *= opacity * coverage;
                }
                let offset = ((y * width + x) * 4) as usize;
                let inv = 1. - source[3] / 255.;
                for channel in 0..4 {
                    output[offset + channel] = (source[channel]
                        + output[offset + channel] as f32 * inv)
                        .round()
                        .clamp(0., 255.) as u8;
                }
            }
        }
    }
}
#[derive(Default)]
struct UiCache {
    textures: HashMap<usize, (Arc<DecodedBitmap>, egui::TextureHandle)>,
    bytes: usize,
}
pub fn paint_image(
    painter: &egui::Painter,
    bitmap: &Arc<DecodedBitmap>,
    vertices: &[TextureVertex],
    indices: &[u32],
    opacity: f32,
    rect: egui::Rect,
    scale: f32,
) {
    let id = egui::Id::unique("astral.raster.textures");
    let pool = painter
        .ctx()
        .data_mut(|d| d.get_temp_mut_or_default::<Arc<Mutex<UiCache>>>(id).clone());
    let mut pool = pool.lock().unwrap_or_else(|p| p.into_inner());
    let key = Arc::as_ptr(bitmap) as usize;
    // Texture cache also retains the image via its ID-derived key until eviction.
    if pool.bytes + bitmap.pixels.len() > 128 * 1024 * 1024 && !pool.textures.contains_key(&key) {
        pool.textures.clear();
        pool.bytes = 0;
    }
    let texture = pool
        .textures
        .entry(key)
        .or_insert_with(|| {
            (
                bitmap.clone(),
                painter.ctx().load_texture(
                    format!("bitmap-{}-{key}", bitmap.id),
                    egui::ColorImage::from_rgba_premultiplied(
                        [bitmap.width as usize, bitmap.height as usize],
                        &bitmap.pixels,
                    ),
                    egui::TextureOptions::LINEAR,
                ),
            )
        })
        .1
        .clone();
    pool.bytes = pool
        .textures
        .values()
        .map(|(image, _)| image.pixels.len())
        .sum();
    drop(pool);
    let color = egui::Color32::from_white_alpha((opacity * 255.).round().clamp(0., 255.) as u8);
    let mesh = egui::Mesh {
        texture_id: texture.id(),
        indices: indices.to_vec(),
        vertices: vertices
            .iter()
            .map(|v| egui::epaint::Vertex {
                pos: egui::Pos2::new(
                    rect.left() + v.position.x * scale,
                    rect.top() + v.position.y * scale,
                ),
                uv: egui::Pos2::new(v.uv.x, v.uv.y),
                color,
            })
            .collect(),
    };
    painter.add(egui::Shape::mesh(mesh));
}
