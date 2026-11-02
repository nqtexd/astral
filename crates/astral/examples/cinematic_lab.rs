//! Original multiplane lighting study; demonstrates tools, not an artistic quality benchmark.
use anyhow::Result;
use astral_core::*;
use image::{
    Delay, Frame,
    codecs::gif::{GifEncoder, Repeat},
};
use std::{fs, path::PathBuf};
fn polygon(points: Vec<Point>, color: Color) -> InkStroke {
    InkStroke {
        brush: None,
        bitmap: None,
        skin: None,
        pressures: vec![],
        style: BrushStyle::Solid,
        filled: true,
        curved: false,
        points,
        color,
        width: 1.,
    }
}
fn pts(v: &[(f32, f32)]) -> Vec<Point> {
    v.iter().map(|(x, y)| Point::new(*x, *y)).collect()
}
fn ellipse(x: f32, y: f32, rx: f32, ry: f32, color: Color) -> InkStroke {
    polygon(
        (0..48)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 48.;
                Point::new(x + a.cos() * rx, y + a.sin() * ry)
            })
            .collect(),
        color,
    )
}
fn layer(p: &mut Project, name: &str, depth: f32, strokes: Vec<InkStroke>) -> Id {
    let l = DrawingLayer::new(name);
    let id = l.id;
    p.layers.push(l);
    p.layer_depths.insert(
        id,
        LayerDepth {
            parallax: depth,
            zoom: depth,
            anchor: None,
        },
    );
    p.cels.push(DrawingCel {
        id: Id::new_v4(),
        layer: id,
        frame: 0,
        strokes,
    });
    id
}
fn node(nodes: &mut Vec<CompositeNode>, name: &str, op: CompositeOp) -> Id {
    let id = Id::new_v4();
    nodes.push(CompositeNode {
        id,
        name: name.into(),
        op,
    });
    id
}
fn finish(nodes: &mut Vec<CompositeNode>, source: Id, name: &str, effect: CinematicEffect) -> Id {
    node(
        nodes,
        name,
        CompositeOp::Cinematic {
            source,
            stage: CinematicStage {
                effect,
                mix: 1.,
                keys: vec![],
            },
        },
    )
}
fn project() -> Result<Project> {
    let mut p = Project::new("Amber passage — cinematic staging lab", 960, 540, 24, 72);
    p.schema = 13;
    p.background = [13, 19, 35, 255];
    p.camera = vec![
        CameraKey {
            frame: 0,
            center: Point::new(480., 270.),
            zoom: 1.,
        },
        CameraKey {
            frame: 71,
            center: Point::new(515., 257.),
            zoom: 1.08,
        },
    ];
    p.camera_easing.insert(0, Easing::Smooth);
    let moon = layer(
        &mut p,
        "Sun disc",
        0.05,
        vec![ellipse(685., 175., 56., 56., [255, 211, 145, 255])],
    );
    let far = layer(
        &mut p,
        "Distant ridges",
        0.18,
        vec![
            polygon(
                pts(&[
                    (-200., 310.),
                    (-60., 280.),
                    (65., 200.),
                    (180., 266.),
                    (330., 166.),
                    (470., 282.),
                    (620., 230.),
                    (780., 266.),
                    (1010., 180.),
                    (1120., 600.),
                    (-200., 600.),
                ]),
                [107, 90, 111, 255],
            ),
            polygon(
                pts(&[
                    (-200., 385.),
                    (50., 285.),
                    (210., 330.),
                    (400., 248.),
                    (600., 320.),
                    (800., 248.),
                    (1100., 380.),
                    (1100., 600.),
                    (-200., 600.),
                ]),
                [72, 80, 104, 255],
            ),
        ],
    );
    let mut forest = vec![polygon(
        pts(&[
            (-100., 380.),
            (250., 335.),
            (540., 385.),
            (980., 330.),
            (1100., 600.),
            (-100., 600.),
        ]),
        [38, 61, 78, 255],
    )];
    for i in 0..24 {
        let x = i as f32 * 47. - 70.;
        let top = 265. + ((i * 37) % 62) as f32;
        forest.push(polygon(
            pts(&[(x - 18., 410.), (x + 3., top), (x + 24., 410.)]),
            [34, 58 + (i % 3 * 4) as u8, 72, 255],
        ));
    }
    let forest = layer(&mut p, "Middle forest", 0.5, forest);
    let ground = layer(
        &mut p,
        "Ridge path",
        1.,
        vec![
            polygon(
                pts(&[
                    (-160., 490.),
                    (60., 452.),
                    (230., 468.),
                    (420., 400.),
                    (550., 417.),
                    (740., 460.),
                    (1100., 401.),
                    (1100., 620.),
                    (-160., 620.),
                ]),
                [22, 33, 46, 255],
            ),
            polygon(
                pts(&[(300., 540.), (480., 422.), (526., 429.), (435., 540.)]),
                [91, 80, 74, 255],
            ),
        ],
    );
    let actor = layer(&mut p, "Traveler artwork — on twos", 1., vec![]);
    p.layer_frame_rates.insert(actor, 12);
    p.cels.retain(|c| c.layer != actor);
    let lantern = layer(
        &mut p,
        "Lantern emission",
        1.,
        vec![ellipse(560., 385., 9., 13., [255, 225, 161, 255])],
    );
    for frame in (0..72).step_by(2) {
        let breath = (frame as f32 / 71. * std::f32::consts::TAU).sin() * 1.2;
        let x = 502.;
        let y = 407. + breath;
        let translate = |values: &[(f32, f32)], color| {
            polygon(
                values
                    .iter()
                    .map(|(a, b)| Point::new(x + a, y + b))
                    .collect(),
                color,
            )
        };
        let body = vec![
            translate(
                &[
                    (-15., -70.),
                    (7., -75.),
                    (20., -30.),
                    (12., 8.),
                    (-29., 8.),
                    (-31., -19.),
                ],
                [42, 45, 62, 255],
            ),
            translate(
                &[
                    (-13., -56.),
                    (4., -65.),
                    (15., -30.),
                    (-2., -5.),
                    (-27., -1.),
                ],
                [68, 67, 77, 255],
            ),
            translate(
                &[(-16., -5.), (-3., -3.), (-7., 27.), (-20., 28.)],
                [12, 23, 35, 255],
            ),
            translate(
                &[(1., -3.), (14., -3.), (18., 26.), (4., 27.)],
                [12, 23, 35, 255],
            ),
            translate(
                &[
                    (-8., -71.),
                    (-20., -82.),
                    (-15., -103.),
                    (0., -113.),
                    (16., -105.),
                    (23., -88.),
                    (13., -73.),
                ],
                [26, 34, 48, 255],
            ),
            translate(
                &[
                    (9., -99.),
                    (21., -95.),
                    (25., -87.),
                    (19., -77.),
                    (11., -79.),
                ],
                [164, 132, 105, 255],
            ),
            translate(
                &[
                    (8., -65.),
                    (21., -60.),
                    (36., -35.),
                    (53., -35.),
                    (54., -27.),
                    (30., -27.),
                    (11., -43.),
                ],
                [49, 53, 65, 255],
            ),
            translate(
                &[(-20., 23.), (-7., 23.), (-4., 31.), (-25., 31.)],
                [7, 18, 30, 255],
            ),
            translate(
                &[(5., 24.), (19., 24.), (25., 30.), (3., 31.)],
                [7, 18, 30, 255],
            ),
        ];
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: actor,
            frame,
            strokes: body,
        });
    }
    let mut foliage = vec![polygon(
        pts(&[
            (-120., -100.),
            (35., -100.),
            (67., 125.),
            (40., 290.),
            (10., 540.),
            (-120., 650.),
        ]),
        [9, 23, 35, 255],
    )];
    for i in 0..9 {
        foliage.push(ellipse(
            12. + i as f32 * 19.,
            12. + ((i * 43) % 75) as f32,
            86.,
            45.,
            [10, 27, 39, 255],
        ));
    }
    foliage.push(polygon(
        pts(&[(795., 540.), (1040., 415.), (1100., 650.)]),
        [9, 22, 34, 255],
    ));
    let foliage = layer(&mut p, "Near silhouette frame", 1.5, foliage);
    let mut nodes = vec![];
    let sky = node(
        &mut nodes,
        "Sky gradient",
        CompositeOp::Gradient {
            start: Point::new(0., 0.),
            end: Point::new(0., 1.),
            from: [29, 41, 65, 255],
            to: [226, 151, 113, 255],
            radial: false,
        },
    );
    let mut output = sky;
    for (name, id) in [
        ("Sun", moon),
        ("Far mountains", far),
        ("Middle forest", forest),
        ("Ground", ground),
        ("Traveler", actor),
        ("Lantern", lantern),
        ("Foreground", foliage),
    ] {
        let mut input = node(&mut nodes, name, CompositeOp::Drawing { layer: id });
        if id == actor {
            input = finish(
                &mut nodes,
                input,
                "Warm edge light",
                CinematicEffect::RimLight {
                    offset: Point::new(-3., 2.),
                    radius: 2,
                    color: [255, 187, 110, 255],
                    intensity: 0.65,
                },
            );
            input = finish(
                &mut nodes,
                input,
                "Soft silhouette shadow",
                CinematicEffect::DropShadow {
                    offset: Point::new(-10., 8.),
                    radius: 8,
                    color: [8, 14, 27, 255],
                    opacity: 0.4,
                },
            );
        }
        output = node(
            &mut nodes,
            "Layer over",
            CompositeOp::Blend {
                below: output,
                above: input,
                mode: BlendMode::Normal,
                opacity: 1.,
            },
        );
    }
    output = finish(
        &mut nodes,
        output,
        "Warm emission bloom",
        CinematicEffect::Bloom {
            threshold: 0.5,
            knee: 0.15,
            radius: 20,
            intensity: 0.65,
        },
    );
    output = finish(
        &mut nodes,
        output,
        "Dusk grade",
        CinematicEffect::Grade {
            exposure: 0.,
            contrast: 1.04,
            saturation: 0.85,
            lift: [0., 0.003, 0.012],
            gamma: [1.02, 1., 0.98],
            gain: [1.04, 1., 0.98],
        },
    );
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Complete scene finish".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output,
        nodes,
    }];
    p.validate()?;
    Ok(p)
}
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/showcase/cinematic-lab"));
    fs::create_dir_all(&output)?;
    let p = project()?;
    let workspace = astral_storage::Workspace::new(&output)?;
    workspace.save("amber-passage.astral", &p)?;
    fs::write(
        output.join("finished.png"),
        astral_render::png(&p, 36., 960, 540)?,
    )?;
    let mut before = p.clone();
    for n in before.compositions.iter_mut().flat_map(|g| &mut g.nodes) {
        if let CompositeOp::Cinematic { stage, .. } = &mut n.op {
            stage.mix = 0.;
            stage.keys.clear();
        }
    }
    fs::write(
        output.join("before.png"),
        astral_render::png(&before, 36., 960, 540)?,
    )?;
    let mut sheet = image::RgbaImage::new(960, 540);
    for (i, f) in [0., 24., 48., 71.].iter().enumerate() {
        let tile = image::load_from_memory(&astral_render::png(&p, *f, 480, 270)?)?.into_rgba8();
        image::imageops::overlay(
            &mut sheet,
            &tile,
            (i % 2) as i64 * 480,
            (i / 2) as i64 * 270,
        );
    }
    sheet.save(output.join("frames.png"))?;
    let mut gif =
        GifEncoder::new_with_speed(fs::File::create(output.join("amber-passage.gif"))?, 10);
    gif.set_repeat(Repeat::Infinite)?;
    for f in (0..72).step_by(2) {
        let tile =
            image::load_from_memory(&astral_render::png(&p, f as f64, 480, 270)?)?.into_rgba8();
        gif.encode_frame(Frame::from_parts(
            tile,
            0,
            0,
            Delay::from_numer_denom_ms(1000, 12),
        ))?;
    }
    println!(
        "Saved original staging/lighting study to {}",
        output.display()
    );
    Ok(())
}
