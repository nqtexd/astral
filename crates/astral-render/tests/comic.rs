use astral_core::*;
use astral_render::{comic::process, *};
#[test]
fn rgb_separates_channels_without_invalid_alpha_and_zero_is_identity() {
    let mut pixels = vec![0; 9 * 4];
    pixels[4 * 4..5 * 4].copy_from_slice(&[255, 255, 255, 255]);
    let mut style = ComicStyle::RgbSplit {
        offset: Point::new(2., 0.),
        strength: 1.,
        keys: vec![],
    };
    let out = process(&pixels, 9, 1, &style, 0., 24);
    assert_eq!(&out[2 * 4..3 * 4], &[0, 0, 255, 255]);
    assert_eq!(&out[6 * 4..7 * 4], &[255, 0, 0, 255]);
    for p in out.chunks_exact(4) {
        assert!(p[..3].iter().all(|c| *c <= p[3]));
    }
    if let ComicStyle::RgbSplit { strength, .. } = &mut style {
        *strength = 0.;
    }
    assert_eq!(pixels, process(&pixels, 9, 1, &style, 0., 24));
}
#[test]
fn seeded_glitch_is_reproducible_time_varying_and_halftone_preserves_coverage() {
    let pixels: Vec<u8> = (0..64 * 64)
        .flat_map(|i| [(i % 256) as u8, (i % 200) as u8, 100, 255])
        .collect();
    let glitch = ComicStyle::Glitch {
        displacement: 20.,
        band_height: 4,
        rate: 12,
        seed: 7,
        strength: 1.,
        keys: vec![],
    };
    let a = process(&pixels, 64, 64, &glitch, 0., 24);
    assert_eq!(a, process(&pixels, 64, 64, &glitch, 0., 24));
    assert_eq!(a, process(&pixels, 64, 64, &glitch, 1., 24));
    assert_ne!(a, process(&pixels, 64, 64, &glitch, 2., 24));
    let dots = ComicStyle::Halftone {
        spacing: 8.,
        angle: 0.7,
        ink: [10, 10, 20, 255],
        strength: 1.,
        keys: vec![],
    };
    let flat = vec![160; 32 * 32 * 4];
    let out = process(&flat, 32, 32, &dots, 0., 24);
    assert_ne!(out, flat);
    assert!(out.chunks_exact(4).all(|p| p[3] == 160));
}
#[test]
fn animated_compositor_invalidates_static_source_cache() {
    let mut p = Project::new("Comic", 32, 32, 24, 24);
    p.schema = 12;
    p.background = [0; 4];
    let a = Id::new_v4();
    let b = Id::new_v4();
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Comic".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: b,
        nodes: vec![
            CompositeNode {
                id: a,
                name: "Art".into(),
                op: CompositeOp::Gradient {
                    start: Point::new(0., 0.),
                    end: Point::new(1., 1.),
                    from: [250, 40, 80, 255],
                    to: [20, 80, 230, 255],
                    radial: false,
                },
            },
            CompositeNode {
                id: b,
                name: "RGB".into(),
                op: CompositeOp::Comic {
                    source: a,
                    style: ComicStyle::RgbSplit {
                        offset: Point::new(6., 0.),
                        strength: 1.,
                        keys: vec![
                            ComicKey {
                                frame: 0,
                                strength: 0.,
                            },
                            ComicKey {
                                frame: 10,
                                strength: 1.,
                            },
                        ],
                    },
                },
            },
        ],
    }];
    p.validate().unwrap();
    let zero = rgba(&p, 0.).unwrap();
    let ten = rgba(&p, 10.).unwrap();
    assert_ne!(zero, ten);
    assert_eq!(ten, rgba(&p, 10.).unwrap());
    assert_eq!(zero, rgba(&p, 0.).unwrap());
}
#[test]
fn graphic_cadence_holds_artwork_but_not_camera() {
    let mut p = Project::new("Cadence", 64, 64, 24, 24);
    p.schema = 12;
    let id = Id::new_v4();
    p.graphics.push(Graphic {
        id,
        name: "Silhouette".into(),
        contours: vec![],
        kind: GraphicKind::Rectangle,
        start: 0,
        end: 23,
        size: Point::new(10., 10.),
        color: [200, 50, 80, 255],
        layer: GraphicLayer::Foreground,
        text: String::new(),
        keys: vec![
            GraphicKey {
                frame: 0,
                position: Point::new(12., 12.),
                scale: 1.,
                rotation: 0.,
                opacity: 1.,
                easing: Easing::Linear,
            },
            GraphicKey {
                frame: 23,
                position: Point::new(50., 40.),
                scale: 1.,
                rotation: 0.,
                opacity: 1.,
                easing: Easing::Linear,
            },
        ],
        draw_on: false,
    });
    p.layer_frame_rates.insert(id, 12);
    p.validate().unwrap();
    assert_eq!(rgba(&p, 0.).unwrap(), rgba(&p, 1.).unwrap());
    assert_ne!(rgba(&p, 1.).unwrap(), rgba(&p, 2.).unwrap());
    p.camera = vec![
        CameraKey {
            frame: 0,
            center: Point::new(32., 32.),
            zoom: 1.,
        },
        CameraKey {
            frame: 23,
            center: Point::new(42., 32.),
            zoom: 1.,
        },
    ];
    assert_ne!(rgba(&p, 0.).unwrap(), rgba(&p, 1.).unwrap());
}
#[cfg(feature = "gpu")]
#[test]
fn comic_texture_reaches_the_physical_gpu_export_path() {
    let mut gpu = match gpu::GpuRenderer::new(32, 32) {
        Ok(gpu) => gpu,
        Err(error) => {
            assert!(
                std::env::var_os("ASTRAL_REQUIRE_GPU").is_none(),
                "{error:#}"
            );
            return;
        }
    };
    let mut p = Project::new("GPU comic", 32, 32, 24, 24);
    p.schema = 12;
    p.background = [0; 4];
    let a = Id::new_v4();
    let b = Id::new_v4();
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Halftone".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: b,
        nodes: vec![
            CompositeNode {
                id: a,
                name: "Source".into(),
                op: CompositeOp::Solid {
                    color: [150, 80, 180, 255],
                },
            },
            CompositeNode {
                id: b,
                name: "Dots".into(),
                op: CompositeOp::Comic {
                    source: a,
                    style: ComicStyle::Halftone {
                        spacing: 8.,
                        angle: 0.7,
                        ink: [10, 10, 20, 255],
                        strength: 0.8,
                        keys: vec![],
                    },
                },
            },
        ],
    }];
    p.validate().unwrap();
    let cpu = rgba(&p, 0.).unwrap();
    let output = gpu.render(&display_list(&p, 0.), false).unwrap();
    for y in 2..30 {
        for x in 2..30 {
            let i = (y * 32 + x) * 4;
            for c in 0..4 {
                assert!(
                    (cpu[i + c] as i32 - output[i + c] as i32).abs() <= 3,
                    "GPU comic mismatch at {x},{y}"
                );
            }
        }
    }
    eprintln!("Comic texture GPU: {}", gpu.adapter);
}

