use astral::{
    cinematic::{CinematicRequest, DepthAssignment},
    service::{Request, Service},
};
use astral_core::*;
fn fixture() -> (tempfile::TempDir, Service, Id, Id, Id) {
    let dir = tempfile::tempdir().unwrap();
    let s = Service::new(dir.path()).unwrap();
    let mut p = Project::new("Stage", 64, 64, 24, 24);
    p.schema = 10;
    let layer = DrawingLayer::new("Plane");
    let l = layer.id;
    p.layers.push(layer);
    let a = Id::new_v4();
    let graph = Id::new_v4();
    p.compositions.push(Composition {
        id: graph,
        name: "Frame".into(),
        enabled: true,
        layer: GraphicLayer::Foreground,
        output: a,
        nodes: vec![CompositeNode {
            id: a,
            name: "Base".into(),
            op: CompositeOp::Solid {
                color: [80, 100, 120, 255],
            },
        }],
    });
    let id = s.insert(p).unwrap();
    (dir, s, id, l, graph)
}
#[test]
fn staging_and_finishing_are_previewable_revision_checked_and_undoable() {
    let (_dir, s, id, layer, graph) = fixture();
    let shot = |apply| CinematicRequest::StageShot {
        project: id,
        expected_revision: 0,
        keys: vec![
            CameraKey {
                frame: 0,
                center: Point::new(32., 32.),
                zoom: 1.,
            },
            CameraKey {
                frame: 23,
                center: Point::new(38., 32.),
                zoom: 1.1,
            },
        ],
        easing: Easing::Smooth,
        layers: vec![DepthAssignment {
            layer,
            depth: Some(LayerDepth {
                parallax: 0.5,
                zoom: 0.5,
                anchor: None,
            }),
        }],
        preview_frame: 12,
        apply,
    };
    let proposal = s.cinematic_tools(shot(false)).unwrap();
    assert_eq!(proposal["applied"], false);
    assert!(
        proposal["image_base64"]
            .as_str()
            .unwrap()
            .starts_with("iVBOR")
    );
    assert_eq!(s.snapshot(id).unwrap().revision, 0);
    s.cinematic_tools(shot(true)).unwrap();
    assert_eq!(s.snapshot(id).unwrap().schema, 13);
    assert!(s.cinematic_tools(shot(true)).is_err());
    let finish = |apply| CinematicRequest::Finish {
        project: id,
        expected_revision: 1,
        composition: graph,
        stages: vec![CinematicStage {
            effect: CinematicEffect::Grade {
                exposure: 1.,
                contrast: 1.,
                saturation: 1.,
                lift: [0.; 3],
                gamma: [1.; 3],
                gain: [1.; 3],
            },
            mix: 1.,
            keys: vec![],
        }],
        preview_frame: 12,
        apply,
    };
    let before = s.cinematic_tools(finish(false)).unwrap();
    assert_ne!(before["image_base64"], before["original_image_base64"]);
    assert_eq!(s.snapshot(id).unwrap().revision, 1);
    s.cinematic_tools(finish(true)).unwrap();
    assert_eq!(s.snapshot(id).unwrap().revision, 2);
    let review = s
        .cinematic_tools(CinematicRequest::Review {
            project: id,
            frames: vec![0, 12, 23],
        })
        .unwrap();
    assert_eq!(review["metrics"].as_array().unwrap().len(), 3);
    assert!(
        review["image_base64"]
            .as_str()
            .unwrap()
            .starts_with("iVBOR")
    );
    s.request(Request::Undo {
        project: id,
        expected_revision: 2,
    })
    .unwrap();
    assert_eq!(s.snapshot(id).unwrap().compositions[0].nodes.len(), 1);
}
#[test]
fn invalid_review_ranges_and_staging_never_partially_commit() {
    let (_dir, s, id, layer, _graph) = fixture();
    assert!(
        s.cinematic_tools(CinematicRequest::Review {
            project: id,
            frames: vec![12, 0]
        })
        .is_err()
    );
    assert!(
        s.cinematic_tools(CinematicRequest::StageShot {
            project: id,
            expected_revision: 0,
            keys: vec![CameraKey {
                frame: 0,
                center: Point::new(32., 32.),
                zoom: 1.
            }],
            easing: Easing::Linear,
            layers: vec![DepthAssignment {
                layer,
                depth: Some(LayerDepth {
                    parallax: f32::NAN,
                    zoom: 1.,
                    anchor: None
                })
            }],
            preview_frame: 0,
            apply: true
        })
        .is_err()
    );
    assert_eq!(s.snapshot(id).unwrap().revision, 0);
    assert!(s.snapshot(id).unwrap().camera.is_empty());
}
