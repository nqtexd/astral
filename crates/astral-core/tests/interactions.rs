use astral_core::{interactions::*, *};

fn scene() -> (Project, Id, Id) {
    let mut p = Project::new("Interactions", 800, 600, 60, 120);
    let c = stick_character("Actor".into(), Point::new(200., 300.), [255; 4]);
    let id = c.id;
    let hand = c.keys[0].pose.root + c.keys[0].pose.joints[6];
    p.characters.push(c);
    let g = Graphic {
        id: Id::new_v4(),
        name: "Prop".into(),
        kind: GraphicKind::Rectangle,
        contours: vec![],
        start: 0,
        end: 119,
        size: Point::new(10., 10.),
        color: [255; 4],
        layer: GraphicLayer::Foreground,
        text: String::new(),
        draw_on: false,
        keys: vec![GraphicKey {
            frame: 0,
            position: hand,
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
    };
    let graphic = g.id;
    p.graphics.push(g);
    (p, id, graphic)
}

fn set(p: &mut Project, cues: Vec<InteractionCue>) {
    validate(p, &cues).unwrap();
    p.metadata
        .insert(KEY.into(), serde_json::to_value(cues).unwrap());
}

#[test]
fn a_touch_checks_every_frame_even_between_coarse_review_samples() {
    let (mut p, character, graphic) = scene();
    set(
        &mut p,
        vec![InteractionCue {
            name: "Hold prop".into(),
            start: 0,
            end: 60,
            kind: InteractionKind::Touch {
                character,
                joint: "right_hand".into(),
                graphic,
                offset: Point::default(),
                tolerance: 1.,
            },
        }],
    );
    assert!(staging::review(&p, 0, 119, 30).unwrap().interactions[0].passed);
    let rest = p.graphics[0].keys[0].clone();
    p.graphics[0].keys.extend([
        GraphicKey {
            frame: 10,
            ..rest.clone()
        },
        GraphicKey {
            frame: 11,
            position: Point::new(700., 100.),
            ..rest.clone()
        },
        GraphicKey { frame: 12, ..rest },
    ]);
    let r = staging::review(&p, 0, 119, 30).unwrap();
    assert!(!r.interactions[0].passed);
    assert_eq!(r.interactions[0].first_failure, Some(11));
}

#[test]
fn passing_a_closed_door_and_never_crossing_are_both_failures() {
    let (mut p, character, barrier) = scene();
    let mut key = p.characters[0].keys[0].clone();
    key.frame = 119;
    key.pose.root.x = 600.;
    p.characters[0].keys[0].easing = Easing::Linear;
    p.characters[0].keys.push(key);
    set(
        &mut p,
        vec![InteractionCue {
            name: "Exit".into(),
            start: 0,
            end: 119,
            kind: InteractionKind::Passage {
                character,
                barrier,
                crossing_x: 400.,
                half_width: 20.,
                maximum_opacity: 0.01,
            },
        }],
    );
    assert!(!review(&p, 0, 119).unwrap()[0].passed);
    p.graphics[0].keys[0].opacity = 0.;
    assert!(review(&p, 0, 119).unwrap()[0].passed);
    p.characters[0].keys[1].pose.root.x = 210.;
    assert!(!review(&p, 0, 119).unwrap()[0].passed);
}

#[test]
fn transformed_prop_offset_and_live_binding_are_used() {
    let (mut p, character, graphic) = scene();
    let hand = p.graphics[0].keys[0].position;
    p.graphics[0].keys[0].position = hand - Point::new(0., 10.);
    p.graphics[0].keys[0].rotation = std::f32::consts::FRAC_PI_2;
    p.graphics[0].keys[0].scale = 2.;
    set(
        &mut p,
        vec![InteractionCue {
            name: "Grasp".into(),
            start: 0,
            end: 119,
            kind: InteractionKind::Touch {
                character,
                joint: "right_hand".into(),
                graphic,
                offset: Point::new(5., 0.),
                tolerance: 1.,
            },
        }],
    );
    assert!(review(&p, 0, 119).unwrap()[0].passed);
    p.graphics[0].keys[0].position = Point::default();
    p.graphics[0].keys[0].scale = 1.;
    p.graphics[0].keys[0].rotation = 0.;
    p.bindings.push(GraphicBinding {
        graphic,
        character,
        joint: "right_hand".into(),
        to_joint: None,
    });
    let mut cues = read(&p).unwrap();
    if let InteractionKind::Touch { offset, .. } = &mut cues[0].kind {
        *offset = Point::default();
    }
    set(&mut p, cues);
    assert!(review(&p, 0, 119).unwrap()[0].passed);
}

#[test]
fn invalid_interactions_roll_back_and_valid_ones_undo() {
    let (p, character, graphic) = scene();
    let mut s = Session::new(p).unwrap();
    let project = s.project.id;
    let cue = InteractionCue {
        name: "Grasp".into(),
        start: 0,
        end: 119,
        kind: InteractionKind::Touch {
            character,
            joint: "missing".into(),
            graphic,
            offset: Point::default(),
            tolerance: 1.,
        },
    };
    assert!(
        s.apply(Batch {
            project,
            expected_revision: 0,
            commands: vec![Command::SetSceneInteractions { cues: vec![cue] }]
        })
        .is_err()
    );
    assert_eq!(s.project.revision, 0);
    assert!(read(&s.project).unwrap().is_empty());
    let cue = InteractionCue {
        name: "Visible".into(),
        start: 0,
        end: 119,
        kind: InteractionKind::GraphicState {
            graphic,
            min_opacity: 0.9,
            max_opacity: 1.,
        },
    };
    s.apply(Batch {
        project,
        expected_revision: 0,
        commands: vec![Command::SetSceneInteractions { cues: vec![cue] }],
    })
    .unwrap();
    assert_eq!(read(&s.project).unwrap().len(), 1);
    s.undo(project, 1).unwrap();
    assert!(read(&s.project).unwrap().is_empty());
}

#[test]
fn ripple_retiming_moves_interaction_ranges_with_the_visual_keys() {
    let (mut p, _, graphic) = scene();
    p.duration = 240;
    set(
        &mut p,
        vec![InteractionCue {
            name: "Visible".into(),
            start: 20,
            end: 40,
            kind: InteractionKind::GraphicState {
                graphic,
                min_opacity: 0.9,
                max_opacity: 1.,
            },
        }],
    );
    let mut s = Session::new(p).unwrap();
    let project = s.project.id;
    s.apply(Batch {
        project,
        expected_revision: 0,
        commands: vec![Command::Retime {
            start: 0,
            end: 119,
            new_end: 238,
        }],
    })
    .unwrap();
    let cues = read(&s.project).unwrap();
    assert_eq!((cues[0].start, cues[0].end), (40, 80));
    assert!(review(&s.project, 0, 238).unwrap()[0].passed);
    s.undo(project, 1).unwrap();
    assert_eq!(read(&s.project).unwrap()[0].end, 40);
}
