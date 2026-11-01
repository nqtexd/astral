use astral_core::*;

fn apply(s: &mut Session, commands: Vec<Command>) -> anyhow::Result<Receipt> {
    s.apply(Batch {
        project: s.project.id,
        expected_revision: s.project.revision,
        commands,
    })
}
fn posed() -> Session {
    let mut p = Project::new("Learn", 960, 540, 30, 120);
    let mut c = stick_character("Actor".into(), Point::new(200., 300.), [220, 200, 255, 255]);
    let base = c.keys[0].pose.clone();
    c.keys.clear();
    for (frame, angle) in [(0, -0.8f32), (15, 0.4), (30, -0.8)] {
        let mut pose = base.clone();
        pose.root.y -= if frame == 15 { 80. } else { 0. };
        pose.joints[6] = pose.joints[5] + Point::new(angle.cos() * 40., angle.sin() * 40.);
        c.keys.push(PoseKey {
            frame,
            pose,
            easing: Easing::Linear,
        });
    }
    p.characters.push(c);
    Session::new(p).unwrap()
}
fn strokes(s: &mut Session) -> Vec<Id> {
    let mut sources = vec![];
    for (frame, dy) in [(0, 0.), (15, -70.), (30, 0.)] {
        let d = Drawing {
            id: Id::new_v4(),
            name: "Outline".into(),
            start: frame,
            end: 119,
            points: vec![
                Point::new(100., 200. + dy),
                Point::new(150., 170. + dy),
                Point::new(210., 200. + dy),
            ],
            color: [220, 240, 255, 255],
            width: 4.,
        };
        sources.push(d.id);
        apply(s, vec![Command::AddDrawing { drawing: d }]).unwrap();
    }
    sources
}

