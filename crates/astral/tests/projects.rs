use astral::service::{Request, Service};
use astral_core::*;
#[test]
fn save_as_copies_media_and_reopens_without_changing_scene_or_sound() {
    let base = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let service = Service::new(base.path()).unwrap();
    let id = service.demo().unwrap();
    let before = service.snapshot(id).unwrap();
    let audio = astral_audio::mix(&before, &service.project_workspace(id)).unwrap();
    let file = other.path().join("Film.astral");
    service.save_local_project_as(id, &file).unwrap();
    let saved = service.project_workspace(id).load("Film.astral").unwrap();
    assert_eq!(
        serde_json::to_value(&saved).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(
        audio,
        astral_audio::mix(&saved, &service.project_workspace(id)).unwrap()
    );
    assert_eq!(service.open_local_project(&file).unwrap(), id);
    service.close_local_project(id).unwrap();
    let reopened = service.open_local_project(&file).unwrap();
    assert_eq!(reopened, id);
    assert_eq!(
        audio,
        astral_audio::mix(
            &service.snapshot(id).unwrap(),
            &service.project_workspace(id)
        )
        .unwrap()
    );
}
#[test]
fn save_as_rejects_asset_conflicts_and_preserves_existing_destination() {
    let base = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let service = Service::new(base.path()).unwrap();
    let id = service.demo().unwrap();
    let p = service.snapshot(id).unwrap();
    let dest = astral_storage::Workspace::new(other.path()).unwrap();
    dest.atomic_write(&p.assets[0].path, b"unrelated sound")
        .unwrap();
    dest.atomic_write("Film.astral", b"previous file").unwrap();
    assert!(
        service
            .save_local_project_as(id, &other.path().join("Film.astral"))
            .is_err()
    );
    assert_eq!(dest.read("Film.astral").unwrap(), b"previous file");
    assert_eq!(dest.read(&p.assets[0].path).unwrap(), b"unrelated sound");
    assert_eq!(
        service.project_workspace(id).root(),
        service.workspace.root()
    );
}
#[test]
fn agent_paths_stay_confined_to_the_document_root_after_native_open() {
    let base = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let service = Service::new(base.path()).unwrap();
    let id = service
        .insert(Project::new("Film", 100, 100, 24, 24))
        .unwrap();
    service
        .save_local_project_as(id, &other.path().join("Film.astral"))
        .unwrap();
    service
        .request(Request::SaveProject {
            project: id,
            path: "copies/Film.astral".into(),
        })
        .unwrap();
    assert!(other.path().join("copies/Film.astral").is_file());
    assert!(!base.path().join("copies/Film.astral").exists());
    assert!(
        service
            .request(Request::SaveProject {
                project: id,
                path: "../escape.astral".into()
            })
            .is_err()
    );
}
#[test]
fn cel_projects_round_trip_with_identical_rendered_frames() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service
        .insert(Project::new("Cels", 100, 100, 24, 24))
        .unwrap();
    let layer = DrawingLayer::new("Ink");
    let lid = layer.id;
    service
        .request(Request::ApplyCommands {
            batch: Batch {
                project: id,
                expected_revision: 0,
                commands: vec![
                    Command::AddLayer { layer },
                    Command::SetCel {
                        layer: lid,
                        frame: 0,
                        strokes: vec![InkStroke {
                            brush: None,
                            bitmap: None,
                            skin: None,
                            pressures: vec![],
                            style: BrushStyle::Solid,
                            filled: false,
                            curved: false,
                            points: vec![Point::new(10., 50.), Point::new(90., 50.)],
                            color: [255, 0, 0, 255],
                            width: 8.,
                        }],
                    },
                    Command::SetCel {
                        layer: lid,
                        frame: 12,
                        strokes: vec![],
                    },
                ],
            },
        })
        .unwrap();
    service
        .request(Request::SaveProject {
            project: id,
            path: "cels.astral".into(),
        })
        .unwrap();
    let before = service.snapshot(id).unwrap();
    let after = service.workspace.load("cels.astral").unwrap();
    for frame in [0., 5., 11., 12., 23.] {
        assert_eq!(
            astral_render::png(&before, frame, 100, 100).unwrap(),
            astral_render::png(&after, frame, 100, 100).unwrap()
        );
    }
    assert_ne!(
        astral_render::png(&after, 11., 100, 100).unwrap(),
        astral_render::png(&after, 12., 100, 100).unwrap()
    );
}
