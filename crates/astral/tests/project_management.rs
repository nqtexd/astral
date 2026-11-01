use astral::service::{Request, Service};
use astral_core::*;
#[test]
fn rename_activity_trash_and_restore_preserve_edits_and_assets() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let p = Project::new("Original", 640, 360, 24, 48);
    let id = service.insert(p).unwrap();
    let file = dir.path().join("Projects/film.astral");
    service.save_local_project_as(id, &file).unwrap();
    std::fs::write(file.parent().unwrap().join("shared.wav"), b"shared asset").unwrap();
    let receipt = service
        .request(Request::RenameProject {
            project: id,
            expected_revision: 0,
            name: "New title".into(),
        })
        .unwrap();
    assert_eq!(receipt["revision"], 1);
    assert_eq!(service.snapshot(id).unwrap().name, "New title");
    let events = service.project_activity(id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].changes, vec!["name"]);
    assert!(events[0].timestamp > 0);
    assert!(service.trash_local_project(id, 0).is_err());
    let response = service
        .request(Request::TrashProject {
            project: id,
            expected_revision: 1,
        })
        .unwrap();
    let relative = response["trash_path"].as_str().unwrap();
    assert!(!std::path::Path::new(relative).is_absolute());
    let trash = service.workspace.path(relative).unwrap();
    assert!(trash.is_file() && !file.exists());
    assert!(service.snapshot(id).is_err());
    assert_eq!(
        std::fs::read(file.parent().unwrap().join("shared.wav")).unwrap(),
        b"shared asset"
    );
    let list = service.saved_project_files().unwrap();
    assert!(list["files"].as_array().unwrap().is_empty());
    std::fs::write(&file, b"occupied").unwrap();
    assert!(service.restore_local_project(&trash).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"occupied");
    std::fs::remove_file(&file).unwrap();
    let restored = service.restore_local_project(&trash).unwrap();
    assert_eq!(restored, id);
    assert_eq!(service.snapshot(id).unwrap().name, "New title");
    assert_eq!(service.project_activity(id).unwrap().len(), 1);
    assert!(file.is_file() && !trash.exists());
    let stale = service.request(Request::RenameProject {
        project: id,
        expected_revision: 0,
        name: "Stale".into(),
    });
    assert!(stale.is_err());
    assert_eq!(service.snapshot(id).unwrap().name, "New title");
}
#[test]
fn save_as_retains_local_activity_and_undo_is_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service
        .insert(Project::new("History", 640, 360, 24, 24))
        .unwrap();
    service
        .request(Request::RenameProject {
            project: id,
            expected_revision: 0,
            name: "Revised".into(),
        })
        .unwrap();
    let file = dir.path().join("nested/film.astral");
    service.save_local_project_as(id, &file).unwrap();
    assert_eq!(service.project_activity(id).unwrap().len(), 1);
    service
        .request(Request::Undo {
            project: id,
            expected_revision: 1,
        })
        .unwrap();
    assert_eq!(service.snapshot(id).unwrap().name, "History");
    let events = service.project_activity(id).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].action, "Undo");
}
