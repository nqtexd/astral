use astral_core::*;
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
