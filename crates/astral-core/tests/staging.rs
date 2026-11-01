use astral_core::{staging::*, storyboard::*, *};

#[test]
fn review_follows_a_tracking_camera_beyond_the_initial_canvas() {
    let mut p = Project::new("Tracking chase", 1280, 720, 60, 60);
    p.characters.push(stick_character(
        "A".into(),
        Point::new(2400., 520.),
        [255; 4],
    ));
    p.characters.push(stick_character(
        "B".into(),
        Point::new(2405., 520.),
        [255; 4],
    ));
    p.camera.push(CameraKey {
        frame: 0,
        center: Point::new(2400., 360.),
        zoom: 1.,
    });
    assert!(!review(&p, 0, 59, 1).unwrap().overlaps.is_empty());
}

#[test]
fn missed_contact_and_torso_overlap_are_detected_without_mutation() {
    let mut p = Project::new("Contact", 800, 600, 60, 120);
    let a = stick_character("Ren".into(), Point::new(200., 300.), [255; 4]);
    let b = stick_character("Zombie".into(), Point::new(400., 300.), [255; 4]);
    let cue = ContactCue {
        name: "Punch".into(),
        frame: 40,
        attacker: a.id,
        target: b.id,
        effector: "right_hand".into(),
        target_joint: "chest".into(),
        tolerance: 3.,
    };
    p.characters = vec![a, b];
    p.metadata.insert(
        astral_core::staging::KEY.into(),
        serde_json::to_value(vec![cue]).unwrap(),
    );
    let before = serde_json::to_value(&p).unwrap();
    let r = review(&p, 0, 119, 30).unwrap();
    assert_eq!(r.contacts.len(), 1);
    assert!(!r.contacts[0].passed);
    assert!(r.overlaps.is_empty());
    assert_eq!(before, serde_json::to_value(&p).unwrap());
    p.characters[1].keys[0].pose.root.x = 205.;
    assert!(!review(&p, 0, 119, 30).unwrap().overlaps.is_empty());
}
#[test]
fn live_binding_follows_edited_joint_and_invalid_reference_rolls_back() {
    let mut p = Project::new("Bound", 800, 600, 60, 120);
    let c = stick_character("Ren".into(), Point::new(200., 300.), [255; 4]);
    let id = c.id;
    let graphic = Graphic {
        contours: vec![],
        id: Id::new_v4(),
        name: "Sleeve".into(),
        kind: GraphicKind::Rectangle,
        start: 0,
        end: 119,
        size: Point::new(15., 40.),
        color: [255; 4],
        layer: GraphicLayer::Foreground,
        text: String::new(),
        draw_on: false,
        keys: vec![GraphicKey {
            frame: 0,
            position: Point::default(),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
    };
    let binding = GraphicBinding {
        graphic: graphic.id,
        character: id,
        joint: "chest".into(),
        to_joint: Some("right_elbow".into()),
    };
    p.characters.push(c);
    p.graphics.push(graphic.clone());
    let mut s = Session::new(p).unwrap();
    let pid = s.project.id;
    s.apply(Batch {
        project: pid,
        expected_revision: 0,
        commands: vec![Command::SetGraphicBindings {
            bindings: vec![binding.clone()],
        }],
    })
    .unwrap();
    assert_eq!(s.project.schema, 6);
    let pose = &s.project.characters[0].keys[0].pose;
    let a = bound_graphic_key(&graphic, &binding, &s.project.characters[0], pose, 0.);
    let mut moved = pose.clone();
    moved.root.x += 90.;
    let b = bound_graphic_key(&graphic, &binding, &s.project.characters[0], &moved, 0.);
    assert!((b.position.x - a.position.x - 90.).abs() < 0.001);
    let bad = GraphicBinding {
        joint: "missing".into(),
        ..binding
    };
    assert!(
        s.apply(Batch {
            project: pid,
            expected_revision: 1,
            commands: vec![Command::SetGraphicBindings {
                bindings: vec![bad]
            }]
        })
        .is_err()
    );
    assert_eq!(s.project.revision, 1);
}
#[test]
fn continuity_flags_lost_prop_and_reversal_and_accepts_an_explained_transition() {
    let id = Id::new_v4();
    let state = CharacterState {
        position: Point::new(100., 300.),
        facing: 1,
        condition: "alive".into(),
        props: vec!["rail".into()],
    };
    let shot = StoryShot {
        id: Id::new_v4(),
        title: "One".into(),
        start: 0,
        end: 59,
        location: "Hall".into(),
        action: "Run".into(),
        camera: String::new(),
        audio: String::new(),
        transition: String::new(),
        cast: vec![ShotCharacter {
            character: id,
            entry: state.clone(),
            exit: state,
        }],
    };
    let mut next = shot.clone();
    next.id = Id::new_v4();
    next.title = "Two".into();
    next.start = 60;
    next.end = 119;
    next.cast[0].entry.props.clear();
    next.cast[0].entry.facing = -1;
    let mut board = Storyboard {
        logline: String::new(),
        characters: vec![],
        shots: vec![shot, next],
    };
    let issues = analyze(&board);
    assert!(issues.iter().any(|i| i.kind == "prop_change"));
    assert!(issues.iter().any(|i| i.kind == "screen_direction"));
    board.shots[1].transition = "He drops the rail and turns toward the noise.".into();
    assert!(analyze(&board).is_empty());
}
