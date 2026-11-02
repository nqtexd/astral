//! Bounded CPU compositing produces ordinary textures shared by viewport and exports.
use crate::*;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};
struct Cached {
    graph: Composition,
    inputs: Vec<(Id, DisplayList)>,
    bitmap: Arc<DecodedBitmap>,
}
static CACHE: OnceLock<Mutex<VecDeque<Cached>>> = OnceLock::new();

fn input_project(p: &Project, op: &CompositeOp) -> Project {
    let mut q = p.clone();
    q.compositions.clear();
    q.background = [0, 0, 0, 0];
    q.effects.clear();
    q.drawings.clear();
    q.drawing_motions.clear();
    q.mouths.clear();
    for c in &mut q.characters {
        c.artwork_only = true;
    }
    q.rasters
        .retain(|r| matches!(op,CompositeOp::Raster{layer} if *layer==r.id));
    q.layers
        .retain(|r| matches!(op,CompositeOp::Drawing{layer} if *layer==r.id));
    q.cels.retain(|c| q.layers.iter().any(|l| l.id == c.layer));
    q.graphics
        .retain(|r| matches!(op,CompositeOp::Graphic{graphic} if *graphic==r.id));
    q.dialogues
        .retain(|r| matches!(op,CompositeOp::Dialogue{dialogue} if *dialogue==r.id));
    q
}
pub fn display(p: &Project, frame: f64) -> DisplayList {
    let mut base = p.clone();
    base.compositions.clear();
    for graph in p.compositions.iter().filter(|c| c.enabled) {
        for n in &graph.nodes {
            match n.op {
                CompositeOp::Raster { layer } => base.rasters.retain(|r| r.id != layer),
                CompositeOp::Drawing { layer } => {
                    base.layers.retain(|l| l.id != layer);
                    base.cels.retain(|c| c.layer != layer);
                }
                CompositeOp::Graphic { graphic } => base.graphics.retain(|g| g.id != graphic),
                CompositeOp::Dialogue { dialogue } => base.dialogues.retain(|g| g.id != dialogue),
                _ => {}
            }
        }
    }
    let mut list = crate::display_list(&base, frame);
    for graph in p.compositions.iter().filter(|c| c.enabled) {
        // Project validation guarantees IDs, DAG shape, dimensions and allocation budgets.
        let inputs: Vec<_> = graph
            .nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.op,
                    CompositeOp::Raster { .. }
                        | CompositeOp::Drawing { .. }
                        | CompositeOp::Graphic { .. }
                        | CompositeOp::Dialogue { .. }
                )
            })
            .map(|n| (n.id, crate::display_list(&input_project(p, &n.op), frame)))
            .collect();
        let cache = CACHE.get_or_init(Default::default);
        let cached = cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|c| c.graph == *graph && c.inputs == inputs)
            .map(|c| c.bitmap.clone());
        let bitmap = cached.unwrap_or_else(|| {
            let pixels = compose(p.width, p.height, graph, &inputs);
            let bitmap = Arc::new(DecodedBitmap {
                id: Id::new_v4(),
                width: p.width,
                height: p.height,
                pixels,
            });
            let mut entries = cache.lock().unwrap_or_else(|e| e.into_inner());
            let bytes = bitmap.pixels.len();
            while entries.len() >= 4
                || entries.iter().map(|e| e.bitmap.pixels.len()).sum::<usize>() + bytes
                    > 64 * 1024 * 1024
            {
                if entries.pop_front().is_none() {
                    break;
                }
            }
            entries.push_back(Cached {
                graph: graph.clone(),
                inputs,
                bitmap: bitmap.clone(),
            });
            bitmap
        });
        let primitive = Primitive::Image {
            bitmap,
            vertices: vec![
                TextureVertex {
                    position: Point::new(0., 0.),
                    uv: Point::new(0., 0.),
                },
                TextureVertex {
                    position: Point::new(p.width as f32, 0.),
                    uv: Point::new(1., 0.),
                },
                TextureVertex {
                    position: Point::new(p.width as f32, p.height as f32),
                    uv: Point::new(1., 1.),
                },
                TextureVertex {
                    position: Point::new(0., p.height as f32),
                    uv: Point::new(0., 1.),
                },
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            opacity: 1.,
        };
        if graph.layer == GraphicLayer::Background {
            list.primitives.insert(0, primitive);
        } else {
            list.primitives.push(primitive);
        }
    }
    list
}
fn compose(w: u32, h: u32, g: &Composition, inputs: &[(Id, DisplayList)]) -> Vec<u8> {
    let mut outputs: BTreeMap<Id, Vec<u8>> = BTreeMap::new();
    let size = w as usize * h as usize * 4;
    for id in g.order().expect("validated compositing DAG") {
        let n = g.nodes.iter().find(|n| n.id == id).unwrap();
        let pixels = match n.op {
            CompositeOp::Raster { .. }
            | CompositeOp::Drawing { .. }
            | CompositeOp::Graphic { .. }
            | CompositeOp::Dialogue { .. } => crate::rasterize(
                &inputs.iter().find(|(input, _)| *input == id).unwrap().1,
                w,
                h,
            )
            .expect("validated dimensions")
            .take(),
            CompositeOp::Solid { color } => {
                let a = color[3] as u16;
                let c = [
                    ((color[0] as u16 * a + 127) / 255) as u8,
                    ((color[1] as u16 * a + 127) / 255) as u8,
                    ((color[2] as u16 * a + 127) / 255) as u8,
                    color[3],
                ];
                let mut v = vec![0; size];
                for p in v.as_chunks_mut::<4>().0.iter_mut() {
                    p.copy_from_slice(&c);
                }
                v
            }
            CompositeOp::Gradient {
                start,
                end,
                from,
                to,
                radial,
            } => {
                let mut v = vec![0; size];
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let len = dx * dx + dy * dy;
                for (y, row) in v.chunks_exact_mut(w as usize * 4).enumerate() {
                    for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                        let qx = (x as f32 + 0.5) / w as f32 - start.x;
                        let qy = (y as f32 + 0.5) / h as f32 - start.y;
                        let t = (if radial {
                            ((qx * qx + qy * qy) / len).sqrt()
                        } else {
                            (qx * dx + qy * dy) / len
                        })
                        .clamp(0., 1.);
                        let alpha = from[3] as f32 + (to[3] as f32 - from[3] as f32) * t;
                        for i in 0..3 {
                            let a = from[i] as f32 * from[3] as f32 / 255.;
                            let b = to[i] as f32 * to[3] as f32 / 255.;
                            p[i] = (a + (b - a) * t).round() as u8;
                        }
                        p[3] = alpha.round() as u8;
                    }
                }
                v
            }
            CompositeOp::Blend {
                below,
                above,
                mode,
                opacity,
            } => blend(&outputs[&below], &outputs[&above], mode, opacity),
            CompositeOp::Mask {
                source,
                mask,
                invert,
                luminance,
            } => {
                let mut v = outputs[&source].clone();
                for (p, m) in v
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(outputs[&mask].as_chunks::<4>().0.iter())
                {
                    let mut a = if luminance {
                        (0.2126 * m[0] as f32 + 0.7152 * m[1] as f32 + 0.0722 * m[2] as f32) / 255.
                    } else {
                        m[3] as f32 / 255.
                    };
                    if invert {
                        a = 1. - a;
                    }
                    for channel in p {
                        *channel = (*channel as f32 * a).round() as u8;
                    }
                }
                v
            }
            CompositeOp::Blur { source, radius } => {
                if radius == 0 {
                    outputs[&source].clone()
                } else {
                    let image = image::RgbaImage::from_raw(w, h, outputs[&source].clone()).unwrap();
                    image::imageops::blur(&image, radius as f32 / 2.).into_raw()
                }
            }
            CompositeOp::Color {
                source,
                exposure,
                saturation,
            } => {
                let mut v = outputs[&source].clone();
                for p in v.as_chunks_mut::<4>().0.iter_mut() {
                    let a = p[3] as f32;
                    if a > 0. {
                        let rgb = [p[0] as f32 / a, p[1] as f32 / a, p[2] as f32 / a];
                        let l = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
                        for i in 0..3 {
                            p[i] = (((l + (rgb[i] - l) * saturation) * 2f32.powf(exposure))
                                .clamp(0., 1.)
                                * a)
                                .round() as u8;
                        }
                    }
                }
                v
            }
        };
        outputs.insert(id, pixels);
    }
    outputs.remove(&g.output).unwrap()
}
pub(crate) fn blend(below: &[u8], above: &[u8], mode: BlendMode, opacity: f32) -> Vec<u8> {
    let mut out = below.to_vec();
    for (p, s) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(above.as_chunks::<4>().0.iter())
    {
        let ab = p[3] as f32 / 255.;
        let original_as = s[3] as f32 / 255.;
        let a = original_as * opacity;
        let alpha = a + ab * (1. - a);
        for i in 0..3 {
            let cb = if ab > 0. { p[i] as f32 / 255. / ab } else { 0. };
            let cs = if original_as > 0. {
                s[i] as f32 / 255. / original_as
            } else {
                0.
            };
            let b = match mode {
                BlendMode::Normal => cs,
                BlendMode::Multiply => cb * cs,
                BlendMode::Screen => cb + cs - cb * cs,
                BlendMode::Overlay => {
                    if cb <= 0.5 {
                        2. * cb * cs
                    } else {
                        1. - 2. * (1. - cb) * (1. - cs)
                    }
                }
                BlendMode::Darken => cb.min(cs),
                BlendMode::Lighten => cb.max(cs),
                BlendMode::Difference => (cb - cs).abs(),
                BlendMode::Add => (cb + cs).min(1.),
            };
            p[i] = (((1. - a) * cb * ab + (1. - ab) * cs * a + a * ab * b) * 255.)
                .round()
                .clamp(0., alpha * 255.) as u8;
        }
        p[3] = (alpha * 255.).round() as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn premultiplied_blends_handle_transparent_pixels() {
        assert_eq!(
            blend(&[0, 0, 0, 0], &[64, 0, 0, 128], BlendMode::Multiply, 1.),
            [64, 0, 0, 128]
        );
        assert_eq!(
            blend(
                &[255, 255, 255, 255],
                &[128, 0, 0, 128],
                BlendMode::Normal,
                1.
            ),
            [255, 127, 127, 255]
        );
        assert_eq!(
            blend(
                &[200, 100, 50, 255],
                &[128, 128, 128, 255],
                BlendMode::Multiply,
                1.
            ),
            [100, 50, 25, 255]
        );
    }
}
