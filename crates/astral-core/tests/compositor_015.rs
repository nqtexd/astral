use astral_core::*;
fn solid(name: &str, color: Color) -> CompositeNode {
    CompositeNode {
        id: Id::new_v4(),
        name: name.into(),
        op: CompositeOp::Solid { color },
    }
}
#[test]
fn graph_cycles_rollback_atomically_and_undo_restores_graph() {
    let p = Project::new("Graph", 64, 64, 24, 24);
    let id = p.id;
    let mut session = Session::new(p).unwrap();
    let a = solid("A", [255, 0, 0, 255]);
    let graph = Composition {
        id: Id::new_v4(),
        name: "Graph".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: a.id,
        nodes: vec![a],
    };
    session
        .apply(Batch {
            project: id,
            expected_revision: 0,
            commands: vec![Command::SetCompositions {
                compositions: vec![graph.clone()],
            }],
        })
        .unwrap();
    assert_eq!(session.project.schema, 10);
    let mut broken = graph.clone();
    broken.nodes[0].op = CompositeOp::Blur {
        source: broken.nodes[0].id,
        radius: 4,
    };
    assert!(
        session
            .apply(Batch {
                project: id,
                expected_revision: 1,
                commands: vec![
                    Command::SetProject {
                        name: Some("Must roll back".into()),
                        duration: None,
                        background: None
                    },
                    Command::SetCompositions {
                        compositions: vec![broken]
                    }
                ]
            })
            .is_err()
    );
    assert_eq!(session.project.name, "Graph");
    assert_eq!(session.project.revision, 1);
    assert_eq!(session.project.compositions[0], graph);
    session.undo(id, 1).unwrap();
    assert!(session.project.compositions.is_empty());
    session.redo(id, 2).unwrap();
    assert_eq!(session.project.compositions[0], graph);
}
#[test]
fn graph_allocation_and_references_are_checked_before_render() {
    let mut p = Project::new("Budget", 8192, 8192, 24, 24);
    p.schema = 10;
    let a = solid("A", [0, 0, 0, 0]);
    p.compositions = vec![Composition {
        id: Id::new_v4(),
        name: "Too large".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: a.id,
        nodes: vec![a, solid("B", [0, 0, 0, 0])],
    }];
    assert!(p.validate().is_err());
    p.width = 64;
    p.height = 64;
    p.compositions[0].nodes[0].op = CompositeOp::Raster {
        layer: Id::new_v4(),
    };
    assert!(p.validate().is_err());
}
fn rectangle(x: f32, y: f32, w: f32, h: f32) -> Graphic {
    Graphic {
        id: Id::new_v4(),
        name: "Rectangle".into(),
        contours: vec![],
        kind: GraphicKind::Rectangle,
        start: 0,
        end: 23,
        size: Point::new(w, h),
        color: [20, 40, 80, 255],
        layer: GraphicLayer::Foreground,
        text: String::new(),
        keys: vec![GraphicKey {
            frame: 0,
            position: Point::new(x, y),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Hold,
        }],
        draw_on: false,
    }
}
#[test]
fn boolean_subtraction_preserves_a_hole_and_source_shapes() {
    let mut p = Project::new("Paths", 64, 64, 24, 24);
    p.schema = 2;
    let a = rectangle(32., 32., 40., 40.);
    let b = rectangle(32., 32., 20., 20.);
    let (left, right) = (a.id, b.id);
    p.graphics = vec![a, b];
    let mut session = Session::new(p).unwrap();
    let result = session
        .apply(Batch {
            project: session.project.id,
            expected_revision: 0,
            commands: vec![Command::BooleanGraphics {
                left,
                right,
                frame: 0,
                operation: vector::BooleanOperation::Subtract,
            }],
        })
        .unwrap();
    assert_eq!(result.created_ids.len(), 1);
    assert_eq!(session.project.graphics.len(), 3);
    let contours = &session.project.graphics[2].contours;
    assert_eq!(contours.len(), 2);
    let area = |points: &Vec<Point>| {
        points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .take(points.len())
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum::<f32>()
    };
    assert!(area(&contours[0]) * area(&contours[1]) < 0.);
}
