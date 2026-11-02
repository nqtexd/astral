//! Original vector courier study: no imported film assets. Reproduce with:
//! cargo run -p astral --example comic_city -- docs/showcase/comic-city
use anyhow::Result;
use astral_core::*;
use image::{
    Delay, Frame,
    codecs::gif::{GifEncoder, Repeat},
};
use std::{fs, path::PathBuf};
fn shape(
    p: &mut Project,
    name: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: Color,
    kind: GraphicKind,
) {
    p.graphics.push(Graphic {
        id: Id::new_v4(),
        name: name.into(),
        contours: vec![],
        kind,
        start: 0,
        end: 47,
        size: Point::new(w, h),
        color,
        layer: GraphicLayer::Background,
        text: String::new(),
        keys: vec![GraphicKey {
            frame: 0,
            position: Point::new(x, y),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
        draw_on: false,
    });
}
fn stroke(
    points: &[(f32, f32)],
    color: Color,
    filled: bool,
    width: f32,
    origin: Point,
    rotation: f32,
) -> InkStroke {
    InkStroke {
        brush: None,
        bitmap: None,
        skin: None,
        pressures: vec![],
        style: BrushStyle::Solid,
        filled,
        curved: false,
        width,
        color,
        points: points
            .iter()
            .map(|(x, y)| {
                origin
                    + Point::new(
                        x * rotation.cos() - y * rotation.sin(),
                        x * rotation.sin() + y * rotation.cos(),
                    )
            })
            .collect(),
    }
}
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/showcase/comic-city"));
    fs::create_dir_all(&output)?;
    let mut p = Project::new(
        "Chromatic Courier — original illustrated 2D study",
        800,
        450,
        24,
        48,
    );
    p.schema = 12;
    p.background = [32, 18, 65, 255];
    shape(
        &mut p,
        "Moon",
        610.,
        30.,
        130.,
        130.,
        [250, 117, 140, 255],
        GraphicKind::Ellipse,
    );
    for i in 0..16 {
        let x = i as f32 * 57. - 30.;
        let height = 90. + ((i * 73) % 150) as f32;
        shape(
            &mut p,
            "City silhouette",
            x,
            450. - height,
            53.,
            height,
            [18 + (i % 3 * 8) as u8, 25, 59 + (i % 4 * 8) as u8, 255],
            GraphicKind::Rectangle,
        );
        for row in 0..(height as u32 / 22) {
            for col in 0..3 {
                if (row + col + i) % 3 != 0 {
                    shape(
                        &mut p,
                        "Window",
                        x + 8. + col as f32 * 14.,
                        458. - height + row as f32 * 22.,
                        5.,
                        9.,
                        if i % 2 == 0 {
                            [35, 183, 201, 220]
                        } else {
                            [213, 81, 147, 210]
                        },
                        GraphicKind::Rectangle,
                    );
                }
            }
        }
    }
    // Bright diagonal graphic accents are authored editable strokes too.
    let trails = DrawingLayer::new("Velocity marks — project cadence");
    let tid = trails.id;
    p.layers.push(trails);
    let hero = DrawingLayer::new("Courier — 12 FPS artwork");
    let hid = hero.id;
    p.layers.push(hero);
    p.layer_frame_rates.insert(hid, 12);
    for f in 0..48 {
        let t = f as f32 / 47.;
        let origin = Point::new(
            115. + t * 565.,
            160. - (t * std::f32::consts::PI).sin() * 42.,
        );
        let r = -0.25 + (t * 6.).sin() * 0.13;
        let swing = (t * 12.).sin() * 12.;
        let navy = [15, 20, 48, 255];
        let ink = [7, 10, 30, 255];
        let pink = [239, 49, 133, 255];
        let cyan = [47, 228, 216, 255];
        let pieces = vec![
            (
                vec![
                    (-20., 14.),
                    (-8., 18.),
                    (-35., 65. + swing),
                    (-65., 75. + swing),
                    (-70., 66. + swing),
                    (-45., 52. + swing),
                ],
                navy,
            ),
            (
                vec![
                    (0., 16.),
                    (14., 16.),
                    (28., 58. - swing),
                    (63., 61. - swing),
                    (64., 73. - swing),
                    (15., 73. - swing),
                    (-2., 40.),
                ],
                navy,
            ),
            (
                vec![
                    (-24., -48.),
                    (10., -52.),
                    (28., -17.),
                    (14., 26.),
                    (-24., 23.),
                    (-33., -7.),
                ],
                ink,
            ),
            (
                vec![
                    (-22., -46.),
                    (7., -49.),
                    (21., -20.),
                    (10., 16.),
                    (-24., 13.),
                    (-27., -8.),
                ],
                pink,
            ),
            (
                vec![
                    (-25., -35.),
                    (-41., -12.),
                    (-74., -22.),
                    (-79., -12.),
                    (-39., 2.),
                    (-12., -23.),
                ],
                pink,
            ),
            (
                vec![
                    (9., -42.),
                    (32., -24.),
                    (62., -43.),
                    (69., -34.),
                    (33., -8.),
                    (11., -16.),
                ],
                pink,
            ),
            (
                vec![
                    (-7., -53.),
                    (-13., -67.),
                    (-7., -89.),
                    (13., -98.),
                    (30., -88.),
                    (34., -69.),
                    (20., -52.),
                ],
                ink,
            ),
            (
                vec![
                    (-6., -69.),
                    (0., -88.),
                    (17., -91.),
                    (29., -82.),
                    (28., -68.),
                    (17., -57.),
                    (3., -57.),
                ],
                [246, 162, 111, 255],
            ),
            (
                vec![(-5., -79.), (31., -82.), (29., -70.), (-5., -67.)],
                cyan,
            ),
            (vec![(-5., -46.), (5., -48.), (9., 14.), (-2., 14.)], cyan),
            (
                vec![
                    (-65., 64. + swing),
                    (-49., 63. + swing),
                    (-59., 77. + swing),
                    (-80., 79. + swing),
                    (-82., 73. + swing),
                ],
                cyan,
            ),
            (
                vec![
                    (45., 61. - swing),
                    (64., 61. - swing),
                    (72., 74. - swing),
                    (46., 74. - swing),
                ],
                cyan,
            ),
        ];
        let strokes = pieces
            .into_iter()
            .map(|(v, c)| stroke(&v, c, true, 1., origin, r))
            .collect();
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: hid,
            frame: f,
            strokes,
        });
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: tid,
            frame: f,
            strokes: (0..5)
                .map(|i| {
                    stroke(
                        &[
                            (-140. - i as f32 * 8., -45. + i as f32 * 24.),
                            (-70. - i as f32 * 5., -45. + i as f32 * 24.),
                        ],
                        [58, 206, 220, 140],
                        false,
                        2.,
                        origin,
                        0.,
                    )
                })
                .collect(),
        });
    }
    let a = Id::new_v4();
    let b = Id::new_v4();
    let c = Id::new_v4();
    let d = Id::new_v4();
    p.compositions.push(Composition {
        id: Id::new_v4(),
        name: "Courier comic finish".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: d,
        nodes: vec![
            CompositeNode {
                id: a,
                name: "Illustrated courier".into(),
                op: CompositeOp::Drawing { layer: hid },
            },
            CompositeNode {
                id: b,
                name: "Rotated print dots".into(),
                op: CompositeOp::Comic {
                    source: a,
                    style: ComicStyle::Halftone {
                        spacing: 5.,
                        angle: 0.7853982,
                        ink: [7, 10, 30, 255],
                        strength: 0.55,
                        keys: vec![],
                    },
                },
            },
            CompositeNode {
                id: c,
                name: "Print misregistration".into(),
                op: CompositeOp::Comic {
                    source: b,
                    style: ComicStyle::RgbSplit {
                        offset: Point::new(4., 0.),
                        strength: 0.65,
                        keys: vec![],
                    },
                },
            },
            CompositeNode {
                id: d,
                name: "Dimensional interruption".into(),
                op: CompositeOp::Comic {
                    source: c,
                    style: ComicStyle::Glitch {
                        displacement: 30.,
                        band_height: 9,
                        rate: 12,
                        seed: 2026,
                        strength: 0.,
                        keys: vec![
                            ComicKey {
                                frame: 0,
                                strength: 0.,
                            },
                            ComicKey {
                                frame: 16,
                                strength: 0.,
                            },
                            ComicKey {
                                frame: 20,
                                strength: 1.,
                            },
                            ComicKey {
                                frame: 26,
                                strength: 0.,
                            },
                            ComicKey {
                                frame: 47,
                                strength: 0.,
                            },
                        ],
                    },
                },
            },
        ],
    });
    p.validate()?;
    let workspace = astral_storage::Workspace::new(&output)?;
    workspace.save("comic-city.astral", &p)?;
    let png = astral_render::png(&p, 20., 800, 450)?;
    fs::write(output.join("preview.png"), png)?;
    let mut sheet = image::RgbaImage::new(800, 450);
    for (i, f) in [0., 16., 22., 36.].iter().enumerate() {
        let png = astral_render::png(&p, *f, 400, 225)?;
        let tile = image::load_from_memory(&png)?.into_rgba8();
        image::imageops::overlay(
            &mut sheet,
            &tile,
            (i % 2) as i64 * 400,
            (i / 2) as i64 * 225,
        );
    }
    sheet.save(output.join("frames.png"))?;
    let mut gif = GifEncoder::new_with_speed(fs::File::create(output.join("comic-city.gif"))?, 10);
    gif.set_repeat(Repeat::Infinite)?;
    for f in 0..48 {
        let png = astral_render::png(&p, f as f64, 480, 270)?;
        let tile = image::load_from_memory(&png)?.into_rgba8();
        gif.encode_frame(Frame::from_parts(
            tile,
            0,
            0,
            Delay::from_numer_denom_ms(1000, 24),
        ))?;
    }
    println!(
        "Wrote editable project, PNG inspection frames and 24 FPS GIF to {}",
        output.display()
    );
    Ok(())
}
