use astral_core::*;
use astral_render::*;

#[test]
fn rich_dialogue_styles_render_and_require_schema_four() {
    let mut p = motion_lab_demo().unwrap();
    let plain = png(&p, 75., 640, 360).unwrap();
    for d in &mut p.dialogues {
        d.formatting = TextFormatting {
            font: TextFont::BoldItalic,
            align: TextAlign::Center,
            line_height: 1.8,
            padding: 24.,
        };
    }
    assert!(p.validate().is_err());
    p.schema = 4;
    p.validate().unwrap();
    assert_ne!(plain, png(&p, 75., 640, 360).unwrap());
    let decoded: Project = serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
    assert_eq!(
        png(&p, 75., 640, 360).unwrap(),
        png(&decoded, 75., 640, 360).unwrap()
    );
    p.dialogues[0].formatting.padding = 10000.;
    assert!(p.validate().is_err());
}

#[test]
fn vector_text_adds_ink_and_matches_gpu_fill_geometry() {
    let p = motion_lab_demo().unwrap();
    let mut no_text = p.clone();
    no_text.dialogues.clear();
    for g in &mut no_text.graphics {
        g.text.clear();
    }
    let with = png(&p, 75., 960, 540).unwrap();
    assert_ne!(with, png(&no_text, 75., 960, 540).unwrap());
    for primitive in display_list(&p, 75.).primitives {
        if let Primitive::Path { contours, .. } = primitive {
            let (vertices, indices) = tessellate(&contours).unwrap();
            assert!(vertices.iter().all(|p| p.finite()));
            assert!(indices.iter().all(|i| (*i as usize) < vertices.len()));
        }
    }
}
#[test]
fn typewriter_reveals_more_glyphs_without_changing_box_geometry() {
    let mut p = motion_lab_demo().unwrap();
    p.graphics.clear();
    p.characters.clear();
    p.actions.clear();
    p.learned_motions.clear();
    p.dialogues.retain(|d| d.style == DialogueStyle::Caption);
    p.dialogues[0].start = 0;
    p.dialogues[0].end = 120;
    p.dialogues[0].typewriter = 10.;
    let a = display_list(&p, 0.);
    let b = display_list(&p, 60.);
    let points = |l: &DisplayList| {
        l.primitives
            .iter()
            .filter_map(|p| {
                if let Primitive::Path { contours, .. } = p {
                    Some(contours.iter().map(Vec::len).sum::<usize>())
                } else {
                    None
                }
            })
            .sum::<usize>()
    };
    assert!(points(&b) > points(&a));
    assert_ne!(
        png(&p, 0., 960, 540).unwrap(),
        png(&p, 60., 960, 540).unwrap()
    );
}
#[test]
fn learned_drawings_bake_to_identical_export_frames() {
    let p = motion_lab_demo().unwrap();
    let id = p.drawing_motions[0].id;
    let frames = [0., 22., 45., 66., 90., 120.];
    let images = frames
        .iter()
        .map(|f| png(&p, *f, 640, 360).unwrap())
        .collect::<Vec<_>>();
    let mut s = Session::new(p).unwrap();
    s.apply(Batch {
        project: s.project.id,
        expected_revision: s.project.revision,
        commands: vec![Command::BakeDrawing { motion: id }],
    })
    .unwrap();
    for (i, f) in frames.into_iter().enumerate() {
        assert_eq!(images[i], png(&s.project, f, 640, 360).unwrap());
    }
}
#[test]
fn ring_has_a_hole_and_draw_on_finishes_even_on_short_clips() {
    let mut p = Project::new("Ring", 200, 200, 30, 3);
    p.background = [0, 0, 0, 255];
    p.graphics.push(Graphic {
        contours: vec![],
        id: Id::new_v4(),
        name: "Ring".into(),
        kind: GraphicKind::Ring,
        start: 0,
        end: 2,
        size: Point::new(120., 120.),
        color: [255, 255, 255, 255],
        layer: GraphicLayer::Foreground,
        text: String::new(),
        keys: vec![GraphicKey {
            frame: 0,
            position: Point::new(100., 100.),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
        draw_on: false,
    });
    let pix = rasterize(&display_list(&p, 1.), 200, 200).unwrap();
    assert_eq!(pix.pixel(100, 100).unwrap().red(), 0);
    assert!(pix.pixel(155, 100).unwrap().red() > 200);
    p.graphics[0].draw_on = true;
    let list = display_list(&p, 2.);
    for primitive in list.primitives {
        if let Primitive::Polyline { points, .. } = primitive {
            assert_eq!(points.first(), points.last());
        }
    }
}

#[test]
fn editable_dark_border_and_no_border_render_without_purple_and_roundtrip() {
    let demo = motion_lab_demo().unwrap();
    let mut p = Project::new("Dialogue styles", 960, 540, 24, 96);
    p.schema = 4;
    p.dialogues = vec![demo.dialogues[0].clone()];
    let d = &mut p.dialogues[0];
    d.start = 0;
    d.end = p.duration - 1;
    d.character = None;
    d.position = Point::new(40., 40.);
    d.size = Point::new(300., 160.);
    d.text = "Editable".into();
    d.speaker = "Speaker".into();
    d.typewriter = 0.;
    d.appearance = DialogueAppearance::dark();
    d.color = [24, 27, 33, 255];
    d.text_color = [240, 240, 245, 255];
    let image = png(&p, 0., 640, 360).unwrap();
    let decoded: Project = serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
    assert_eq!(image, png(&decoded, 0., 640, 360).unwrap());
    let list = display_list(&p, 0.);
    assert!(!list.primitives.iter().any(|p| matches!(
        p,
        Primitive::Path {
            color: [167, 139, 250, 255] | [111, 74, 179, 255],
            ..
        }
    )));
    p.dialogues[0].appearance.border_width = 0.;
    assert_ne!(image, png(&p, 0., 640, 360).unwrap());
}
#[test]
fn full_canvas_fast_path_matches_unframed_rasterization() {
    let p = motion_lab_demo().unwrap();
    let list = display_list(&p, 75.);
    assert_eq!(
        rasterize(&list, 960, 540).unwrap().data(),
        rasterize_framed(&list, 960, 540, false).unwrap().data()
    );
}
