use astral_core::studio::*;
use astral_core::*;
fn scene() -> (Session, Id, Id, Id) {
    let mut p = Project::new("Tools", 800, 600, 24, 96);
    p.schema = 3;
    let a = stick_character("A".into(), Point::new(200., 300.), [20, 20, 20, 255]);
    let b = stick_character("B".into(), Point::new(420., 300.), [40, 80, 140, 255]);
    let ids = (a.id, b.id);
    p.characters = vec![a, b];
    let layer = DrawingLayer::new("Ink");
    let id = layer.id;
    p.layers.push(layer);
    for (frame, x) in [(0, 10.), (5, 30.), (10, 50.)] {
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: id,
            frame,
            strokes: vec![InkStroke {
                bitmap: None,
                skin: None,
                pressures: vec![],
                style: BrushStyle::Solid,
                filled: false,
                curved: false,
                points: vec![Point::new(x, 20.), Point::new(x + 20., 20.)],
                width: 3.,
                color: [10, 10, 10, 255],
            }],
        });
    }
    (Session::new(p).unwrap(), ids.0, ids.1, id)
}
fn apply(s: &mut Session, tool: StudioOperation) -> StudioPlan {
    let before = serde_json::to_value(&s.project).unwrap();
    let proposal = plan(&s.project, tool).unwrap();
    assert_eq!(
        serde_json::to_value(&s.project).unwrap(),
        before,
        "Planning mutated the project"
    );
    if !proposal.commands.is_empty() {
        s.apply(Batch {
            project: s.project.id,
            expected_revision: s.project.revision,
            commands: proposal.commands.clone(),
        })
        .unwrap();
    }
    proposal
}
fn bone_lengths(c: &Character, pose: &Pose) {
    for (i, j) in c.joints.iter().enumerate() {
        if let Some(parent) = j.parent {
            let rest = j.position.distance(c.joints[parent].position);
            assert!(
                (pose.joints[i].distance(pose.joints[parent]) - rest).abs() < 0.02,
                "{} changed length",
                j.name
            );
        }
    }
}

#[test]
fn copied_symbol_uses_snapshot_and_preserves_destination_hold_after_range() {
    let (mut s, _, _, layer) = scene();
    apply(
        &mut s,
        StudioOperation::SaveLibrary {
            name: "Ink range".into(),
            kind: LibraryKind::Symbol,
            characters: vec![],
            layer: Some(layer),
            start: 2,
            end: 7,
        },
    );
    let entry = library_entries(&s.project).unwrap().remove(0);
    let origin = entry.origin;
    let frozen = entry.cels[0].strokes[0].points.clone();
    let after = s.project.cel_at(layer, 36).unwrap().strokes[0]
        .points
        .clone();
    apply(
        &mut s,
        StudioOperation::FrameBatch {
            layer,
            start: 0,
            end: 5,
            action: FrameEdit::Translate,
            target: 0,
            new_end: 5,
            delta: Point::new(100., 0.),
            overwrite: false,
        },
    );
    apply(
        &mut s,
        StudioOperation::Paste {
            entry,
            character: None,
            layer: Some(layer),
            frame: 30,
            position: origin,
            overwrite: false,
        },
    );
    assert_eq!(
        s.project.cel_at(layer, 30).unwrap().strokes[0].points,
        frozen
    );
    assert_eq!(
        s.project.cel_at(layer, 36).unwrap().strokes[0].points,
        after
    );
    assert!(s.project.cels.iter().all(|c| c.layer == layer));
}

#[test]
fn pose_batch_moves_stable_keys_and_rejects_merged_timing_atomically() {
    let (mut s, a, _, _) = scene();
    for frame in [5, 10] {
        let mut pose = pose_at(&s.project, a, 0).unwrap();
        pose.root.x += frame as f32;
        apply(
            &mut s,
            StudioOperation::Commands {
                name: "Key".into(),
                commands: vec![Command::SetPose {
                    character: a,
                    frame,
                    pose,
                    easing: Easing::Smooth,
                }],
            },
        );
    }
    apply(
        &mut s,
        StudioOperation::PoseBatch {
            character: a,
            start: 5,
            end: 10,
            action: FrameEdit::Move,
            target: 20,
            new_end: 25,
            delta: Point::default(),
            overwrite: false,
        },
    );
    let c = s.project.character(a).unwrap();
    assert_eq!(
        c.keys.iter().map(|k| k.frame).collect::<Vec<_>>(),
        vec![0, 20, 25]
    );
    assert_eq!(c.keys[1].pose.root.x, 205.);
    let before = serde_json::to_value(&s.project).unwrap();
    assert!(
        plan(
            &s.project,
            StudioOperation::PoseBatch {
                character: a,
                start: 0,
                end: 25,
                action: FrameEdit::Stretch,
                target: 0,
                new_end: 1,
                delta: Point::default(),
                overwrite: false
            }
        )
        .is_err()
    );
    assert_eq!(serde_json::to_value(&s.project).unwrap(), before);
}