#[test]
fn cadence_selects_drawing_cels_and_fps_changes_invalidate_glitch_cache() {
    let mut p = Project::new("Timing", 64, 64, 24, 24);
    p.schema = 12;
    p.background = [0; 4];
    let layer = DrawingLayer::new("Ink");
    let id = layer.id;
    p.layers.push(layer);
    p.layer_frame_rates.insert(id, 12);
    let stroke = |x| InkStroke {
        brush: None,
        bitmap: None,
        skin: None,
        pressures: vec![],
        style: BrushStyle::Solid,
        filled: false,
        curved: false,
        points: vec![Point::new(x, 8.), Point::new(x, 56.)],
        color: [255, 100, 80, 255],
        width: 4.,
    };
    for frame in 0..3 {
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: id,
            frame,
            strokes: vec![stroke(10. + frame as f32 * 15.)],
        });
    }
    let a = Id::new_v4();
    let b = Id::new_v4();
    assert_eq!(rgba(&p, 0.).unwrap(), rgba(&p, 1.).unwrap());
    assert_ne!(rgba(&p, 1.).unwrap(), rgba(&p, 2.).unwrap());
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Glitch".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: b,
        nodes: vec![
            CompositeNode {
                id: a,
                name: "Drawing".into(),
                op: CompositeOp::Drawing { layer: id },
            },
            CompositeNode {
                id: b,
                name: "Glitch".into(),
                op: CompositeOp::Comic {
                    source: a,
                    style: ComicStyle::Glitch {
                        displacement: 20.,
                        band_height: 4,
                        rate: 12,
                        seed: 7,
                        strength: 1.,
                        keys: vec![],
                    },
                },
            },
        ],
    }];
    p.validate().unwrap();
    let first = rgba(&p, 2.).unwrap();
    p.fps = 12;
    let changed = rgba(&p, 2.).unwrap();
    assert_ne!(first, changed);
    p.fps = 24;
    assert_eq!(first, rgba(&p, 2.).unwrap());
}
