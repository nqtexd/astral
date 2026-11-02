use astral_core::*;
use astral_render::*;
#[test]
fn gradient_mask_is_deterministic_and_keeps_straight_alpha() {
    let mut p = Project::new("Mask", 32, 16, 24, 24);
    p.schema = 10;
    p.background = [0, 0, 0, 0];
    let red = Id::new_v4();
    let mask = Id::new_v4();
    let output = Id::new_v4();
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Masked".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output,
        nodes: vec![
            CompositeNode {
                id: red,
                name: "Red".into(),
                op: CompositeOp::Solid {
                    color: [255, 0, 0, 255],
                },
            },
            CompositeNode {
                id: mask,
                name: "Mask".into(),
                op: CompositeOp::Gradient {
                    start: Point::new(0., 0.),
                    end: Point::new(1., 0.),
                    from: [0, 0, 0, 0],
                    to: [255, 255, 255, 255],
                    radial: false,
                },
            },
            CompositeNode {
                id: output,
                name: "Masked".into(),
                op: CompositeOp::Mask {
                    source: red,
                    mask,
                    invert: false,
                    luminance: false,
                },
            },
        ],
    }];
    p.validate().unwrap();
    let a = display_list(&p, 0.);
    let b = display_list(&p, 1.);
    assert_eq!(a, b, "Unchanged graph uses the identical evaluated texture");
    let rgba = rgba(&p, 0.).unwrap();
    let left = &rgba[(8 * 32 + 1) * 4..][..4];
    let right = &rgba[(8 * 32 + 30) * 4..][..4];
    assert_eq!(left[0], 255);
    assert_eq!(right[0], 255);
    assert!(left[3] < 25 && right[3] > 230);
}
#[test]
fn procedural_brush_grain_is_reproducible_and_survives_frame_isolation() {
    let mut p = Project::new("Ink", 64, 64, 24, 24);
    let layer = DrawingLayer::new("Ink");
    let id = layer.id;
    p.layers.push(layer);
    p.schema = 10;
    let stroke = InkStroke {
        brush: Some(StampBrush {
            spacing: 0.1,
            scatter: 0.2,
            grain: 0.8,
            ..Default::default()
        }),
        points: vec![Point::new(10., 20.), Point::new(55., 45.)],
        width: 12.,
        color: [40, 80, 120, 255],
        bitmap: None,
        skin: None,
        pressures: vec![0.2, 1.],
        style: BrushStyle::Solid,
        filled: false,
        curved: false,
    };
    let mut s = Session::new(p).unwrap();
    s.apply(Batch {
        project: s.project.id,
        expected_revision: 0,
        commands: vec![Command::SetFrameDrawing {
            layer: id,
            frame: 0,
            strokes: vec![stroke],
        }],
    })
    .unwrap();
    let a = png(&s.project, 0., 64, 64).unwrap();
    assert_eq!(a, png(&s.project, 0., 64, 64).unwrap());
    assert_ne!(a, png(&s.project, 1., 64, 64).unwrap());
}