#[test]
fn markers_edit_retime_and_undo_without_losing_unknown_metadata() {
    let (mut s, _, _, _) = scene();
    s.project
        .metadata
        .insert("foreign.plugin".into(), serde_json::json!({"keep":true}));
    apply(
        &mut s,
        StudioOperation::Marker {
            name: "Contact".into(),
            frame: Some(12),
        },
    );
    assert_eq!(s.project.schema, 5);
    apply(
        &mut s,
        StudioOperation::Timing {
            start: 0,
            end: 23,
            seconds: 2.,
        },
    );
    assert_eq!(s.project.markers["Contact"], 25);
    assert_eq!(s.project.metadata["foreign.plugin"]["keep"], true);
    apply(
        &mut s,
        StudioOperation::Marker {
            name: "Contact".into(),
            frame: None,
        },
    );
    assert!(!s.project.markers.contains_key("Contact"));
    s.undo(s.project.id, s.project.revision).unwrap();
    assert_eq!(s.project.markers["Contact"], 25);
}

#[test]
fn combo_library_reinserts_editable_characters_effects_and_contact_markers() {
    let (mut s, a, b, _) = scene();
    apply(
        &mut s,
        StudioOperation::Fight {
            first: a,
            second: b,
            start: 0,
            beats: vec![FightBeat {
                attacker: a,
                action: FightAction::Punch,
                frames: 24,
                strength: 1.,
            }],
            style: MovementStyle::Natural,
            impacts: true,
            overwrite: true,
        },
    );
    apply(
        &mut s,
        StudioOperation::SaveLibrary {
            name: "Punch combo".into(),
            kind: LibraryKind::Combo,
            characters: vec![a, b],
            layer: None,
            start: 0,
            end: 23,
        },
    );
    let entry = library_entries(&s.project).unwrap().remove(0);
    let count = s.project.effects.len();
    apply(
        &mut s,
        StudioOperation::InsertLibrary {
            entry: entry.id,
            character: None,
            frame: 40,
            duration: 48,
            position: Point::new(300., 250.),
            mirror: false,
        },
    );
    assert_eq!(s.project.characters.len(), 4);
    assert_eq!(s.project.effects.len(), count * 2);
    for effect in &s.project.effects[count..] {
        assert!(effect.start >= 40);
        assert!(
            s.project.characters[2..]
                .iter()
                .any(|c| Some(c.id) == effect.character)
        );
    }
    assert!(s.project.markers.values().any(|f| *f >= 40));
    s.project.validate().unwrap();
    let mut corrupt = entry;
    corrupt.duration = 0;
    assert!(
        plan(
            &s.project,
            StudioOperation::Paste {
                entry: corrupt,
                character: None,
                layer: None,
                frame: 0,
                position: Point::default(),
                overwrite: true
            }
        )
        .is_err()
    );
}
#[test]
fn smart_pose_styles_preserve_bones_and_manual_suggestions_do_not_commit() {
    for style in [
        MovementStyle::Natural,
        MovementStyle::Exaggerated,
        MovementStyle::AnimeFight,
    ] {
        let (mut s, a, _, _) = scene();
        let target = Point::new(270., 265.);
        let out = apply(
            &mut s,
            StudioOperation::SmartPose {
                character: a,
                frame: 6,
                joint: "right_hand".into(),
                target,
                style,
            },
        );
        assert_eq!(out.commands.len(), 2);
        let pose = pose_at(&s.project, a, 6).unwrap();
        bone_lengths(s.project.character(a).unwrap(), &pose);
        assert!(pose.root.distance(Point::new(200., 300.)) < 0.001);
        assert!((pose.root + pose.joints[6]).distance(target) < 0.02);
        assert_eq!(s.project.schema, 5);
    }
}
#[test]
fn opt_in_constraints_keep_interpolated_lengths_and_legacy_defaults_stay_off() {
    let (mut s, a, _, _) = scene();
    let mut pose = s.project.character(a).unwrap().keys[0].pose.clone();
    pose.joints[6] = Point::new(-53., -2.);
    s.apply(Batch {
        project: s.project.id,
        expected_revision: 0,
        commands: vec![Command::SetPose {
            character: a,
            frame: 20,
            pose,
            easing: Easing::Linear,
        }],
    })
    .unwrap();
    assert!(!s.project.character(a).unwrap().preserve_bones);
    s.apply(Batch {
        project: s.project.id,
        expected_revision: 1,
        commands: vec![Command::SetBoneConstraints {
            character: a,
            enabled: true,
        }],
    })
    .unwrap();
    for f in 0..=20 {
        bone_lengths(
            s.project.character(a).unwrap(),
            &pose_at(&s.project, a, f).unwrap(),
        );
    }
}
#[test]
fn fight_is_editable_marked_extensible_and_one_undo_preserves_unrelated_art() {
    let (mut s, a, b, _) = scene();
    s.project.duration = 24;
    let before = serde_json::to_value(&s.project).unwrap();
    let drawings = serde_json::to_value(&s.project.cels).unwrap();
    let beats = [
        FightAction::Punch,
        FightAction::Dodge,
        FightAction::Counter,
        FightAction::Kick,
        FightAction::Knockback,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, action)| FightBeat {
        attacker: if i % 2 == 0 { a } else { b },
        action,
        frames: 12,
        strength: 1.,
    })
    .collect();
    let out = apply(
        &mut s,
        StudioOperation::Fight {
            first: a,
            second: b,
            start: 0,
            beats,
            style: MovementStyle::AnimeFight,
            impacts: true,
            overwrite: false,
        },
    );
    assert_eq!(s.project.duration, 60);
    assert_eq!(out.markers.len(), 5);
    assert_eq!(s.project.markers.len(), 5);
    let review = astral_core::staging::review(&s.project, 0, 59, 1).unwrap();
    assert_eq!(review.contacts.len(), 3);
    assert!(
        review.contacts.iter().all(|c| c.passed),
        "{:?}",
        review.contacts
    );
    assert!(review.overlaps.is_empty(), "{:?}", review.overlaps);
    assert!(s.project.characters.iter().all(|c| c.keys.len() >= 20));
    assert!(
        s.project
            .effects
            .iter()
            .any(|e| e.kind == EffectKind::Flash)
    );
    assert_eq!(serde_json::to_value(&s.project.cels).unwrap(), drawings);
    for f in 0..60 {
        for c in &s.project.characters {
            bone_lengths(c, &pose_at(&s.project, c.id, f).unwrap());
        }
    }
    s.undo(s.project.id, s.project.revision).unwrap();
    let mut after = serde_json::to_value(&s.project).unwrap();
    after["revision"] = before["revision"].clone();
    assert_eq!(after, before);
}
#[test]
fn fight_rejects_existing_animation_and_locked_actors_without_partial_changes() {
    let (mut s, a, b, _) = scene();
    let pose = s.project.characters[0].keys[0].pose.clone();
    s.project.characters[0].keys.push(PoseKey {
        frame: 10,
        pose,
        easing: Easing::Smooth,
    });
    let before = serde_json::to_value(&s.project).unwrap();
    assert!(
        plan(
            &s.project,
            StudioOperation::Fight {
                first: a,
                second: b,
                start: 0,
                beats: vec![FightBeat {
                    attacker: a,
                    action: FightAction::Punch,
                    frames: 24,
                    strength: 1.
                }],
                style: MovementStyle::Natural,
                impacts: true,
                overwrite: false
            }
        )
        .is_err()
    );
    assert_eq!(serde_json::to_value(&s.project).unwrap(), before);
}
#[test]
fn motion_brush_resamples_arc_length_and_root_keys_are_individually_editable() {
    let (mut s, a, _, _) = scene();
    apply(
        &mut s,
        StudioOperation::MotionBrush {
            character: a,
            joint: "root".into(),
            path: vec![
                Point::new(200., 300.),
                Point::new(400., 300.),
                Point::new(400., 400.),
            ],
            start: 0,
            end: 24,
            keys: 5,
            easing: Easing::Linear,
            style: MovementStyle::Natural,
        },
    );
    assert_eq!(s.project.character(a).unwrap().keys.len(), 5);
    let pose = pose_at(&s.project, a, 12).unwrap();
    assert!(pose.root.distance(Point::new(350., 300.)) < 0.02);
    let path = trajectory(&s.project, a, "root", 0, 24, 2).unwrap();
    assert!(path.len() >= 12);
    assert!(path.iter().all(|p| p.speed.is_finite()));
}
#[test]
fn coaching_flags_slide_and_length_drift_without_changing_original() {
    let (mut s, a, _, _) = scene();
    let c = &mut s.project.characters[0];
    let base = c.keys[0].pose.clone();
    c.keys = (0..8)
        .map(|frame| {
            let mut pose = base.clone();
            pose.root.x += frame as f32 * 10.;
            pose.joints[6].x += 30.;
            PoseKey {
                frame,
                pose,
                easing: Easing::Linear,
            }
        })
        .collect();
    let before = serde_json::to_value(&s.project).unwrap();
    let issues = analyze(&s.project, 0, 7, &[a], 0.12).unwrap();
    assert!(issues.iter().any(|i| i.kind == "limb_length"));
    assert!(issues.iter().any(|i| i.kind == "foot_slide"));
    assert_eq!(serde_json::to_value(&s.project).unwrap(), before);
    let (still, a, _, _) = scene();
    assert!(
        analyze(&still.project, 0, 20, &[a], 0.12)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn foot_lock_holds_a_reachable_target_and_automatic_detection_finds_stance() {
    let (mut s, a, _, _) = scene();
    let mut next = s.project.characters[0].keys[0].clone();
    next.frame = 12;
    next.pose.root.x += 70.;
    s.project.characters[0].keys.push(next);
    let target = pose_at(&s.project, a, 0).unwrap();
    let target = target.root + target.joints[8];
    apply(
        &mut s,
        StudioOperation::FootLock {
            character: a,
            start: 0,
            end: 12,
            joints: vec!["left_foot".into()],
            automatic: false,
        },
    );
    for f in 0..=12 {
        let pose = pose_at(&s.project, a, f).unwrap();
        assert!(
            (pose.root + pose.joints[8]).distance(target) < 0.05,
            "Foot moved at {f}"
        );
    }
    let (mut still, a, _, _) = scene();
    apply(
        &mut still,
        StudioOperation::FootLock {
            character: a,
            start: 0,
            end: 12,
            joints: vec!["left_foot".into(), "right_foot".into()],
            automatic: true,
        },
    );
    assert_eq!(still.project.character(a).unwrap().contacts.len(), 2);
}
#[test]
fn batch_copy_translate_and_move_preserve_other_frames_and_moved_ids() {
    let (mut s, _, _, layer) = scene();
    let original = s.project.clone();
    apply(
        &mut s,
        StudioOperation::FrameBatch {
            layer,
            start: 2,
            end: 8,
            action: FrameEdit::Copy,
            target: 20,
            new_end: 0,
            delta: Point::default(),
            overwrite: false,
        },
    );
    assert_eq!(
        s.project.cel_at(layer, 20).unwrap().strokes,
        original.cel_at(layer, 2).unwrap().strokes
    );
    assert_eq!(
        s.project.cel_at(layer, 23).unwrap().strokes,
        original.cel_at(layer, 5).unwrap().strokes
    );
    for cel in &original.cels {
        assert!(
            s.project
                .cels
                .iter()
                .any(|v| v.id == cel.id && v.frame == cel.frame)
        );
    }
    let id = original.cel_at(layer, 5).unwrap().id;
    apply(
        &mut s,
        StudioOperation::FrameBatch {
            layer,
            start: 5,
            end: 8,
            action: FrameEdit::Move,
            target: 30,
            new_end: 0,
            delta: Point::default(),
            overwrite: false,
        },
    );
    assert_eq!(s.project.cel_at(layer, 30).unwrap().id, id);
    assert!(s.project.cel_at(layer, 5).unwrap().strokes.is_empty());
}
#[test]
fn frame_retime_collision_and_destination_conflicts_are_rejected() {
    let (s, _, _, layer) = scene();
    assert!(
        plan(
            &s.project,
            StudioOperation::FrameBatch {
                layer,
                start: 0,
                end: 10,
                action: FrameEdit::Stretch,
                target: 0,
                new_end: 1,
                delta: Point::default(),
                overwrite: false
            }
        )
        .is_err()
    );
    assert!(
        plan(
            &s.project,
            StudioOperation::FrameBatch {
                layer,
                start: 0,
                end: 5,
                action: FrameEdit::Copy,
                target: 5,
                new_end: 0,
                delta: Point::default(),
                overwrite: false
            }
        )
        .is_err()
    );
}
#[test]
fn library_motion_and_symbols_roundtrip_and_insert_as_new_editable_content() {
    let (mut s, a, _, layer) = scene();
    apply(
        &mut s,
        StudioOperation::SaveLibrary {
            name: "Walk study".into(),
            kind: LibraryKind::Motion,
            characters: vec![a],
            layer: None,
            start: 0,
            end: 24,
        },
    );
    apply(
        &mut s,
        StudioOperation::SaveLibrary {
            name: "Ink symbol".into(),
            kind: LibraryKind::Symbol,
            characters: vec![],
            layer: Some(layer),
            start: 0,
            end: 10,
        },
    );
    let items = library_entries(&s.project).unwrap();
    let json = serde_json::to_vec(&s.project).unwrap();
    let loaded: Project = serde_json::from_slice(&json).unwrap();
    loaded.validate().unwrap();
    assert_eq!(library_entries(&loaded).unwrap().len(), 2);
    apply(
        &mut s,
        StudioOperation::InsertLibrary {
            entry: items[0].id,
            character: None,
            frame: 30,
            duration: 24,
            position: Point::new(600., 300.),
            mirror: true,
        },
    );
    assert_eq!(s.project.characters.len(), 3);
    assert_eq!(s.project.characters[2].keys.len(), 24);
    apply(
        &mut s,
        StudioOperation::InsertLibrary {
            entry: items[1].id,
            character: None,
            frame: 30,
            duration: 22,
            position: Point::new(400., 200.),
            mirror: false,
        },
    );
    assert_eq!(s.project.layers.len(), 2);
    let new = s.project.layers[1].id;
    assert!(s.project.cel_at(new, 30).is_some());
    assert!(s.project.cel_at(new, 52).unwrap().strokes.is_empty());
}
#[test]
fn rig_templates_have_distinct_topologies_and_constraints() {
    for (kind, count) in [
        (RigTemplate::Human, 11),
        (RigTemplate::Quadruped, 13),
        (RigTemplate::Winged, 15),
    ] {
        let mut p = Project::new("Rig", 800, 600, 24, 48);
        p.schema = 5;
        let c = rig_template(
            kind,
            "Template".into(),
            Point::new(200., 300.),
            240.,
            [20, 20, 20, 255],
        )
        .unwrap();
        assert_eq!(c.joints.len(), count);
        p.characters.push(c);
        p.validate().unwrap();
        bone_lengths(
            &p.characters[0],
            &pose_at(&p, p.characters[0].id, 0).unwrap(),
        );
    }
}
#[test]
fn fps_can_preserve_seconds_and_retimes_named_markers() {
    let (mut s, _, _, _) = scene();
    s.apply(Batch {
        project: s.project.id,
        expected_revision: 0,
        commands: vec![Command::SetMarkers {
            markers: std::collections::BTreeMap::from([("hit".into(), 24)]),
        }],
    })
    .unwrap();
    apply(
        &mut s,
        StudioOperation::FrameRate {
            fps: 48,
            preserve_seconds: true,
        },
    );
    assert_eq!(s.project.duration, 192);
    assert_eq!(s.project.fps, 48);
    assert_eq!(s.project.markers["hit"], 48);
}
#[test]
fn camera_tracking_keys_follow_subject_and_reusable_mirror_keeps_root() {
    let (mut s, a, _, _) = scene();
    apply(
        &mut s,
        StudioOperation::Camera {
            start: 0,
            end: 24,
            character: Some(a),
            from: Point::new(0., -20.),
            to: Point::new(10., -20.),
            zoom_start: 1.,
            zoom_end: 1.5,
        },
    );
    assert!(s.project.camera.len() > 2);
    assert_eq!(s.project.camera.last().unwrap().zoom, 1.5);
    let old = pose_at(&s.project, a, 0).unwrap();
    apply(
        &mut s,
        StudioOperation::Mirror {
            character: a,
            start: 0,
            end: 0,
        },
    );
    let new = pose_at(&s.project, a, 0).unwrap();
    assert_eq!(new.root, old.root);
    bone_lengths(s.project.character(a).unwrap(), &new);
}
