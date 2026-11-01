use astral::{
    production::ProductionRequest,
    service::{Request, Service},
};
use astral_core::{studio::*, *};

#[test]
fn motion_tool_schema_has_an_object_root_and_the_mcp_router_starts() {
    let schema = serde_json::to_value(schemars::schema_for!(ProductionRequest)).unwrap();
    assert_eq!(schema["type"], "object");
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let _server = astral::mcp::AstralMcp::new(service, false);
}

fn edit(
    service: &Service,
    id: Id,
    revision: u64,
    tool: StudioOperation,
    apply: bool,
    frame: Option<u32>,
) -> anyhow::Result<serde_json::Value> {
    service.production(ProductionRequest::Edit {
        project: id,
        expected_revision: revision,
        tool,
        apply,
        preview_frame: frame,
    })
}

#[test]
fn invalid_preview_and_stale_revision_never_commit_a_plan() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let id = service
        .insert(Project::new("Preview", 640, 360, 24, 48))
        .unwrap();
    let tool = || StudioOperation::Rig {
        template: RigTemplate::Human,
        name: "Actor".into(),
        position: Point::new(320., 220.),
        height: 180.,
        color: [20, 20, 20, 255],
    };
    assert!(edit(&service, id, 0, tool(), true, Some(99)).is_err());
    assert_eq!(service.snapshot(id).unwrap().revision, 0);
    assert!(service.snapshot(id).unwrap().characters.is_empty());
    let preview = edit(&service, id, 0, tool(), false, Some(0)).unwrap();
    assert!(preview["image_base64"].as_str().unwrap().len() > 100);
    assert_ne!(preview["image_base64"], preview["original_image_base64"]);
    assert_eq!(service.snapshot(id).unwrap().revision, 0);
    edit(&service, id, 0, tool(), true, None).unwrap();
    assert!(edit(&service, id, 0, tool(), true, None).is_err());
    assert_eq!(service.snapshot(id).unwrap().characters.len(), 1);
    service
        .request(Request::Undo {
            project: id,
            expected_revision: 1,
        })
        .unwrap();
    assert!(service.snapshot(id).unwrap().characters.is_empty());
}

#[test]
fn library_files_roundtrip_with_fresh_ids_and_reject_traversal_and_invalid_data() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let mut p = Project::new("Library", 640, 360, 24, 48);
    let actor = stick_character("Actor".into(), Point::new(300., 240.), [20, 20, 20, 255]);
    let aid = actor.id;
    p.characters.push(actor);
    let id = service.insert(p).unwrap();
    edit(
        &service,
        id,
        0,
        StudioOperation::SaveLibrary {
            name: "Idle".into(),
            kind: LibraryKind::Motion,
            characters: vec![aid],
            layer: None,
            start: 0,
            end: 23,
        },
        true,
        None,
    )
    .unwrap();
    service
        .production(ProductionRequest::ExportLibrary {
            project: id,
            path: "library/demo.astral-library.json".into(),
            entries: vec![],
        })
        .unwrap();
    service
        .production(ProductionRequest::ImportLibrary {
            project: id,
            expected_revision: 1,
            path: "library/demo.astral-library.json".into(),
        })
        .unwrap();
    let p = service.snapshot(id).unwrap();
    let entries = library_entries(&p).unwrap();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].id, entries[1].id);
    assert!(
        service
            .production(ProductionRequest::ExportLibrary {
                project: id,
                path: "../escape.astral-library.json".into(),
                entries: vec![]
            })
            .is_err()
    );
    service
        .workspace
        .atomic_write(
            "library/bad.astral-library.json",
            br#"{"format":"astral_library","version":1,"entries":[{"id":"bad"}]}"#,
        )
        .unwrap();
    assert!(
        service
            .production(ProductionRequest::ImportLibrary {
                project: id,
                expected_revision: 2,
                path: "library/bad.astral-library.json".into()
            })
            .is_err()
    );
    assert_eq!(service.snapshot(id).unwrap().revision, 2);
    service
        .request(Request::SaveProject {
            project: id,
            path: "film.astral".into(),
        })
        .unwrap();
    let reopened = service.workspace.load("film.astral").unwrap();
    assert_eq!(
        serde_json::to_value(entries).unwrap(),
        serde_json::to_value(library_entries(&reopened).unwrap()).unwrap()
    );
}

#[test]
fn waveform_uses_real_confined_wav_and_respects_column_budget() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let mut p = Project::new("Sound", 640, 360, 24, 48);
    let aid = Id::new_v4();
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut wav = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..8000 {
            wav.write_sample(if i < 4000 { 0i16 } else { 16000i16 })
                .unwrap();
        }
        wav.finalize().unwrap();
    }
    service
        .workspace
        .atomic_write("assets/cue.wav", bytes.get_ref())
        .unwrap();
    p.assets.push(Asset {
        id: aid,
        name: "Cue".into(),
        path: "assets/cue.wav".into(),
        kind: "audio".into(),
        tags: vec![],
    });
    let id = service.insert(p).unwrap();
    let wave = service
        .production(ProductionRequest::Waveform {
            project: id,
            asset: aid,
            columns: 32,
        })
        .unwrap();
    assert_eq!(wave["duration"], 1.0);
    let peaks = wave["peaks"].as_array().unwrap();
    assert_eq!(peaks.len(), 32);
    assert_eq!(peaks[0], 0.0);
    assert!(peaks[31].as_f64().unwrap() > 0.45);
    assert!(
        service
            .production(ProductionRequest::Waveform {
                project: id,
                asset: aid,
                columns: 1_000_000
            })
            .is_err()
    );
}
