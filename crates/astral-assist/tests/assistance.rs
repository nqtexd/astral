use astral_assist::{drawing::*, network, recognition};
use astral_core::*;
fn image(background: Color) -> Vec<u8> {
    let mut p = Project::new("Reference", 256, 320, 24, 24);
    p.background = background;
    p.characters.push(stick_character(
        "Figure".into(),
        Point::new(128., 210.),
        [20, 20, 20, 255],
    ));
    astral_render::png(&p, 0., 256, 320).unwrap()
}
#[test]
fn trained_model_improves_held_out_missing_joints_and_pins_observations() {
    let model = network::PoseNetwork::bundled().unwrap();
    let metrics = network::evaluate_model(&model, 256, 0xabcdef123).unwrap();
    assert!(
        metrics["missing_joint_mse"].as_f64().unwrap()
            < metrics["rest_pose_baseline_mse"].as_f64().unwrap() * 0.5
    );
    let c = stick_character("Actor".into(), Point::new(120., 210.), [0, 0, 0, 255]);
    let mut pose = c.keys[0].pose.clone();
    pose.joints[4] = Point::new(-70., -50.);
    let known = vec![
        "hips".into(),
        "chest".into(),
        "head".into(),
        "left_hand".into(),
    ];
    let completed = network::complete_pose(&model, &c, &pose, &known).unwrap();
    for name in known {
        let j = c.joints.iter().position(|j| j.name == name).unwrap();
        assert_eq!(completed.joints[j], pose.joints[j]);
    }
    assert!(completed.joints.iter().all(|p| p.finite()));
    assert_eq!(completed.root, pose.root);
}
#[test]
fn model_validation_and_training_cancellation_are_bounded() {
    let mut model = network::PoseNetwork::bundled().unwrap();
    model.w1[0] = f32::NAN;
    assert!(model.validate().is_err());
    assert!(network::train_with_progress(512, 5, 123, |_| anyhow::bail!("cancelled")).is_err());
    assert!(network::train(1, 1, 123).is_err());
}
#[test]
fn recognizes_clear_and_transparent_stick_figures_as_editable_rigs() {
    for background in [[255, 255, 255, 255], [0, 0, 0, 0]] {
        let detected = recognition::recognize(&image(background)).unwrap();
        assert_eq!(detected.landmarks.len(), 11);
        assert!(detected.score > 0.7, "{detected:?}");
        assert!(
            detected.landmarks[0]
                .position
                .distance(Point::new(128., 210.))
                < 20.,
            "{detected:?}"
        );
        assert!(detected.landmarks[8].position.x < detected.landmarks[10].position.x);
        let rig =
            recognition::rig(&detected, "Mapped".into(), Point::new(300., 300.), 240., 0).unwrap();
        assert_eq!(rig.joints[4].name, "left_hand");
        let mut p = Project::new("Imported", 640, 480, 24, 24);
        p.characters.push(rig);
        p.validate().unwrap();
        assert!(astral_render::png(&p, 0., 640, 480).unwrap().len() > 1000);
    }
}
#[test]
fn rejects_empty_dense_and_invalid_images() {
    assert!(recognition::recognize(b"not an image").is_err());
    let mut blank = Project::new("Blank", 128, 128, 24, 24);
    blank.background = [255, 255, 255, 255];
    assert!(recognition::recognize(&astral_render::png(&blank, 0., 128, 128).unwrap()).is_err());
    let mut image = image::RgbaImage::from_pixel(128, 128, image::Rgba([255, 255, 255, 255]));
    for y in 8..120 {
        for x in 8..120 {
            image.put_pixel(x, y, image::Rgba([10, 10, 10, 255]));
        }
    }
    let mut bytes = std::io::Cursor::new(vec![]);
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    assert!(recognition::recognize(bytes.get_ref()).is_err());
}
#[test]
fn geometric_suggestions_preserve_the_drawn_prefix() {
    let line = InkStroke {
        bitmap: None,
        skin: None,
        pressures: vec![],
        style: BrushStyle::Solid,
        filled: false,
        curved: false,
        points: vec![Point::new(0., 0.), Point::new(80., 0.)],
        color: [0, 0, 0, 255],
        width: 3.,
    };
    let suggestion = suggest_stroke(&line, 0.5).unwrap();
    assert_eq!(&suggestion.stroke.points[..2], &line.points);
    assert!(suggestion.stroke.points.last().unwrap().x > 80.);
    let circle = InkStroke {
        bitmap: None,
        skin: None,
        pressures: vec![],
        style: BrushStyle::Solid,
        filled: false,
        curved: false,
        points: (0..24)
            .map(|i| {
                let a = i as f32 / 23. * std::f32::consts::FRAC_PI_2;
                Point::new(100. + a.cos() * 50., 100. + a.sin() * 50.)
            })
            .collect(),
        ..line
    };
    let suggestion = suggest_stroke(&circle, 0.5).unwrap();
    assert_eq!(&suggestion.stroke.points[..24], &circle.points);
    assert_eq!(suggestion.stroke.points.last(), circle.points.first());
    assert!(suggestion.ghost.points.len() > 20);
}
#[test]
fn learns_cel_inbetweens_and_keeps_authored_frames_and_partial_ink() {
    let mut p = Project::new("Examples", 640, 360, 24, 24);
    let layer = DrawingLayer::new("Ink");
    let id = layer.id;
    p.layers.push(layer);
    p.schema = 3;
    let stroke = |y| InkStroke {
        bitmap: None,
        skin: None,
        pressures: vec![],
        style: BrushStyle::Solid,
        filled: false,
        curved: false,
        points: vec![
            Point::new(10., y),
            Point::new(40., y + 5.),
            Point::new(80., y),
        ],
        color: [0, 0, 0, 255],
        width: 3.,
    };
    for (frame, y) in [(0, 100.), (10, 40.), (20, 100.)] {
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: id,
            frame,
            strokes: vec![stroke(y), stroke(y + 40.)],
        });
    }
    let model = CelLearner::train(&p, id, &[0, 10, 20], 0.0001).unwrap();
    let known = stroke(123.);
    p.cels.push(DrawingCel {
        id: Id::new_v4(),
        layer: id,
        frame: 5,
        strokes: vec![known.clone()],
    });
    let completed = model.complete(&p, id, 5).unwrap();
    assert_eq!(completed[0], known);
    assert_eq!(completed.len(), 2);
    let commands = model.generate(&p, id, 0, 20, false).unwrap();
    assert_eq!(commands.len(), 17);
    let before = serde_json::to_value(&p.cels).unwrap();
    let mut s = Session::new(p).unwrap();
    s.apply(Batch {
        project: s.project.id,
        expected_revision: 0,
        commands,
    })
    .unwrap();
    assert_eq!(s.project.cels.len(), 21);
    s.undo(s.project.id, s.project.revision).unwrap();
    assert_eq!(serde_json::to_value(&s.project.cels).unwrap(), before);
}
