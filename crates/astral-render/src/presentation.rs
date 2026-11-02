use crate::{DisplayList, Primitive, text};
use astral_core::*;

fn screen(p: &Project, s: &EvaluatedScene, q: Point) -> Point {
    Point::new(
        (q.x - s.center.x) * s.zoom + p.width as f32 / 2.,
        (q.y - s.center.y) * s.zoom + p.height as f32 / 2.,
    )
}
fn ellipse(x: f32, y: f32, rx: f32, ry: f32) -> Vec<Point> {
    (0..64)
        .map(|i| {
            let a = i as f32 / 64. * std::f32::consts::TAU;
            Point::new(x + a.cos() * rx, y + a.sin() * ry)
        })
        .collect()
}
fn rounded(width: f32, height: f32, r: f32) -> Vec<Point> {
    let mut out = vec![];
    for (cx, cy, angle) in [
        (r, r, std::f32::consts::PI),
        (width - r, r, std::f32::consts::FRAC_PI_2 * 3.),
        (width - r, height - r, 0.),
        (r, height - r, std::f32::consts::FRAC_PI_2),
    ] {
        for i in 0..=10 {
            let a = angle + i as f32 / 10. * std::f32::consts::FRAC_PI_2;
            out.push(Point::new(cx + a.cos() * r, cy + a.sin() * r));
        }
    }
    out
}
fn cubic(out: &mut Vec<Point>, a: Point, b: Point, c: Point, d: Point) {
    for i in 1..=16 {
        let t = i as f32 / 16.;
        let u = 1. - t;
        out.push(Point::new(
            u * u * u * a.x + 3. * u * u * t * b.x + 3. * u * t * t * c.x + t * t * t * d.x,
            u * u * u * a.y + 3. * u * u * t * b.y + 3. * u * t * t * c.y + t * t * t * d.y,
        ));
    }
}
fn cloud() -> Vec<Point> {
    let mut out = vec![Point::new(-0.4, 0.4)];
    for (b, c, d) in [
        ([-0.65, 0.4], [-0.7, -0.05], [-0.38, -0.1]),
        ([-0.4, -0.4], [-0.02, -0.6], [0.1, -0.26]),
        ([0.32, -0.45], [0.58, -0.22], [0.49, 0.]),
        ([0.78, 0.05], [0.69, 0.4], [0.4, 0.4]),
    ] {
        let a = *out.last().unwrap();
        cubic(
            &mut out,
            a,
            Point::new(b[0], b[1]),
            Point::new(c[0], c[1]),
            Point::new(d[0], d[1]),
        );
    }
    out
}
pub fn append_graphics(
    list: &mut DisplayList,
    p: &Project,
    s: &EvaluatedScene,
    frame: f64,
    layer: GraphicLayer,
) {
    let bindings: std::collections::BTreeMap<_, _> =
        p.bindings.iter().map(|b| (b.graphic, b)).collect();
    for g in p
        .graphics
        .iter()
        .filter(|g| g.layer == layer && frame >= g.start as f64 && frame <= g.end as f64)
    {
        let adjusted = p.scene_for_layer(g.id, s);
        let s = &adjusted;
        let frame = p.layer_frame(g.id, frame);
        let key = if let Some(binding) = bindings.get(&g.id) {
            let Some(evaluated) = s.characters.iter().find(|c| c.id == binding.character) else {
                continue;
            };
            bound_graphic_key(
                g,
                binding,
                p.character(binding.character).unwrap(),
                &evaluated.pose,
                frame,
            )
        } else {
            g.at(frame)
        };
        let transform = |v: Point| {
            let x = v.x * key.scale;
            let y = v.y * key.scale;
            screen(
                p,
                s,
                key.position
                    + Point::new(
                        x * key.rotation.cos() - y * key.rotation.sin(),
                        x * key.rotation.sin() + y * key.rotation.cos(),
                    ),
            )
        };
        let w = g.size.x;
        let h = g.size.y;
        let mut contours = if !g.contours.is_empty() {
            g.contours.clone()
        } else {
            match g.kind {
                GraphicKind::Cloud => vec![
                    cloud()
                        .into_iter()
                        .map(|q| Point::new(q.x * w, q.y * h))
                        .collect(),
                ],
                GraphicKind::Rectangle | GraphicKind::LowerThird => vec![
                    rounded(w, h, 12f32.min(w / 4.).min(h / 4.))
                        .into_iter()
                        .map(|v| v - Point::new(w / 2., h / 2.))
                        .collect(),
                ],
                GraphicKind::Ellipse => vec![ellipse(0., 0., w / 2., h / 2.)],
                GraphicKind::Ring => {
                    let outer = ellipse(0., 0., w / 2., h / 2.);
                    let mut inner = ellipse(0., 0., w * 0.37, h * 0.37);
                    inner.reverse();
                    vec![outer, inner]
                }
                GraphicKind::Star | GraphicKind::Burst => {
                    let points = if g.kind == GraphicKind::Star { 10 } else { 24 };
                    vec![
                        (0..points)
                            .map(|i| {
                                let a = i as f32 / points as f32 * std::f32::consts::TAU
                                    - std::f32::consts::FRAC_PI_2;
                                let r = if i % 2 == 0 { 0.5 } else { 0.23 };
                                Point::new(a.cos() * w * r, a.sin() * h * r)
                            })
                            .collect(),
                    ]
                }
                GraphicKind::Arrow => vec![vec![
                    Point::new(-w / 2., -h * 0.16),
                    Point::new(w * 0.05, -h * 0.16),
                    Point::new(w * 0.05, -h / 2.),
                    Point::new(w / 2., 0.),
                    Point::new(w * 0.05, h / 2.),
                    Point::new(w * 0.05, h * 0.16),
                    Point::new(-w / 2., h * 0.16),
                ]],
            }
        };
        let mut color = g.color;
        color[3] = (color[3] as f32 * key.opacity.clamp(0., 1.)) as u8;
        if g.draw_on {
            let t = if g.end == g.start {
                1.
            } else {
                ((frame - g.start as f64) / (g.end - g.start) as f64) as f32
            };
            for contour in contours {
                let count = ((contour.len() + 1) as f32 * t).ceil() as usize;
                let points = contour
                    .iter()
                    .chain(contour.first())
                    .take(count.max(1))
                    .map(|v| transform(*v))
                    .collect();
                list.primitives.push(Primitive::Polyline {
                    points,
                    width: 4. * key.scale * s.zoom,
                    color,
                });
            }
        } else {
            for contour in &mut contours {
                for v in contour {
                    *v = transform(*v)
                }
            }
            // Reject art outside the camera before CPU rasterization/GPU tessellation.
            // Tracking scenes can span several canvas widths of editable scenery.
            let mut bounds = [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ];
            for point in contours.iter().flatten() {
                bounds[0] = bounds[0].min(point.x);
                bounds[1] = bounds[1].min(point.y);
                bounds[2] = bounds[2].max(point.x);
                bounds[3] = bounds[3].max(point.y);
            }
            if bounds[2] < -1.
                || bounds[3] < -1.
                || bounds[0] > p.width as f32 + 1.
                || bounds[1] > p.height as f32 + 1.
            {
                continue;
            }
            list.primitives.push(Primitive::Path { contours, color });
        }
        if g.kind == GraphicKind::LowerThird && !g.text.is_empty() {
            let contours = text::outlines(&g.text, h * 0.32, w - 40., h - 20., usize::MAX)
                .into_iter()
                .map(|c| {
                    c.into_iter()
                        .map(|v| transform(v - Point::new(w / 2. - 20., h / 2. - 5.)))
                        .collect()
                })
                .collect();
            list.primitives.push(Primitive::Path {
                contours,
                color: [255, 255, 255, color[3]],
            });
        }
    }
}
pub fn append_dialogues(list: &mut DisplayList, p: &Project, s: &EvaluatedScene, frame: f64) {
    for d in p
        .dialogues
        .iter()
        .filter(|d| frame >= d.start as f64 && frame <= d.end as f64)
    {
        let speaker = d
            .character
            .and_then(|id| s.characters.iter().find(|c| c.id == id));
        if d.character.is_some() && speaker.is_none() {
            continue;
        }
        let position = d.position + speaker.map(|c| c.pose.root).unwrap_or_default();
        let transform = |q: Point| screen(p, s, position + q);
        let border = d
            .appearance
            .border_width
            .min(d.size.x / 2.)
            .min(d.size.y / 2.);
        let radius = d
            .appearance
            .corner_radius
            .min(d.size.x / 2.)
            .min(d.size.y / 2.);
        if border > 0. {
            list.primitives.push(Primitive::Path {
                contours: vec![
                    rounded(d.size.x, d.size.y, radius)
                        .iter()
                        .map(|v| transform(*v))
                        .collect(),
                ],
                color: d.appearance.border_color,
            });
        }
        list.primitives.push(Primitive::Path {
            contours: vec![
                rounded(
                    d.size.x - 2. * border,
                    d.size.y - 2. * border,
                    (radius - border).max(0.),
                )
                .iter()
                .map(|v| transform(*v + Point::new(border, border)))
                .collect(),
            ],
            color: d.color,
        });
        if d.style == DialogueStyle::Speech {
            let target = speaker
                .map(|c| {
                    let head = p
                        .character(c.id)
                        .and_then(|c| c.joints.iter().position(|j| j.name == "head"))
                        .unwrap_or(0);
                    c.pose.root + c.pose.joints[head] - position
                })
                .unwrap_or(Point::new(d.size.x * 0.35, d.size.y + 35.));
            list.primitives.push(Primitive::Path {
                contours: vec![vec![
                    transform(Point::new(d.size.x * 0.4, d.size.y - 3.)),
                    transform(Point::new(d.size.x * 0.55, d.size.y - 3.)),
                    transform(target),
                ]],
                color: d.color,
            });
        }
        if d.style == DialogueStyle::Thought {
            for i in 0..3 {
                let center = transform(Point::new(
                    d.size.x * 0.35 - i as f32 * 12.,
                    d.size.y + 8. + i as f32 * 15.,
                ));
                list.primitives.push(Primitive::Circle {
                    center,
                    radius: (9. - i as f32 * 2.) * s.zoom,
                    color: d.color,
                });
            }
        }
        let padding = d.formatting.padding;
        let mut y = (padding - 2.).max(0.);
        if !d.speaker.is_empty() {
            let contours = text::outlines(
                &d.speaker,
                d.font_size * 0.75,
                d.size.x - padding * 2.,
                d.font_size * 1.1,
                usize::MAX,
            )
            .into_iter()
            .map(|c| {
                c.into_iter()
                    .map(|v| transform(v + Point::new(padding, y)))
                    .collect()
            })
            .collect();
            list.primitives.push(Primitive::Path {
                contours,
                color: d.appearance.speaker_color,
            });
            y += d.font_size * 1.2;
        }
        let reveal = if d.typewriter > 0. {
            (((frame - d.start as f64) / p.fps as f64 * d.typewriter as f64).floor() as usize)
                .saturating_add(1)
        } else {
            usize::MAX
        };
        let contours = text::outlines_styled(
            &d.text,
            d.font_size,
            d.size.x - padding * 2.,
            d.size.y - y - (padding - 4.).max(0.),
            reveal,
            &d.formatting,
        )
        .into_iter()
        .map(|c| {
            c.into_iter()
                .map(|v| transform(v + Point::new(padding, y)))
                .collect()
        })
        .collect();
        list.primitives.push(Primitive::Path {
            contours,
            color: d.text_color,
        });
    }
}
