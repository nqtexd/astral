use astral_core::*;
fn apply(s: &mut Session, commands: Vec<Command>) -> anyhow::Result<Receipt> {
    s.apply(Batch {
        project: s.project.id,
        expected_revision: s.project.revision,
        commands,
    })
}
fn fixture() -> (Session, Id) {
    let mut s = Session::new(Project::new("Frames", 100, 100, 24, 24)).unwrap();
    let layer = DrawingLayer::new("Ink");
    let id = layer.id;
    apply(
        &mut s,
        vec![
            Command::AddLayer { layer },
            Command::SetCel {
                layer: id,
                frame: 0,
                strokes: vec![ink(0.)],
            },
        ],
    )
    .unwrap();
    (s, id)
}
fn ink(x: f32) -> InkStroke {
    InkStroke {
        brush: None,
        bitmap: None,
        skin: None,
        pressures: vec![],
        style: BrushStyle::Solid,
        filled: false,
        curved: false,
        points: vec![Point::new(10. + x, 10.), Point::new(30. + x, 20.)],
        color: [20, 20, 20, 255],
        width: 3.,
    }
}
#[test]
fn single_frame_ink_does_not_fill_the_rest_of_the_timeline() {
    let mut s = Session::new(Project::new("Manual", 100, 100, 24, 24)).unwrap();
    let layer = DrawingLayer::new("Ink");
    let id = layer.id;
    apply(
        &mut s,
        vec![
            Command::AddLayer { layer },
            Command::SetFrameDrawing {
                layer: id,
                frame: 0,
                strokes: vec![ink(0.)],
            },
        ],
    )
    .unwrap();
    assert_eq!(s.project.cel_at(id, 0).unwrap().strokes.len(), 1);
    for frame in 1..24 {
        assert!(s.project.cel_at(id, frame).unwrap().strokes.is_empty());
    }
    apply(
        &mut s,
        vec![Command::SetFrameDrawing {
            layer: id,
            frame: 1,
            strokes: vec![ink(40.)],
        }],
    )
    .unwrap();
    assert_ne!(
        s.project.cel_at(id, 0).unwrap().strokes,
        s.project.cel_at(id, 1).unwrap().strokes
    );
    assert!(s.project.cel_at(id, 2).unwrap().strokes.is_empty());
    s.undo(s.project.id, s.project.revision).unwrap();
    assert!(s.project.cel_at(id, 1).unwrap().strokes.is_empty());
}

