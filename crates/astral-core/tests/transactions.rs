use astral_core::*;

#[test]
fn shared_bitmap_storage_does_not_consume_the_undo_budget_per_edit() {
    let mut project = Project::new("Shared artwork", 64, 64, 24, 48);
    project.schema = 8;
    // Core validates metadata; image decoding is independently tested in the renderer.
    project.bitmaps.push(Bitmap {
        id: Id::new_v4(),
        name: "Shared payload budget fixture".into(),
        width: 1,
        height: 1,
        png: std::sync::Arc::from("A".repeat(2 * 1024 * 1024)),
    });
    let mut session = Session::new(project).unwrap();
    for i in 0..40 {
        session
            .apply(Batch {
                project: session.project.id,
                expected_revision: session.project.revision,
                commands: vec![Command::SetProject {
                    name: Some(format!("Edit {i}")),
                    duration: None,
                    background: None,
                }],
            })
            .unwrap();
    }
    for _ in 0..40 {
        session
            .undo(session.project.id, session.project.revision)
            .unwrap();
    }
    assert_eq!(session.project.name, "Shared artwork");
}
use std::collections::BTreeMap;

fn session() -> Session {
    Session::new(fight_demo().unwrap()).unwrap()
}
fn batch(s: &Session, commands: Vec<Command>) -> Batch {
    Batch {
        project: s.project.id,
        expected_revision: s.project.revision,
        commands,
    }
}
fn encoded(s: &Session) -> Vec<u8> {
    serde_json::to_vec(&s.project).unwrap()
}

