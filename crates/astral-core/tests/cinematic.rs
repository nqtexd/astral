use astral_core::*;
use std::collections::BTreeMap;
fn apply(s: &mut Session, commands: Vec<Command>) -> anyhow::Result<Receipt> {
    s.apply(Batch {
        project: s.project.id,
        expected_revision: s.project.revision,
        commands,
    })
}
#[test]
fn camera_easing_and_anchored_depth_preserve_start_and_separate_planes() {
    let mut p = Project::new("Camera", 100, 100, 24, 25);
    p.camera = vec![
        CameraKey {
            frame: 0,
            center: Point::new(-50., 70.),
            zoom: 1.5,
        },
        CameraKey {
            frame: 24,
            center: Point::new(50., 90.),
            zoom: 3.,
        },
    ];
    let l = DrawingLayer::new("Far");
    let id = l.id;
    p.layers.push(l);
    p.schema = 3;
    let mut s = Session::new(p).unwrap();
    let start = evaluate(&s.project, 0.);
    apply(
        &mut s,
        vec![
            Command::SetCameraEasing {
                frame: 0,
                easing: Some(Easing::Smooth),
            },
            Command::SetLayerDepth {
                layer: id,
                depth: Some(LayerDepth {
                    parallax: 0.25,
                    zoom: 0.,
                    anchor: None,
                }),
            },
        ],
    )
    .unwrap();
    let anchored = s.project.scene_for_layer(id, &start);
    assert_eq!(anchored.center, start.center);
    assert_eq!(anchored.zoom, 1.5);
    let quarter = evaluate(&s.project, 6.);
    assert!((quarter.center.x - (-34.375)).abs() < 0.001);
    let end = evaluate(&s.project, 24.);
    let far = s.project.scene_for_layer(id, &end);
    assert_eq!(far.center, Point::new(-25., 75.));
    assert_eq!(far.zoom, 1.5);
    let loaded: Project =
        serde_json::from_value(serde_json::to_value(&s.project).unwrap()).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded.schema, 13);
    s.undo(s.project.id, 1).unwrap();
    assert!(s.project.layer_depths.is_empty());
    assert!(s.project.camera_easing.is_empty());
}
#[test]
fn cinematic_keys_and_camera_timing_retime_atomically() {
    let mut p = Project::new("Retime", 32, 32, 24, 24);
    p.schema = 13;
    let a = Id::new_v4();
    let b = Id::new_v4();
    p.camera = vec![
        CameraKey {
            frame: 0,
            center: Point::new(16., 16.),
            zoom: 1.,
        },
        CameraKey {
            frame: 10,
            center: Point::new(20., 16.),
            zoom: 1.,
        },
    ];
    p.camera_easing.insert(10, Easing::Smooth);
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Finish".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: b,
        nodes: vec![
            CompositeNode {
                id: a,
                name: "Base".into(),
                op: CompositeOp::Solid {
                    color: [90, 100, 120, 255],
                },
            },
            CompositeNode {
                id: b,
                name: "Bloom".into(),
                op: CompositeOp::Cinematic {
                    source: a,
                    stage: CinematicStage {
                        effect: CinematicEffect::Bloom {
                            threshold: 0.5,
                            knee: 0.1,
                            radius: 8,
                            intensity: 1.,
                        },
                        mix: 1.,
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
    let mut s = Session::new(p).unwrap();
    apply(
        &mut s,
        vec![
            Command::SetProject {
                name: None,
                duration: Some(48),
                background: None,
            },
            Command::Retime {
                start: 0,
                end: 23,
                new_end: 46,
            },
        ],
    )
    .unwrap();
    assert!(s.project.camera_easing.contains_key(&20));
    if let CompositeOp::Cinematic { stage, .. } = &s.project.compositions[0].nodes[1].op {
        assert_eq!(stage.keys[1].frame, 20);
    } else {
        panic!("Missing finish");
    }
    let bad = Command::SetCameraEasing {
        frame: 0,
        easing: Some(Easing::Bezier {
            x1: 0.2,
            y1: -1.,
            x2: 0.8,
            y2: 1.,
        }),
    };
    assert!(apply(&mut s, vec![bad]).is_err());
    assert_eq!(s.project.revision, 1);
}
#[test]
fn budget_unknown_targets_and_invalid_controls_reject_without_partial_edits() {
    let mut s = Session::new(Project::new("Validation", 32, 32, 24, 24)).unwrap();
    assert!(
        apply(
            &mut s,
            vec![Command::SetLayerDepth {
                layer: Id::new_v4(),
                depth: Some(LayerDepth {
                    parallax: 1.,
                    zoom: 1.,
                    anchor: None
                })
            }]
        )
        .is_err()
    );
    assert!(
        apply(
            &mut s,
            vec![Command::SetCinematicCamera {
                keys: vec![CameraKey {
                    frame: 24,
                    center: Point::new(1., 1.),
                    zoom: 1.
                }],
                easing: BTreeMap::new()
            }]
        )
        .is_err()
    );
    assert_eq!(s.project.revision, 0);
    let bad = CinematicStage {
        effect: CinematicEffect::Grade {
            exposure: 0.,
            contrast: 1.,
            saturation: 1.,
            lift: [0.; 3],
            gamma: [0.; 3],
            gain: [1.; 3],
        },
        mix: 1.,
        keys: vec![],
    };
    assert!(bad.validate(24).is_err());
}
