use astral::{comic::ComicRequest, service::Service};
use astral_core::*;
fn fixture() -> (tempfile::TempDir, Service, Id, Id) {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let mut p = Project::new("Illustrated", 64, 64, 24, 24);
    p.schema = 9;
    p.background = [80, 120, 180, 255];
    let layer = DrawingLayer::new("Artwork");
    let source = layer.id;
    p.layers.push(layer);
    p.cels.push(DrawingCel {
        id: Id::new_v4(),
        layer: source,
        frame: 0,
        strokes: vec![InkStroke {
            brush: None,
            bitmap: None,
            skin: None,
            pressures: vec![],
            style: BrushStyle::Solid,
            filled: true,
            curved: false,
            points: vec![
                Point::new(8., 8.),
                Point::new(56., 8.),
                Point::new(56., 56.),
                Point::new(8., 56.),
            ],
            color: [200, 150, 180, 255],
            width: 1.,
        }],
    });
    let id = service.insert(p).unwrap();
    (dir, service, id, source)
}
#[test]
fn look_preview_apply_inspect_refine_and_stale_write() {
    let (_dir, s, id, source) = fixture();
    let request = |apply| ComicRequest::CreateLook {
        project: id,
        expected_revision: 0,
        source,
        styles: vec![ComicStyle::Halftone {
            spacing: 12.,
            angle: 0.7,
            ink: [0, 0, 0, 255],
            strength: 1.,
            keys: vec![],
        }],
        layer_fps: Some(12),
        apply,
    };
    let proposal = s.comic_tools(request(false)).unwrap();
    assert_eq!(s.snapshot(id).unwrap().revision, 0);
    assert!(!proposal["commands"].as_array().unwrap().is_empty());
    let result = s.comic_tools(request(true)).unwrap();
    let composition = serde_json::from_value(result["composition"].clone()).unwrap();
    assert_eq!(s.snapshot(id).unwrap().revision, 1);
    assert!(s.comic_tools(request(true)).is_err());
    let inspection = s
        .comic_tools(ComicRequest::InspectFrame {
            project: id,
            frame: 0.,
        })
        .unwrap();
    assert!(
        inspection["image_base64"]
            .as_str()
            .unwrap()
            .starts_with("iVBOR")
    );
    let original = s.snapshot(id).unwrap().compositions.clone();
    let refine = s
        .comic_tools(ComicRequest::Refine {
            project: id,
            expected_revision: 1,
            composition,
            frames: vec![0, 12],
            max_difference: 0.01,
            rounds: 3,
            apply: false,
        })
        .unwrap();
    assert!(refine["iterations"].as_u64().unwrap() > 0);
    assert!(
        refine["after_difference"].as_f64().unwrap()
            < refine["before_difference"].as_f64().unwrap()
    );
    assert_eq!(s.snapshot(id).unwrap().revision, 1);
    let applied = s
        .comic_tools(ComicRequest::Refine {
            project: id,
            expected_revision: 1,
            composition,
            frames: vec![0, 12],
            max_difference: 0.01,
            rounds: 3,
            apply: true,
        })
        .unwrap();
    assert_eq!(applied["applied"], true);
    assert_eq!(s.snapshot(id).unwrap().revision, 2);
    s.request(astral::service::Request::Undo {
        project: id,
        expected_revision: 2,
    })
    .unwrap();
    assert_eq!(s.snapshot(id).unwrap().compositions, original);
}
#[test]
fn refinement_and_inspection_reject_invalid_bounds_without_mutation() {
    let (_dir, s, id, _source) = fixture();
    assert!(
        s.comic_tools(ComicRequest::InspectFrame {
            project: id,
            frame: f64::NAN
        })
        .is_err()
    );
    assert!(
        s.comic_tools(ComicRequest::Refine {
            project: id,
            expected_revision: 0,
            composition: Id::new_v4(),
            frames: vec![24],
            max_difference: 0.1,
            rounds: 4,
            apply: true
        })
        .is_err()
    );
    assert_eq!(s.snapshot(id).unwrap().revision, 0);
}