#[test]
fn learned_motion_keeps_endpoints_and_limb_lengths() {
    let mut s = posed();
    let id = s.project.characters[0].id;
    let first = s.project.characters[0].keys[0].pose.clone();
    let last = s.project.characters[0].keys[2].pose.clone();
    apply(
        &mut s,
        vec![Command::TrainMotion {
            character: id,
            name: "Leap".into(),
            smoothing: 0.0001,
        }],
    )
    .unwrap();
    for frame in [0., 7., 15., 22., 30.] {
        let pose = &evaluate(&s.project, frame).characters[0].pose;
        assert!((pose.joints[5].distance(pose.joints[6]) - 40.).abs() < 0.001);
    }
    assert_eq!(
        serde_json::to_value(&evaluate(&s.project, 0.).characters[0].pose).unwrap(),
        serde_json::to_value(first).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&evaluate(&s.project, 30.).characters[0].pose).unwrap(),
        serde_json::to_value(last).unwrap()
    );
}
#[test]
fn editing_a_source_invalidates_learning_and_undo_restores_it() {
    let mut s = posed();
    let id = s.project.characters[0].id;
    apply(
        &mut s,
        vec![Command::TrainMotion {
            character: id,
            name: "Leap".into(),
            smoothing: 0.0001,
        }],
    )
    .unwrap();
    let pose = evaluate(&s.project, 7.).characters[0].pose.clone();
    apply(
        &mut s,
        vec![Command::SetPose {
            character: id,
            frame: 7,
            pose,
            easing: Easing::Linear,
        }],
    )
    .unwrap();
    assert!(s.project.learned_motions.is_empty());
    s.undo(s.project.id, s.project.revision).unwrap();
    assert_eq!(s.project.learned_motions.len(), 1);
}
#[test]
fn baking_pose_motion_preserves_sampled_animation() {
    let mut s = posed();
    let id = s.project.characters[0].id;
    apply(
        &mut s,
        vec![Command::TrainMotion {
            character: id,
            name: "Leap".into(),
            smoothing: 0.0001,
        }],
    )
    .unwrap();
    let before: (Vec<_>, _) = (
        (0..=30)
            .map(|f| serde_json::to_value(evaluate(&s.project, f as f64)).unwrap())
            .collect(),
        s.project.learned_motions[0].id,
    );
    apply(&mut s, vec![Command::BakeMotion { motion: before.1 }]).unwrap();
    assert!(s.project.learned_motions.is_empty());
    assert_eq!(s.project.characters[0].keys.len(), 31);
    for (frame, value) in before.0.into_iter().enumerate() {
        assert_eq!(
            value,
            serde_json::to_value(evaluate(&s.project, frame as f64)).unwrap()
        );
    }
}
#[test]
fn stroke_learning_suppresses_repeated_redrawing_and_can_bake() {
    let mut s = posed();
    let sources = strokes(&mut s);
    let original = s.project.drawings[0].points.clone();
    apply(
        &mut s,
        vec![Command::TrainDrawing {
            sources,
            name: "Outline motion".into(),
            smoothing: 0.0001,
        }],
    )
    .unwrap();
    let m = &s.project.drawing_motions[0];
    assert_eq!(sample_drawing(&s.project, m, 0.), original);
    assert!(learned_points(m, 7.)[0].y < 195.);
    let before = sample_drawing(&s.project, m, 7.);
    let motion = m.id;
    apply(&mut s, vec![Command::BakeDrawing { motion }]).unwrap();
    assert!(s.project.drawing_motions.is_empty());
    assert_eq!(
        s.project
            .drawings
            .iter()
            .find(|d| d.start == 7)
            .unwrap()
            .points,
        before
    );
    assert_eq!(s.project.drawings.last().unwrap().end, 119);
}
#[test]
fn editing_a_learned_stroke_invalidates_its_model() {
    let mut s = posed();
    let sources = strokes(&mut s);
    apply(
        &mut s,
        vec![Command::TrainDrawing {
            sources,
            name: "Motion".into(),
            smoothing: 0.0001,
        }],
    )
    .unwrap();
    let mut d = s.project.drawings[0].clone();
    d.points[0].x += 4.;
    apply(&mut s, vec![Command::ReplaceDrawing { drawing: d }]).unwrap();
    assert!(s.project.drawing_motions.is_empty());
}
#[test]
fn incompatible_strokes_fail_without_partial_timing_changes() {
    let mut s = posed();
    let sources = strokes(&mut s);
    s.project.drawings[1].color = [255, 0, 0, 255];
    let before = serde_json::to_value(&s.project).unwrap();
    assert!(
        apply(
            &mut s,
            vec![Command::TrainDrawing {
                sources,
                name: "Motion".into(),
                smoothing: 0.0001
            }]
        )
        .is_err()
    );
    assert_eq!(before, serde_json::to_value(&s.project).unwrap());
}
#[test]
fn duplicate_timestamps_and_zero_length_strokes_are_rejected() {
    let mut s = posed();
    let sources = strokes(&mut s);
    s.project.drawings[1].start = 0;
    assert!(train_drawing(&s.project, &sources, 0.001, "Bad".into()).is_err());
    assert!(resample_stroke(&[Point::default(), Point::default()], 64).is_err());
}
#[test]
fn retiming_refits_models_to_the_new_authored_frames() {
    let mut s = posed();
    let id = s.project.characters[0].id;
    apply(
        &mut s,
        vec![Command::TrainMotion {
            character: id,
            name: "Motion".into(),
            smoothing: 0.0001,
        }],
    )
    .unwrap();
    apply(
        &mut s,
        vec![Command::Retime {
            start: 0,
            end: 30,
            new_end: 60,
        }],
    )
    .unwrap();
    assert_eq!(s.project.learned_motions[0].source_frames, vec![0, 30, 60]);
    assert_eq!(s.project.learned_motions[0].end, 60);
    s.project.validate().unwrap();
}
#[test]
fn corrupted_model_and_old_schema_are_rejected() {
    let mut s = posed();
    let id = s.project.characters[0].id;
    apply(
        &mut s,
        vec![Command::TrainMotion {
            character: id,
            name: "Motion".into(),
            smoothing: 0.0001,
        }],
    )
    .unwrap();
    s.project.learned_motions[0].model.weights[0][0] = f64::NAN;
    assert!(s.project.validate().is_err());
    let mut p = motion_lab_demo().unwrap();
    p.schema = 1;
    assert!(p.validate().is_err());
}
#[test]
fn motion_graphics_interpolate_rotation_scale_and_alpha() {
    let p = motion_lab_demo().unwrap();
    let g = p
        .graphics
        .iter()
        .find(|g| g.kind == GraphicKind::Star)
        .unwrap();
    let key = g.at(74.5);
    assert!((key.rotation - std::f32::consts::FRAC_PI_2).abs() < 0.001);
    assert!((key.scale - 0.975).abs() < 0.001);
    assert!((key.opacity - 0.85).abs() < 0.001);
}
#[test]
fn version_one_projects_load_with_empty_new_collections() {
    let mut value = serde_json::to_value(fight_demo().unwrap()).unwrap();
    value["schema"] = 1.into();
    for key in [
        "graphics",
        "dialogues",
        "learned_motions",
        "drawing_motions",
    ] {
        value.as_object_mut().unwrap().remove(key);
    }
    let p: Project = serde_json::from_value(value).unwrap();
    p.validate().unwrap();
    assert!(p.graphics.is_empty());
}
#[test]
fn dialogue_character_dependencies_and_removal_are_consistent() {
    let mut s = Session::new(motion_lab_demo().unwrap()).unwrap();
    let id = s.project.characters[0].id;
    assert!(s.project.dialogues.iter().any(|d| d.character == Some(id)));
    apply(&mut s, vec![Command::Remove { id }]).unwrap();
    assert!(s.project.dialogues.iter().all(|d| d.character != Some(id)));
    assert!(s.project.learned_motions.is_empty());
    s.project.validate().unwrap();
}
