#[cfg(windows)]
#[test]
fn preview_service_returns_the_versioned_path_when_viewer_holds_the_old_image() {
    use astral::service::{Request, Service};
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let result = service
        .request(Request::CreateProject {
            name: "Preview lock".into(),
            width: 640,
            height: 360,
            fps: 24,
            duration: 24,
        })
        .unwrap();
    let id = serde_json::from_value(result["project"].clone()).unwrap();
    let request = || Request::RenderPreview {
        project: id,
        frame: 0.,
        width: 320,
        height: 180,
        path: Some("preview.png".into()),
    };
    service.request(request()).unwrap();
    let original = std::fs::read(dir.path().join("preview.png")).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(dir.path().join("preview.png"))
        .unwrap();
    let result = service.request(request()).unwrap();
    assert_ne!(result["path"], "preview.png");
    assert_eq!(result["requested_path"], "preview.png");
    assert_eq!(result["warnings"].as_array().unwrap().len(), 1);
    assert!(
        result["layout_review"]["issues"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read(dir.path().join(result["path"].as_str().unwrap())).unwrap(),
        original
    );
    drop(lock);
    assert_eq!(
        std::fs::read(dir.path().join("preview.png")).unwrap(),
        original
    );
}