#[test]
fn invalid_batch_rolls_back_every_command() {
    let mut s = session();
    let before = encoded(&s);
    let b = batch(
        &s,
        vec![
            Command::AddCharacter {
                name: "Temporary".into(),
                position: Point::default(),
                color: [255; 4],
            },
            Command::Remove { id: Id::new_v4() },
        ],
    );
    assert!(s.apply(b).is_err());
    assert_eq!(before, encoded(&s));
}
#[test]
fn stale_and_cross_project_mutations_are_rejected() {
    let mut s = session();
    let mut b = batch(
        &s,
        vec![Command::SetProject {
            name: Some("Changed".into()),
            duration: None,
            background: None,
        }],
    );
    b.expected_revision += 1;
    assert!(s.apply(b.clone()).is_err());
    b.expected_revision -= 1;
    b.project = Id::new_v4();
    assert!(s.apply(b).is_err());
}
#[test]
fn undo_redo_revisions_are_monotonic() {
    let mut s = session();
    let before = s.project.name.clone();
    let b = batch(
        &s,
        vec![Command::SetProject {
            name: Some("Changed".into()),
            duration: None,
            background: None,
        }],
    );
    s.apply(b).unwrap();
    let r = s.project.revision;
    s.undo(s.project.id, r).unwrap();
    assert_eq!(s.project.name, before);
    assert_eq!(s.project.revision, r + 1);
    s.redo(s.project.id, r + 1).unwrap();
    assert_eq!(s.project.name, "Changed");
    assert_eq!(s.project.revision, r + 2);
}
#[test]
fn new_edit_clears_redo() {
    let mut s = session();
    let b = batch(
        &s,
        vec![Command::SetProject {
            name: Some("A".into()),
            duration: None,
            background: None,
        }],
    );
    s.apply(b).unwrap();
    s.undo(s.project.id, s.project.revision).unwrap();
    let b = batch(
        &s,
        vec![Command::SetProject {
            name: Some("B".into()),
            duration: None,
            background: None,
        }],
    );
    s.apply(b).unwrap();
    assert!(s.redo(s.project.id, s.project.revision).is_err());
}
#[test]
fn deleting_character_removes_dependent_actions() {
    let mut s = session();
    let id = s.project.characters[0].id;
    let b = batch(&s, vec![Command::Remove { id }]);
    s.apply(b).unwrap();
    assert!(s.project.character(id).is_none());
    assert!(!s.project.actions.iter().any(|a| a.character == id));
    s.project.validate().unwrap();
}
#[test]
fn overlapping_action_is_rejected_without_change() {
    let mut s = session();
    let mut a = s.project.actions[0].clone();
    a.id = Id::new_v4();
    let before = encoded(&s);
    let b = batch(&s, vec![Command::ApplyAction { clip: a }]);
    assert!(s.apply(b).is_err());
    assert_eq!(before, encoded(&s));
}
#[test]
fn replace_action_leaves_unrelated_tracks_identical() {
    let mut s = session();
    let before = s.project.clone();
    let mut a = s.project.actions[1].clone();
    a.intensity = 2.;
    let id = a.id;
    let b = batch(&s, vec![Command::ReplaceAction { clip: a }]);
    s.apply(b).unwrap();
    assert_eq!(
        serde_json::to_value(&s.project.effects).unwrap(),
        serde_json::to_value(before.effects).unwrap()
    );
    for a in &s.project.actions {
        if a.id != id {
            assert_eq!(
                serde_json::to_value(a).unwrap(),
                serde_json::to_value(before.actions.iter().find(|v| v.id == a.id).unwrap())
                    .unwrap()
            );
        }
    }
}
#[test]
fn visible_pose_key_on_action_does_not_double_action() {
    let mut s = session();
    let id = s.project.characters[0].id;
    let target = evaluate(&s.project, 104.).characters[0].pose.clone();
    let b = batch(
        &s,
        vec![Command::SetPose {
            character: id,
            frame: 104,
            pose: target.clone(),
            easing: Easing::Linear,
        }],
    );
    s.apply(b).unwrap();
    let actual = &evaluate(&s.project, 104.).characters[0].pose;
    assert!(actual.root.distance(target.root) < 0.001);
    for (a, b) in actual.joints.iter().zip(&target.joints) {
        assert!(a.distance(*b) < 0.001);
    }
}
#[test]
fn failed_rig_does_not_publish() {
    let mut s = session();
    let mut c = stick_character("Invalid".into(), Point::default(), [255; 4]);
    c.joints[0].parent = Some(2);
    let before = encoded(&s);
    let b = batch(&s, vec![Command::AddRig { character: c }]);
    assert!(s.apply(b).is_err());
    assert_eq!(before, encoded(&s));
}
#[test]
fn invalid_pose_shape_and_nan_are_rejected() {
    let mut s = session();
    let id = s.project.characters[0].id;
    let b = batch(
        &s,
        vec![Command::SetPose {
            character: id,
            frame: 0,
            pose: Pose {
                root: Point::new(f32::NAN, 0.),
                joints: vec![],
            },
            easing: Easing::Linear,
        }],
    );
    assert!(s.apply(b).is_err());
}
#[test]
fn locked_characters_require_unlock() {
    let mut s = session();
    let id = s.project.characters[0].id;
    let b = batch(
        &s,
        vec![Command::SetCharacter {
            character: id,
            name: None,
            color: None,
            visible: None,
            locked: Some(true),
            secondary_motion: None,
        }],
    );
    s.apply(b).unwrap();
    let b = batch(&s, vec![Command::Remove { id }]);
    assert!(s.apply(b).is_err());
}
#[test]
fn pose_assets_retarget_by_joint_name() {
    let mut s = session();
    let id = s.project.characters[0].id;
    let b = batch(
        &s,
        vec![Command::SavePose {
            character: id,
            frame: 100,
            name: "Strike".into(),
        }],
    );
    s.apply(b).unwrap();
    let pose = s.project.poses[0].id;
    let b = batch(
        &s,
        vec![Command::ApplyPose {
            character: s.project.characters[1].id,
            pose,
            frame: 105,
            blend: 0.5,
            joints: vec!["left_hand".into()],
            mirror: true,
        }],
    );
    s.apply(b).unwrap();
    s.project.validate().unwrap();
}
#[test]
fn short_retime_rejects_key_collisions() {
    let mut s = session();
    let id = s.project.characters[0].id;
    let pose = s.project.characters[0].keys[0].pose.clone();
    let b = batch(
        &s,
        vec![
            Command::SetPose {
                character: id,
                frame: 1,
                pose: pose.clone(),
                easing: Easing::Linear,
            },
            Command::SetPose {
                character: id,
                frame: 2,
                pose,
                easing: Easing::Linear,
            },
        ],
    );
    s.apply(b).unwrap();
    let before = encoded(&s);
    let b = batch(
        &s,
        vec![Command::Retime {
            start: 0,
            end: 299,
            new_end: 1,
        }],
    );
    assert!(s.apply(b).is_err());
    assert_eq!(before, encoded(&s));
}
#[test]
fn evaluation_is_deterministic_and_translation_persists() {
    let p = fight_demo().unwrap();
    assert_eq!(
        serde_json::to_value(evaluate(&p, 104.)).unwrap(),
        serde_json::to_value(evaluate(&p, 104.)).unwrap()
    );
    let final_pose = evaluate(&p, 299.);
    assert!(final_pose.characters[0].pose.root.x > 850.);
}
#[test]
fn two_bone_contact_preserves_lengths() {
    let (root, target) = (Point::new(0., 0.), Point::new(60., 30.));
    let (mid, end) = solve_two_bone(root, target, 50., 40., 1.);
    assert!((root.distance(mid) - 50.).abs() < 0.001);
    assert!((mid.distance(end) - 40.).abs() < 0.001);
    assert!(end.distance(target) < 0.001);
}
#[test]
fn duplicate_ids_and_unsafe_asset_paths_fail_validation() {
    let mut p = fight_demo().unwrap();
    let mut c = p.characters[0].clone();
    c.name = "Duplicate".into();
    p.characters.push(c);
    assert!(p.validate().is_err());
    p.characters.pop();
    p.assets.push(Asset {
        id: Id::new_v4(),
        name: "Unsafe".into(),
        path: "../secret.wav".into(),
        kind: "audio".into(),
        tags: vec![],
    });
    assert!(p.validate().is_err());
}
#[test]
fn project_truncation_rejects_out_of_range_content() {
    let mut s = session();
    let before = encoded(&s);
    let b = batch(
        &s,
        vec![Command::SetProject {
            name: None,
            duration: Some(10),
            background: None,
        }],
    );
    assert!(s.apply(b).is_err());
    assert_eq!(before, encoded(&s));
}
#[test]
fn custom_bezier_has_correct_endpoints() {
    let easing = Easing::Bezier {
        x1: 0.3,
        y1: 0.,
        x2: 0.7,
        y2: 1.,
    };
    assert!(ease(&easing, 0.).abs() < 0.0001);
    assert!((ease(&easing, 1.) - 1.).abs() < 0.0001);
    assert!((ease(&easing, 0.5) - 0.5).abs() < 0.0001);
}
#[test]
fn action_markers_must_stay_in_clip_range() {
    let mut s = session();
    let mut clip = s.project.actions[0].clone();
    clip.markers = BTreeMap::from([("invalid".into(), 9999)]);
    let b = batch(&s, vec![Command::ReplaceAction { clip }]);
    assert!(s.apply(b).is_err());
}
