use astral_core::*;
#[test]
fn pressure_tapers_across_a_long_segment_instead_of_using_average_width() {
    let mut p = Project::new("Pressure", 64, 32, 24, 1);
    p.schema = 9;
    let layer = DrawingLayer::new("Ink");
    let id = layer.id;
    p.layers.push(layer);
    p.cels.push(DrawingCel {
        id: Id::new_v4(),
        layer: id,
        frame: 0,
        strokes: vec![InkStroke {
            bitmap: None,
            skin: None,
            pressures: vec![0.2, 1.],
            style: BrushStyle::Solid,
            filled: false,
            curved: false,
            points: vec![Point::new(10., 10.), Point::new(30., 10.)],
            color: [0, 0, 0, 255],
            width: 10.,
        }],
    });
    p.validate().unwrap();
    let list = astral_render::display_list(&p, 0.);
    let contour = list
        .primitives
        .iter()
        .find_map(|p| {
            if let astral_render::Primitive::Path { contours, .. } = p {
                Some(&contours[0])
            } else {
                None
            }
        })
        .unwrap();
    assert!((contour[0].y - 11.).abs() < 0.001);
    assert!((contour[1].y - 15.).abs() < 0.001);
}
#[test]
fn layer_order_opacity_and_visibility_match_rendered_cels() {
    let mut p = Project::new("Layers", 50, 50, 24, 24);
    p.background = [255, 255, 255, 255];
    let mut s = Session::new(p).unwrap();
    let red = DrawingLayer::new("Red");
    let rid = red.id;
    let blue = DrawingLayer::new("Blue");
    let bid = blue.id;
    let ink = |color| {
        vec![InkStroke {
            bitmap: None,
            skin: None,
            pressures: vec![],
            style: BrushStyle::Solid,
            filled: false,
            curved: false,
            points: vec![Point::new(5., 25.), Point::new(45., 25.)],
            color,
            width: 10.,
        }]
    };
    let apply = |s: &mut Session, commands| {
        s.apply(Batch {
            project: s.project.id,
            expected_revision: s.project.revision,
            commands,
        })
        .unwrap();
    };
    apply(
        &mut s,
        vec![
            Command::AddLayer { layer: red },
            Command::AddLayer { layer: blue },
            Command::SetCel {
                layer: rid,
                frame: 0,
                strokes: ink([255, 0, 0, 255]),
            },
            Command::SetCel {
                layer: bid,
                frame: 0,
                strokes: ink([0, 0, 255, 255]),
            },
        ],
    );
    let pixel = |p: &Project| {
        let bytes = astral_render::rgba(p, 0.).unwrap();
        let at = (25 * 50 + 25) * 4;
        bytes[at..at + 4].to_vec()
    };
    assert_eq!(pixel(&s.project), [0, 0, 255, 255]);
    apply(
        &mut s,
        vec![Command::MoveLayer {
            layer: rid,
            index: 1,
        }],
    );
    assert_eq!(pixel(&s.project), [255, 0, 0, 255]);
    apply(
        &mut s,
        vec![
            Command::MoveLayer {
                layer: bid,
                index: 1,
            },
            Command::SetLayer {
                layer: bid,
                name: None,
                visible: None,
                locked: None,
                opacity: Some(0.5),
            },
        ],
    );
    let blended = pixel(&s.project);
    assert!((126..=129).contains(&blended[0]) && (126..=129).contains(&blended[2]));
    apply(
        &mut s,
        vec![Command::SetLayer {
            layer: bid,
            name: None,
            visible: Some(false),
            locked: None,
            opacity: None,
        }],
    );
    assert_eq!(pixel(&s.project), [255, 0, 0, 255]);
}
