use astral::service::{Request, Service};
use astral_core::*;

#[test]
fn learned_projects_reopen_with_identical_models_and_preview() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service.demo().unwrap();
    let p = service.snapshot(id).unwrap();
    let before = astral_render::png(&p, 64., 960, 540).unwrap();
    service.workspace.save("learned.astral", &p).unwrap();
    let after = service.workspace.load("learned.astral").unwrap();
    assert_eq!(
        serde_json::to_value(&p.learned_motions).unwrap(),
        serde_json::to_value(&after.learned_motions).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&p.drawing_motions).unwrap(),
        serde_json::to_value(&after.drawing_motions).unwrap()
    );
    assert_eq!(before, astral_render::png(&after, 64., 960, 540).unwrap());
}

#[test]
fn starting_demo_never_replaces_user_sound_assets() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    service
        .workspace
        .atomic_write("assets/impact.wav", b"user-owned sound")
        .unwrap();
    let a = service.demo().unwrap();
    let b = service.demo().unwrap();
    assert_eq!(
        service.workspace.read("assets/impact.wav").unwrap(),
        b"user-owned sound"
    );
    assert_ne!(
        service.snapshot(a).unwrap().assets[0].path,
        service.snapshot(b).unwrap().assets[0].path
    );
}

#[test]
fn create_edit_save_reopen_and_render() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service.demo().unwrap();
    let p = service.snapshot(id).unwrap();
    service
        .request(Request::SaveProject {
            project: id,
            path: "projects/demo.astral".into(),
        })
        .unwrap();
    let fresh = Service::new(dir.path()).unwrap();
    fresh
        .request(Request::OpenProject {
            path: "projects/demo.astral".into(),
        })
        .unwrap();
    assert_eq!(fresh.snapshot(id).unwrap().revision, p.revision);
    let preview = fresh
        .request(Request::RenderPreview {
            project: id,
            frame: 104.,
            width: 480,
            height: 270,
            path: Some("renders/preview.png".into()),
        })
        .unwrap();
    assert!(preview["image_base64"].as_str().unwrap().len() > 1000);
    assert!(dir.path().join("renders/preview.png").is_file());
}
#[test]
fn inspect_and_history_share_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service.demo().unwrap();
    let p = service.snapshot(id).unwrap();
    let batch = Batch {
        project: id,
        expected_revision: p.revision,
        commands: vec![Command::SetProject {
            name: Some("Agent edit".into()),
            duration: None,
            background: None,
        }],
    };
    service.request(Request::ApplyCommands { batch }).unwrap();
    let inspected = service
        .request(Request::InspectProject {
            project: id,
            frame: Some(104.),
        })
        .unwrap();
    assert_eq!(inspected["document"]["name"], "Agent edit");
    service
        .request(Request::Undo {
            project: id,
            expected_revision: p.revision + 1,
        })
        .unwrap();
    assert_eq!(service.snapshot(id).unwrap().name, p.name);
}
#[test]
fn mixer_produces_audible_separate_cues() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service.demo().unwrap();
    let p = service.snapshot(id).unwrap();
    let bytes = astral_audio::mix(&p, &service.workspace).unwrap();
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(reader.spec().sample_rate, 48000);
    let samples: Vec<i16> = reader.samples().collect::<Result<_, _>>().unwrap();
    assert!(samples[..10000].iter().all(|v| *v == 0));
    let onset = (104. * 48000. / 30.) as usize * 2;
    assert!(samples[onset..onset + 1000].iter().any(|v| v.abs() > 100));
}
#[test]
fn workspace_confinement_applies_to_agent_requests() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service.demo().unwrap();
    assert!(
        service
            .request(Request::SaveProject {
                project: id,
                path: "../escape.astral".into()
            })
            .is_err()
    );
    assert!(
        service
            .request(Request::RenderPreview {
                project: id,
                frame: f64::NAN,
                width: 10,
                height: 10,
                path: None
            })
            .is_err()
    );
}
