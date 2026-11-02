use astral_core::*;
fn batch(s: &mut Session, commands: Vec<Command>) -> anyhow::Result<Receipt> {
    s.apply(Batch {
        project: s.project.id,
        expected_revision: s.project.revision,
        commands,
    })
}
#[test]
fn cadence_is_independent_serialized_and_undoable() {
    let mut p = Project::new("Mixed timing", 64, 64, 24, 24);
    let a = DrawingLayer::new("12 FPS");
    let b = DrawingLayer::new("24 FPS");
    let (a, b) = (a.id, {
        p.layers.push(a);
        p.layers.push(b.clone());
        b.id
    });
    p.schema = 3;
    let mut s = Session::new(p).unwrap();
    batch(
        &mut s,
        vec![Command::SetLayerFrameRate {
            layer: a,
            fps: Some(12),
        }],
    )
    .unwrap();
    assert_eq!(s.project.schema, 12);
    assert_eq!(s.project.layer_frame(a, 3.), 2.);
    assert_eq!(s.project.layer_frame(b, 3.), 3.);
    let loaded: Project =
        serde_json::from_value(serde_json::to_value(&s.project).unwrap()).unwrap();
    assert_eq!(loaded.layer_frame(a, 5.), 4.);
    let rev = s.project.revision;
    s.undo(s.project.id, rev).unwrap();
    assert!(s.project.layer_frame_rates.is_empty());
    assert!(
        batch(
            &mut s,
            vec![Command::SetLayerFrameRate {
                layer: a,
                fps: Some(25)
            }]
        )
        .is_err()
    );
    assert!(
        batch(
            &mut s,
            vec![Command::SetLayerFrameRate {
                layer: Id::new_v4(),
                fps: Some(12)
            }]
        )
        .is_err()
    );
}
#[test]
fn comic_key_validation_and_cyclic_graph_rollback() {
    let mut s = Session::new(Project::new("Comic", 32, 32, 24, 24)).unwrap();
    let source = Id::new_v4();
    let output = Id::new_v4();
    let mut style = ComicStyle::RgbSplit {
        offset: Point::new(5., 0.),
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
    };
    assert!((style.strength_at(5.) - 0.5).abs() < 0.001);
    let graph = |style| Composition {
        id: Id::new_v4(),
        name: "Comic".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output,
        nodes: vec![
            CompositeNode {
                id: source,
                name: "Source".into(),
                op: CompositeOp::Solid {
                    color: [180, 80, 200, 255],
                },
            },
            CompositeNode {
                id: output,
                name: "RGB".into(),
                op: CompositeOp::Comic { source, style },
            },
        ],
    };
    batch(
        &mut s,
        vec![Command::SetCompositions {
            compositions: vec![graph(style.clone())],
        }],
    )
    .unwrap();
    assert_eq!(s.project.schema, 12);
    if let ComicStyle::RgbSplit { keys, .. } = &mut style {
        keys[1].frame = 0;
    }
    assert!(
        batch(
            &mut s,
            vec![Command::SetCompositions {
                compositions: vec![graph(style)]
            }]
        )
        .is_err()
    );
    assert_eq!(s.project.revision, 1);
    let mut cycle = s.project.compositions.clone();
    cycle[0].nodes[1].op = CompositeOp::Comic {
        source: output,
        style: ComicStyle::Halftone {
            spacing: 8.,
            angle: 0.,
            ink: [0, 0, 0, 255],
            strength: 1.,
            keys: vec![],
        },
    };
    assert!(
        batch(
            &mut s,
            vec![Command::SetCompositions {
                compositions: cycle
            }]
        )
        .is_err()
    );
}

#[test]
fn project_retime_maps_effect_keys_with_artwork() {
    let mut p = Project::new("Retime", 32, 32, 24, 24);
    p.schema = 12;
    let source = Id::new_v4();
    let output = Id::new_v4();
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Comic".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output,
        nodes: vec![
            CompositeNode {
                id: source,
                name: "Base".into(),
                op: CompositeOp::Solid {
                    color: [80, 100, 120, 255],
                },
            },
            CompositeNode {
                id: output,
                name: "Glitch".into(),
                op: CompositeOp::Comic {
                    source,
                    style: ComicStyle::Glitch {
                        displacement: 20.,
                        band_height: 4,
                        rate: 12,
                        seed: 42,
                        strength: 0.,
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
    let mut session = Session::new(p).unwrap();
    batch(
        &mut session,
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
    if let CompositeOp::Comic { style, .. } = &session.project.compositions[0].nodes[1].op {
        assert_eq!(style.controls().1[1].frame, 20);
    } else {
        panic!("Missing effect");
    }
}
