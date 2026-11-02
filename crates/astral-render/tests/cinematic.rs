use astral_core::*;
use astral_render::{cinematic::*, *};
fn stage(effect: CinematicEffect) -> CinematicStage {
    CinematicStage {
        effect,
        mix: 1.,
        keys: vec![],
    }
}
#[test]
fn transfer_functions_and_linear_mix_match_known_values() {
    assert!((linear_to_srgb(0.5) - 0.735357).abs() < 0.00001);
    assert!((srgb_to_linear(linear_to_srgb(0.18)) - 0.18).abs() < 0.00001);
    let mut grade = stage(CinematicEffect::Grade {
        exposure: 0.,
        contrast: 1.,
        saturation: 1.,
        lift: [0.; 3],
        gamma: [1.; 3],
        gain: [1.; 3],
    });
    let src = [32, 64, 80, 128, 0, 0, 0, 0];
    assert_eq!(process(&src, 2, 1, &grade, 0.), src);
    grade.mix = 0.;
    assert_eq!(process(&src, 2, 1, &grade, 0.), src);
}
#[test]
fn shadows_follow_alpha_and_rim_light_preserves_coverage() {
    let mut src = vec![0; 9 * 3 * 4];
    let i = (1 * 9 + 3) * 4;
    src[i..i + 4].copy_from_slice(&[80, 40, 20, 128]);
    let shadow = stage(CinematicEffect::DropShadow {
        offset: Point::new(2., 0.),
        radius: 0,
        color: [0, 0, 0, 255],
        opacity: 0.5,
    });
    let out = process(&src, 9, 3, &shadow, 0.);
    assert_eq!(&out[i..i + 4], &src[i..i + 4]);
    assert!((out[(1 * 9 + 5) * 4 + 3] as i32 - 64).abs() <= 1);
    let rim = stage(CinematicEffect::RimLight {
        offset: Point::new(1., 0.),
        radius: 0,
        color: [100, 180, 255, 255],
        intensity: 0.5,
    });
    let rim = process(&src, 9, 3, &rim, 0.);
    assert!(rim[i + 2] > src[i + 2]);
    for (a, b) in rim.chunks_exact(4).zip(src.chunks_exact(4)) {
        assert_eq!(a[3], b[3]);
        assert!(a[..3].iter().all(|c| *c <= a[3]));
    }
}
#[test]
fn bloom_spreads_bright_energy_without_transparent_rgb_leaks() {
    let mut src = vec![0; 32 * 32 * 4];
    let i = (16 * 32 + 16) * 4;
    src[i..i + 4].copy_from_slice(&[255, 220, 180, 255]);
    let bloom = stage(CinematicEffect::Bloom {
        threshold: 0.4,
        knee: 0.1,
        radius: 9,
        intensity: 2.,
    });
    let out = process(&src, 32, 32, &bloom, 0.);
    assert!(out[(16 * 32 + 18) * 4 + 3] > 0);
    assert!(
        out.chunks_exact(4)
            .all(|p| p[..3].iter().all(|c| *c <= p[3]))
    );
    let invisible = vec![255, 120, 40, 0];
    assert_eq!(process(&invisible, 1, 1, &bloom, 0.), [0, 0, 0, 0]);
}
#[test]
fn animated_finishing_invalidates_cache_and_depth_applies_before_source_crop() {
    let mut p = Project::new("Depth", 64, 64, 24, 24);
    p.schema = 13;
    p.background = [0; 4];
    let l = DrawingLayer::new("Ink");
    let id = l.id;
    p.layers.push(l);
    p.cels.push(DrawingCel {
        id: Id::new_v4(),
        layer: id,
        frame: 0,
        strokes: vec![InkStroke {
            brush: None,
            bitmap: None,
            skin: None,
            pressures: vec![],
            style: BrushStyle::Solid,
            filled: true,
            curved: false,
            points: vec![
                Point::new(20., 20.),
                Point::new(44., 20.),
                Point::new(44., 44.),
                Point::new(20., 44.),
            ],
            color: [120, 80, 60, 255],
            width: 1.,
        }],
    });
    p.camera = vec![
        CameraKey {
            frame: 0,
            center: Point::new(32., 32.),
            zoom: 1.,
        },
        CameraKey {
            frame: 23,
            center: Point::new(96., 32.),
            zoom: 1.,
        },
    ];
    p.layer_depths.insert(
        id,
        LayerDepth {
            parallax: 0.,
            zoom: 0.,
            anchor: None,
        },
    );
    assert_eq!(rgba(&p, 0.).unwrap(), rgba(&p, 23.).unwrap());
    let a = Id::new_v4();
    let b = Id::new_v4();
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Finish".into(),
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
                name: "Grade".into(),
                op: CompositeOp::Cinematic {
                    source: a,
                    stage: CinematicStage {
                        effect: CinematicEffect::Grade {
                            exposure: 1.,
                            contrast: 1.,
                            saturation: 1.,
                            lift: [0.; 3],
                            gamma: [1.; 3],
                            gain: [1.; 3],
                        },
                        mix: 1.,
                        keys: vec![
                            ComicKey {
                                frame: 0,
                                strength: 0.,
                            },
                            ComicKey {
                                frame: 23,
                                strength: 1.,
                            },
                        ],
                    },
                },
            },
        ],
    }];
    p.validate().unwrap();
    let first = rgba(&p, 0.).unwrap();
    let last = rgba(&p, 23.).unwrap();
    assert_ne!(first, last);
    assert_eq!(first, rgba(&p, 0.).unwrap());
}
#[test]
fn composited_planes_retain_camera_shake() {
    let mut p = Project::new("Shake", 64, 64, 24, 24);
    p.schema = 13;
    p.background = [0; 4];
    let l = DrawingLayer::new("Plane");
    let id = l.id;
    p.layers.push(l);
    p.cels.push(DrawingCel {
        id: Id::new_v4(),
        layer: id,
        frame: 0,
        strokes: vec![InkStroke {
            brush: None,
            bitmap: None,
            skin: None,
            pressures: vec![],
            style: BrushStyle::Solid,
            filled: true,
            curved: false,
            points: vec![
                Point::new(24., 24.),
                Point::new(40., 24.),
                Point::new(40., 40.),
                Point::new(24., 40.),
            ],
            color: [180, 100, 80, 255],
            width: 1.,
        }],
    });
    p.effects.push(Effect {
        id: Id::new_v4(),
        name: "Shake".into(),
        kind: EffectKind::CameraShake,
        start: 0,
        end: 23,
        position: Point::new(32., 32.),
        character: None,
        joint: None,
        color: [0; 4],
        intensity: 1.,
        seed: 4,
    });
    let original = rgba(&p, 3.).unwrap();
    let a = Id::new_v4();
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Plane".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: a,
        nodes: vec![CompositeNode {
            id: a,
            name: "Ink".into(),
            op: CompositeOp::Drawing { layer: id },
        }],
    }];
    let composed = rgba(&p, 3.).unwrap();
    for (a, b) in original.iter().zip(composed) {
        assert!((*a as i32 - b as i32).abs() <= 2);
    }
}
#[cfg(feature = "gpu")]
#[test]
fn linear_finished_texture_reaches_gpu_with_matching_pixels() {
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
    let mut p = Project::new("Finish", 32, 32, 24, 24);
    p.schema = 13;
    p.background = [0; 4];
    let a = Id::new_v4();
    let b = Id::new_v4();
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Grade".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: b,
        nodes: vec![
            CompositeNode {
                id: a,
                name: "Base".into(),
                op: CompositeOp::Solid {
                    color: [100, 130, 160, 180],
                },
            },
            CompositeNode {
                id: b,
                name: "Grade".into(),
                op: CompositeOp::Cinematic {
                    source: a,
                    stage: stage(CinematicEffect::Grade {
                        exposure: 0.5,
                        contrast: 1.1,
                        saturation: 0.8,
                        lift: [0.01, 0., 0.],
                        gamma: [1.; 3],
                        gain: [1.; 3],
                    }),
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
                assert!((cpu[i + c] as i32 - output[i + c] as i32).abs() <= 3);
            }
        }
    }
    eprintln!("Cinematic GPU: {}", gpu.adapter);
}
