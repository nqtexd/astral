use astral::{
    assistance::AssistRequest,
    service::{Request, Service},
};
use astral_core::*;
fn wait(service: &Service, value: serde_json::Value) -> serde_json::Value {
    let job = serde_json::from_value(value["job"].clone()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let value = service.request(Request::JobStatus { job }).unwrap();
        if matches!(
            value["state"].as_str(),
            Some("completed" | "failed" | "cancelled")
        ) {
            return value;
        }
        assert!(std::time::Instant::now() < deadline, "{value}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
#[test]
fn managed_project_creation_open_close_and_confined_library() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let created = service
        .request(Request::CreateManagedProject {
            name: "Agent film".into(),
            width: 640,
            height: 360,
            fps: 24,
            duration: 48,
            path: "Projects/Film/Film.astral".into(),
            background: None,
        })
        .unwrap();
    let id = serde_json::from_value(created["project"].clone()).unwrap();
    assert!(service.project_file(id).unwrap().is_file());
    assert_eq!(service.snapshot(id).unwrap().layers.len(), 1);
    assert!(
        service.request(Request::ListSavedProjects).unwrap()["files"]
            .as_array()
            .unwrap()
            .len()
            == 1
    );
    service
        .request(Request::ApplyCommands {
            batch: Batch {
                project: id,
                expected_revision: 0,
                commands: vec![Command::SetProject {
                    name: Some("Edited".into()),
                    duration: None,
                    background: None,
                }],
            },
        })
        .unwrap();
    assert!(
        service
            .request(Request::CloseProject {
                project: id,
                expected_revision: 1,
                discard: false
            })
            .is_err()
    );
    service
        .request(Request::SaveProject {
            project: id,
            path: "Film.astral".into(),
        })
        .unwrap();
    service
        .request(Request::CloseProject {
            project: id,
            expected_revision: 1,
            discard: false,
        })
        .unwrap();
    let opened = service
        .request(Request::OpenProject {
            path: "Projects/Film/Film.astral".into(),
        })
        .unwrap();
    assert_eq!(opened["project"], created["project"]);
    assert_eq!(service.snapshot(id).unwrap().name, "Edited");
    assert!(
        service
            .request(Request::CreateManagedProject {
                name: "Bad".into(),
                width: 100,
                height: 100,
                fps: 24,
                duration: 24,
                path: "../Bad.astral".into(),
                background: None
            })
            .is_err()
    );
}
#[test]
fn stroke_proposals_require_the_original_revision_and_do_not_auto_apply() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service
        .insert(Project::new("Drawing", 320, 240, 24, 24))
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
                            points: vec![Point::new(10., 100.), Point::new(100., 100.)],
                            color: [0, 0, 0, 255],
                            width: 3.,
                        }],
                    },
                ],
            },
        })
        .unwrap();
    let proposed = service
        .assist(AssistRequest::SuggestStroke {
            project: id,
            layer: lid,
            frame: 0,
            strength: 0.5,
        })
        .unwrap();
    assert_eq!(service.snapshot(id).unwrap().revision, 1);
    let commands = serde_json::from_value(proposed["commands"].clone()).unwrap();
    service
        .request(Request::ApplyCommands {
            batch: Batch {
                project: id,
                expected_revision: 1,
                commands: vec![Command::SetProject {
                    name: Some("Changed".into()),
                    duration: None,
                    background: None,
                }],
            },
        })
        .unwrap();
    assert!(
        service
            .request(Request::ApplyCommands {
                batch: Batch {
                    project: id,
                    expected_revision: 1,
                    commands
                }
            })
            .is_err()
    );
    assert_eq!(
        service
            .snapshot(id)
            .unwrap()
            .cel_at(lid, 0)
            .unwrap()
            .strokes[0]
            .points
            .len(),
        2
    );
}
#[cfg(windows)]
#[test]
fn offline_voice_creates_audible_audio_and_matching_editable_dialogue() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let mut source = Project::new("Voice", 256, 320, 24, 24);
    let character = stick_character("Speaker".into(), Point::new(128., 210.), [10, 10, 10, 255]);
    let character_id = character.id;
    source.characters.push(character);
    let id = service.insert(source).unwrap();
    let voices = service.assist(AssistRequest::ListVoices).unwrap();
    assert!(!voices["voices"].as_array().unwrap().is_empty());
    let queued = service
        .assist(AssistRequest::Voice {
            project: id,
            expected_revision: 0,
            text: "Astral keeps your drawings editable.".into(),
            start: 0,
            character: Some(character_id),
            options: Default::default(),
            dialogue: true,
            formatting: TextFormatting {
                font: TextFont::Bold,
                align: TextAlign::Center,
                ..Default::default()
            },
            speaker: "Narrator".into(),
        })
        .unwrap();
    let result = wait(&service, queued);
    assert_eq!(result["state"], "completed", "{result}");
    let p = service.snapshot(id).unwrap();
    assert_eq!(p.audio.len(), 1);
    assert_eq!(p.dialogues.len(), 1);
    assert_eq!(p.schema, 4);
    let position = p.dialogues[0].position + p.characters[0].keys[0].pose.root;
    assert!(position.x >= 0. && position.y >= 0.);
    assert!(position.x + p.dialogues[0].size.x <= p.width as f32);
    assert!(position.y + p.dialogues[0].size.y <= p.height as f32);
    let wav = astral_audio::mix(&p, &service.project_workspace(id)).unwrap();
    let mut reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
    assert!(reader.samples::<i16>().any(|s| s.unwrap() != 0));
    assert_eq!(p.dialogues[0].end + 1, p.duration);
    assert!(p.audio[0].duration_seconds > 0.5);
    service
        .request(Request::Undo {
            project: id,
            expected_revision: p.revision,
        })
        .unwrap();
    let p = service.snapshot(id).unwrap();
    assert!(p.audio.is_empty() && p.dialogues.is_empty() && p.assets.is_empty());
    assert_eq!(p.duration, 24);
}