#[test]
fn isolated_edit_restores_a_legacy_hold_and_preserves_next_authored_key() {
    let (mut s, id) = fixture();
    let original = s.project.cel_at(id, 0).unwrap().strokes.clone();
    apply(
        &mut s,
        vec![
            Command::SetCel {
                layer: id,
                frame: 7,
                strokes: vec![ink(60.)],
            },
            Command::SetFrameDrawing {
                layer: id,
                frame: 5,
                strokes: vec![ink(30.)],
            },
        ],
    )
    .unwrap();
    assert_eq!(s.project.cel_at(id, 4).unwrap().strokes, original);
    assert_eq!(s.project.cel_at(id, 6).unwrap().strokes, original);
    assert_eq!(s.project.cel_at(id, 7).unwrap().strokes, vec![ink(60.)]);
    apply(
        &mut s,
        vec![Command::SetFrameDrawing {
            layer: id,
            frame: 23,
            strokes: vec![ink(80.)],
        }],
    )
    .unwrap();
    assert!(s.project.cels.iter().all(|c| c.frame < 24));
}
#[test]
fn editing_a_held_drawing_creates_an_independent_cel() {
    let (mut s, layer) = fixture();
    let original = s.project.cel_at(layer, 0).unwrap().id;
    let mut strokes = s.project.cel_at(layer, 5).unwrap().strokes.clone();
    strokes.push(ink(20.));
    apply(
        &mut s,
        vec![Command::SetCel {
            layer,
            frame: 5,
            strokes,
        }],
    )
    .unwrap();
    assert_eq!(s.project.cel_at(layer, 4).unwrap().strokes.len(), 1);
    assert_eq!(s.project.cel_at(layer, 5).unwrap().strokes.len(), 2);
    assert_eq!(s.project.cel_at(layer, 0).unwrap().id, original);
    assert_ne!(s.project.cel_at(layer, 5).unwrap().id, original);
    assert_eq!(s.project.schema, 3);
}
#[test]
fn blanks_stop_exposure_and_undo_restores_the_drawing() {
    let (mut s, layer) = fixture();
    apply(
        &mut s,
        vec![Command::SetCel {
            layer,
            frame: 4,
            strokes: vec![],
        }],
    )
    .unwrap();
    assert_eq!(s.project.cel_at(layer, 3).unwrap().strokes.len(), 1);
    assert!(s.project.cel_at(layer, 20).unwrap().strokes.is_empty());
    s.undo(s.project.id, s.project.revision).unwrap();
    assert_eq!(s.project.cel_at(layer, 20).unwrap().strokes.len(), 1);
}
#[test]
fn layer_lock_protects_cels_deletion_and_retiming() {
    let (mut s, layer) = fixture();
    apply(
        &mut s,
        vec![Command::SetLayer {
            layer,
            name: None,
            visible: None,
            locked: Some(true),
            opacity: None,
        }],
    )
    .unwrap();
    let before = serde_json::to_value(&s.project).unwrap();
    for command in [
        Command::SetCel {
            layer,
            frame: 1,
            strokes: vec![],
        },
        Command::DeleteCel { layer, frame: 0 },
        Command::ShiftCels {
            layer,
            start: 0,
            delta: 1,
        },
        Command::Remove { id: layer },
        Command::Retime {
            start: 0,
            end: 10,
            new_end: 12,
        },
    ] {
        assert!(apply(&mut s, vec![command]).is_err());
        assert_eq!(serde_json::to_value(&s.project).unwrap(), before);
    }
}
#[test]
fn exposure_moves_later_keys_on_only_the_selected_layer() {
    let (mut s, layer) = fixture();
    let other = DrawingLayer::new("Other");
    let other_id = other.id;
    apply(
        &mut s,
        vec![
            Command::SetCel {
                layer,
                frame: 4,
                strokes: vec![ink(10.)],
            },
            Command::SetCel {
                layer,
                frame: 22,
                strokes: vec![],
            },
            Command::AddLayer { layer: other },
            Command::SetCel {
                layer: other_id,
                frame: 7,
                strokes: vec![ink(5.)],
            },
        ],
    )
    .unwrap();
    apply(
        &mut s,
        vec![Command::SetExposure {
            layer,
            frame: 0,
            frames: 8,
        }],
    )
    .unwrap();
    assert!(
        s.project
            .cels
            .iter()
            .any(|c| c.layer == layer && c.frame == 8)
    );
    assert!(
        s.project
            .cels
            .iter()
            .any(|c| c.layer == layer && c.frame == 26)
    );
    assert_eq!(s.project.duration, 27);
    assert_eq!(s.project.cel_at(other_id, 8).unwrap().frame, 7);
}
#[test]
fn shortened_last_exposure_inserts_an_explicit_blank() {
    let (mut s, layer) = fixture();
    apply(
        &mut s,
        vec![Command::SetExposure {
            layer,
            frame: 0,
            frames: 3,
        }],
    )
    .unwrap();
    assert!(!s.project.cel_at(layer, 2).unwrap().strokes.is_empty());
    assert!(s.project.cel_at(layer, 3).unwrap().strokes.is_empty());
    assert_eq!(s.project.duration, 24);
}
#[test]
fn collision_and_invalid_geometry_roll_back_the_batch() {
    let (mut s, layer) = fixture();
    apply(
        &mut s,
        vec![Command::SetCel {
            layer,
            frame: 2,
            strokes: vec![],
        }],
    )
    .unwrap();
    let revision = s.project.revision;
    assert!(
        apply(
            &mut s,
            vec![Command::ShiftCels {
                layer,
                start: 2,
                delta: -2
            }]
        )
        .is_err()
    );
    let mut bad = ink(0.);
    bad.points[0].x = f32::NAN;
    assert!(
        apply(
            &mut s,
            vec![Command::SetCel {
                layer,
                frame: 3,
                strokes: vec![bad]
            }]
        )
        .is_err()
    );
    assert_eq!(s.project.revision, revision);
    assert!(
        apply(
            &mut s,
            vec![Command::SetExposure {
                layer,
                frame: 0,
                frames: 1_000_000
            }]
        )
        .is_err()
    );
}
#[test]
fn deleting_a_layer_and_undo_preserve_all_cel_data() {
    let (mut s, layer) = fixture();
    let before = serde_json::to_value(&s.project.cels).unwrap();
    apply(&mut s, vec![Command::Remove { id: layer }]).unwrap();
    assert!(s.project.cels.is_empty());
    assert!(s.project.layers.is_empty());
    s.undo(s.project.id, s.project.revision).unwrap();
    assert_eq!(serde_json::to_value(&s.project.cels).unwrap(), before);
}
#[test]
fn older_projects_default_to_no_cels_without_data_changes() {
    for schema in [1, 2] {
        let mut value = serde_json::to_value(Project::new("Old", 100, 100, 24, 24)).unwrap();
        value["schema"] = schema.into();
        value.as_object_mut().unwrap().remove("layers");
        value.as_object_mut().unwrap().remove("cels");
        let p: Project = serde_json::from_value(value).unwrap();
        p.validate().unwrap();
        assert!(p.layers.is_empty() && p.cels.is_empty());
    }
}
#[test]
fn moving_a_key_keeps_its_identity_and_refuses_to_overwrite() {
    let (mut s, layer) = fixture();
    let id = s.project.cel_at(layer, 0).unwrap().id;
    apply(
        &mut s,
        vec![
            Command::SetCel {
                layer,
                frame: 5,
                strokes: vec![],
            },
            Command::MoveCel {
                layer,
                from: 0,
                to: 2,
            },
        ],
    )
    .unwrap();
    assert_eq!(s.project.cel_at(layer, 2).unwrap().id, id);
    let revision = s.project.revision;
    assert!(
        apply(
            &mut s,
            vec![Command::MoveCel {
                layer,
                from: 2,
                to: 5
            }]
        )
        .is_err()
    );
    assert_eq!(s.project.revision, revision);
    assert_eq!(s.project.cel_at(layer, 2).unwrap().id, id);
}
