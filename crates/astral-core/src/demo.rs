use crate::*;

/// Editable examples, including the actual authored samples used by the learner.
pub fn motion_lab_demo() -> anyhow::Result<Project> {
    let mut session = Session::new(Project::new("Motion lab", 1920, 1080, 30, 300))?;
    let mut character =
        stick_character("Luma".into(), Point::new(800., 820.), [188, 166, 255, 255]);
    character.thickness = 12.;
    for j in &mut character.joints {
        j.position.x *= 1.5;
        j.position.y *= 1.5;
    }
    let mut keys = vec![];
    for (frame, root, angle) in [
        (0, Point::new(800., 820.), -0.9f32),
        (45, Point::new(960., 670.), 0.4),
        (90, Point::new(1120., 820.), -0.9),
    ] {
        let mut pose = Pose {
            root,
            joints: character.joints.iter().map(|j| j.position).collect(),
        };
        let elbow = pose.joints[5];
        let length = pose.joints[5].distance(pose.joints[6]);
        pose.joints[6] = elbow + Point::new(angle.cos() * length, angle.sin() * length);
        keys.push(PoseKey {
            frame,
            pose,
            easing: Easing::Smooth,
        });
    }
    character.keys = keys;
    let id = character.id;
    let mut commands = vec![
        Command::AddRig { character },
        Command::TrainMotion {
            character: id,
            name: "Luma's learned leap".into(),
            smoothing: 0.0001,
        },
    ];
    for (i, position) in [
        Point::new(380., 170.),
        Point::new(1350., 220.),
        Point::new(1740., 110.),
    ]
    .into_iter()
    .enumerate()
    {
        let a = GraphicKey {
            frame: 0,
            position,
            scale: 1.,
            rotation: 0.,
            opacity: 0.8,
            easing: Easing::Linear,
        };
        let mut b = a.clone();
        b.frame = 299;
        b.position.x += if i == 1 { -130. } else { 140. };
        commands.push(Command::AddGraphic {
            graphic: Graphic {
                contours: vec![],
                id: Id::new_v4(),
                name: format!("Cloud {}", i + 1),
                kind: GraphicKind::Cloud,
                start: 0,
                end: 299,
                size: Point::new(330., 155.),
                color: [222, 232, 255, 175],
                layer: GraphicLayer::Background,
                text: String::new(),
                keys: vec![a, b],
                draw_on: false,
            },
        });
    }
    for (kind, size, color) in [
        (
            GraphicKind::Star,
            Point::new(150., 150.),
            [209, 172, 255, 255],
        ),
        (
            GraphicKind::Ring,
            Point::new(250., 250.),
            [112, 164, 235, 180],
        ),
    ] {
        let a = GraphicKey {
            frame: 0,
            position: Point::new(1570., 640.),
            scale: 0.8,
            rotation: 0.,
            opacity: 0.7,
            easing: Easing::Smooth,
        };
        let mut b = a.clone();
        b.frame = 149;
        b.scale = 1.15;
        b.rotation = std::f32::consts::PI;
        b.opacity = 1.;
        let mut c = a.clone();
        c.frame = 299;
        c.rotation = std::f32::consts::TAU;
        commands.push(Command::AddGraphic {
            graphic: Graphic {
                contours: vec![],
                id: Id::new_v4(),
                name: format!("{kind:?}"),
                kind,
                start: 0,
                end: 299,
                size,
                color,
                layer: GraphicLayer::Foreground,
                text: String::new(),
                keys: vec![a, b, c],
                draw_on: false,
            },
        });
    }
    commands.push(Command::AddGraphic {
        graphic: Graphic {
            contours: vec![],
            id: Id::new_v4(),
            name: "Animated lower third".into(),
            kind: GraphicKind::LowerThird,
            start: 0,
            end: 299,
            size: Point::new(710., 105.),
            color: [94, 66, 151, 255],
            layer: GraphicLayer::Foreground,
            text: "ASTRAL  /  MOTION LAB".into(),
            keys: vec![
                GraphicKey {
                    frame: 0,
                    position: Point::new(-360., 1000.),
                    scale: 1.,
                    rotation: 0.,
                    opacity: 0.,
                    easing: Easing::Smooth,
                },
                GraphicKey {
                    frame: 24,
                    position: Point::new(470., 1000.),
                    scale: 1.,
                    rotation: 0.,
                    opacity: 1.,
                    easing: Easing::Hold,
                },
            ],
            draw_on: false,
        },
    });
    commands.push(Command::AddGraphic {
        graphic: Graphic {
            contours: vec![],
            id: Id::new_v4(),
            name: "Draw-on arrow".into(),
            kind: GraphicKind::Arrow,
            start: 0,
            end: 89,
            size: Point::new(220., 85.),
            color: [113, 221, 207, 255],
            layer: GraphicLayer::Foreground,
            text: String::new(),
            keys: vec![GraphicKey {
                frame: 0,
                position: Point::new(420., 670.),
                scale: 1.,
                rotation: -0.3,
                opacity: 1.,
                easing: Easing::Smooth,
            }],
            draw_on: true,
        },
    });
    commands.push(Command::AddDialogue {
        dialogue: Dialogue {
            id: Id::new_v4(),
            name: "Luma speaks".into(),
            text: "Three poses. A whole motion.\nNow let's animate your story.".into(),
            speaker: "LUMA".into(),
            start: 15,
            end: 135,
            position: Point::new(-250., -430.),
            size: Point::new(600., 200.),
            font_size: 30.,
            color: [250, 248, 255, 255],
            text_color: [38, 29, 59, 255],
            appearance: DialogueAppearance::default(),
            style: DialogueStyle::Speech,
            character: Some(id),
            typewriter: 28.,
            formatting: Default::default(),
        },
    });
    commands.push(Command::AddDialogue {
        dialogue: Dialogue {
            id: Id::new_v4(),
            name: "Caption".into(),
            text: "Draw · pose · learn · refine".into(),
            speaker: String::new(),
            start: 150,
            end: 299,
            position: Point::new(640., 360.),
            size: Point::new(650., 110.),
            font_size: 32.,
            color: [250, 248, 255, 255],
            text_color: [38, 29, 59, 255],
            appearance: DialogueAppearance::default(),
            style: DialogueStyle::Caption,
            character: None,
            typewriter: 0.,
            formatting: Default::default(),
        },
    });
    let mut strokes = vec![];
    for (frame, phase) in [(0, 0f32), (45, 1.), (90, 2.)] {
        let drawing = Drawing {
            id: Id::new_v4(),
            name: "Ribbon sample".into(),
            start: frame,
            end: 299,
            points: (0..33)
                .map(|i| {
                    Point::new(
                        150. + i as f32 * 12.,
                        490. + (i as f32 * 0.22 + phase).sin() * 30.,
                    )
                })
                .collect(),
            color: [108, 215, 199, 255],
            width: 5.,
        };
        strokes.push(drawing.id);
        commands.push(Command::AddDrawing { drawing });
    }
    commands.push(Command::TrainDrawing {
        sources: strokes,
        name: "Learned ribbon".into(),
        smoothing: 0.0001,
    });
    commands.push(Command::AddDrawing {
        drawing: Drawing {
            id: Id::new_v4(),
            name: "Ground".into(),
            start: 0,
            end: 299,
            points: vec![Point::new(120., 960.), Point::new(1800., 960.)],
            color: [65, 83, 120, 255],
            width: 3.,
        },
    });
    for frame in [104, 134, 224] {
        commands.push(Command::AddEffect {
            effect: Effect {
                id: Id::new_v4(),
                name: "Star accent".into(),
                kind: EffectKind::Impact,
                start: frame,
                end: frame + 10,
                position: Point::new(1570., 640.),
                character: None,
                joint: None,
                color: [255, 216, 156, 255],
                intensity: 1.2,
                seed: frame as u64,
            },
        });
    }
    session.apply(Batch {
        project: session.project.id,
        expected_revision: 0,
        commands,
    })?;
    Ok(session.project)
}
