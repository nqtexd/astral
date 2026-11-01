use astral_core::*;
fn fixture() -> Project {
    let mut p = Project::new("Feedback", 960, 540, 24, 72);
    p.schema = 6;
    p.characters.push(stick_character(
        "Actor".into(),
        Point::new(480., 360.),
        [20, 30, 40, 255],
    ));
    p.dialogues.push(Dialogue {
        id: Id::new_v4(),
        name: "Caption".into(),
        text: "Editable words".into(),
        speaker: String::new(),
        start: 0,
        end: 71,
        position: Point::new(430., 170.),
        size: Point::new(240., 100.),
        font_size: 24.,
        color: [24, 27, 33, 255],
        text_color: [240, 240, 245, 255],
        appearance: DialogueAppearance::dark(),
        style: DialogueStyle::Caption,
        character: None,
        typewriter: 0.,
        formatting: Default::default(),
    });
    p
}
#[test]
fn structural_validation_is_not_a_composition_pass_and_proposals_are_undoable() {
    let p = fixture();
    p.validate().unwrap();
    let before = serde_json::to_vec(&p).unwrap();
    let issues = staging::review(&p, 0, 71, 1).unwrap().layout;
    assert!(issues.iter().any(|i| i.kind == "dialogue_head_collision"));
    let plan = studio::plan(
        &p,
        studio::StudioOperation::LayoutDialogue {
            dialogue: p.dialogues[0].id,
        },
    )
    .unwrap();
    assert_eq!(before, serde_json::to_vec(&p).unwrap());
    assert!(plan.warnings.is_empty());
    let mut session = Session::new(p).unwrap();
    let id = session.project.id;
    session
        .apply(Batch {
            project: id,
            expected_revision: 0,
            commands: plan.commands,
        })
        .unwrap();
    assert!(
        composition::review(&session.project, 0, 71, 1)
            .unwrap()
            .is_empty()
    );
    assert_eq!(session.project.dialogues[0].text, "Editable words");
    session.undo(id, session.project.revision).unwrap();
    assert_eq!(
        session.project.dialogues[0].position,
        Point::new(430., 170.)
    );
}
#[test]
fn layout_review_detects_moving_actors_feet_props_boxes_and_camera_crop() {
    let mut p = fixture();
    p.dialogues[0].position = Point::new(440., 350.);
    let mut other = p.dialogues[0].clone();
    other.id = Id::new_v4();
    p.dialogues.push(other);
    let mut last = p.characters[0].keys[0].clone();
    last.frame = 71;
    last.pose.root.x = 850.;
    p.characters[0].keys.push(last);
    p.graphics.push(Graphic {
        id: Id::new_v4(),
        name: "Prop".into(),
        kind: GraphicKind::Rectangle,
        start: 0,
        end: 71,
        size: Point::new(120., 80.),
        color: [50, 50, 50, 255],
        layer: GraphicLayer::Foreground,
        text: String::new(),
        draw_on: false,
        contours: vec![],
        keys: vec![GraphicKey {
            frame: 0,
            position: Point::new(490., 390.),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
    });
    let issues = composition::review(&p, 0, 71, 1).unwrap();
    for kind in [
        "dialogue_foot_collision",
        "dialogue_graphic_collision",
        "dialogue_dialogue_collision",
    ] {
        assert!(issues.iter().any(|i| i.kind == kind), "{kind}: {issues:?}");
    }
    p.dialogues[0].position = Point::new(940., 10.);
    assert!(
        composition::review(&p, 0, 0, 1)
            .unwrap()
            .iter()
            .any(|i| i.kind == "off_canvas")
    );
}
#[test]
fn flow_uses_joint_arcs_and_passes_keys_without_pausing_or_shortening_bones() {
    let mut p = fixture();
    p.dialogues.clear();
    let c = &mut p.characters[0];
    let base = c.keys[0].pose.clone();
    let elbow = c
        .joints
        .iter()
        .position(|j| j.name == "right_elbow")
        .unwrap();
    let hand = c
        .joints
        .iter()
        .position(|j| j.name == "right_hand")
        .unwrap();
    let length = base.joints[elbow].distance(base.joints[hand]);
    c.keys = (0..4)
        .map(|i| {
            let mut pose = base.clone();
            pose.root.x += i as f32 * 60.;
            let a = -0.8 + i as f32 * 0.5;
            pose.joints[hand] = pose.joints[elbow] + Point::new(a.cos() * length, a.sin() * length);
            PoseKey {
                frame: i * 20,
                pose,
                easing: Easing::Flow,
            }
        })
        .collect();
    let expected = c.keys[1].pose.clone();
    let id = c.id;
    let at = |frame: f64| {
        evaluate(&p, frame)
            .characters
            .into_iter()
            .find(|c| c.id == id)
            .unwrap()
            .pose
    };
    assert_eq!(at(20.).root, expected.root);
    assert!(at(20.).joints[hand].distance(expected.joints[hand]) < 0.001);
    let before = at(19.99);
    let key = at(20.);
    let after = at(20.01);
    let v0 = (key.root.x - before.root.x) / 0.01;
    let v1 = (after.root.x - key.root.x) / 0.01;
    assert!((v0 - v1).abs() < 0.02 && v0 > 2.9, "{v0} {v1}");
    for f in 0..600 {
        let pose = at(f as f64 / 10.);
        assert!((pose.joints[elbow].distance(pose.joints[hand]) - length).abs() < 0.001);
    }
}
#[test]
fn dialogue_appearance_is_backward_compatible_and_rejects_invalid_geometry() {
    let mut p = fixture();
    let mut json = serde_json::to_value(&p).unwrap();
    json["dialogues"][0]
        .as_object_mut()
        .unwrap()
        .remove("appearance");
    let old: Project = serde_json::from_value(json).unwrap();
    assert_eq!(
        old.dialogues[0].appearance.border_color,
        [167, 139, 250, 255]
    );
    p.dialogues[0].appearance.border_width = -1.;
    assert!(p.validate().is_err());
}

#[test]
fn text_graphics_are_reviewed_and_oversized_dialogue_is_not_claimed_clear() {
    let mut p = fixture();
    let source = p.dialogues[0].clone();
    p.graphics.push(Graphic {
        id: Id::new_v4(),
        name: "Title".into(),
        kind: GraphicKind::LowerThird,
        start: 0,
        end: 71,
        size: Point::new(300., 100.),
        color: [20, 20, 20, 255],
        layer: GraphicLayer::Foreground,
        text: "Editable title".into(),
        draw_on: false,
        contours: vec![],
        keys: vec![GraphicKey {
            frame: 0,
            position: Point::new(480., 250.),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
    });
    let issues = composition::review(&p, 0, 71, 1).unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i.kind == "graphic_text_head_collision" && i.subject_kind == "graphic_text")
    );
    p.dialogues[0].size = Point::new(1200., 800.);
    let (_, warnings) = composition::place_dialogue(&p, source.id).unwrap();
    assert!(!warnings.is_empty());
    p.dialogues[0].size = Point::new(240., 100.);
    p.dialogues[0].position = Point::new(40., 40.);
    p.camera = vec![CameraKey {
        frame: 0,
        center: Point::new(480., 270.),
        zoom: 3.,
    }];
    assert!(
        composition::review(&p, 0, 0, 1)
            .unwrap()
            .iter()
            .any(|i| i.kind == "off_canvas")
    );
}
