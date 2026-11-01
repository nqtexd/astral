use astral::{
    artwork::ArtworkRequest,
    service::{Request, Service},
};
use astral_core::*;
fn wait(service: &Service, value: serde_json::Value) -> serde_json::Value {
    let id = serde_json::from_value(value["job"].clone()).unwrap();
    for _ in 0..1000 {
        let v = service.request(Request::JobStatus { job: id }).unwrap();
        match v["state"].as_str() {
            Some("completed") => return v,
            Some("failed" | "cancelled") => panic!("{v}"),
            _ => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    panic!("Artwork job timed out")
}
#[test]
fn sequence_paint_changes_only_one_frame_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let value = service
        .request(Request::CreateProject {
            name: "Raster".into(),
            width: 64,
            height: 64,
            fps: 24,
            duration: 24,
        })
        .unwrap();
    let id = serde_json::from_value(value["project"].clone()).unwrap();
    for (name, color) in [("a.png", [255, 0, 0, 255]), ("b.png", [0, 0, 255, 255])] {
        image::RgbaImage::from_pixel(8, 8, image::Rgba(color))
            .save(dir.path().join(name))
            .unwrap();
    }
    wait(
        &service,
        service
            .artwork(ArtworkRequest::Import {
                project: id,
                expected_revision: 0,
                paths: vec!["a.png".into(), "b.png".into()],
                start: 0,
                source_fps: 24,
                looping: true,
            })
            .unwrap(),
    );
    let p = service.snapshot(id).unwrap();
    let layer = p.rasters[0].id;
    let original0 = astral_render::rgba(&p, 0.).unwrap();
    let original1 = astral_render::rgba(&p, 1.).unwrap();
    service
        .artwork(ArtworkRequest::Paint {
            project: id,
            expected_revision: p.revision,
            layer,
            frame: 0,
            points: vec![Point::new(4., 4.)],
            radius: 3.,
            color: [0, 255, 0, 255],
            erase: false,
        })
        .unwrap();
    let p = service.snapshot(id).unwrap();
    assert_ne!(astral_render::rgba(&p, 0.).unwrap(), original0);
    assert_eq!(astral_render::rgba(&p, 1.).unwrap(), original1);
    assert_eq!(p.rasters[0].bitmap_at(2., 24), p.rasters[0].sources[0]);
    service
        .request(Request::SaveProject {
            project: id,
            path: "raster.astral".into(),
        })
        .unwrap();
    let reopened = service.workspace.load("raster.astral").unwrap();
    assert_eq!(
        astral_render::rgba(&p, 0.).unwrap(),
        astral_render::rgba(&reopened, 0.).unwrap()
    );
    assert!(
        service
            .artwork(ArtworkRequest::Paint {
                project: id,
                expected_revision: 0,
                layer,
                frame: 0,
                points: vec![Point::new(1., 1.)],
                radius: 1.,
                color: [0; 4],
                erase: true
            })
            .is_err()
    );
    let revision = service.snapshot(id).unwrap().revision;
    service
        .request(Request::ApplyCommands {
            batch: Batch {
                project: id,
                expected_revision: revision,
                commands: vec![Command::Remove { id: layer }],
            },
        })
        .unwrap();
    let deleted = service.snapshot(id).unwrap();
    assert!(deleted.rasters.is_empty());
    assert!(deleted.bitmaps.is_empty());
    service
        .request(Request::Undo {
            project: id,
            expected_revision: deleted.revision,
        })
        .unwrap();
    assert_eq!(service.snapshot(id).unwrap().rasters.len(), 1);
}
#[test]
fn recording_wav_preserves_stereo_rate_without_opening_microphone() {
    let bytes = astral::recording::wav(&[0., 0.5, -0.5, 1.], 48000, 2).unwrap();
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(reader.spec().sample_rate, 48000);
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.duration(), 2);
    let samples = reader
        .samples::<i16>()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(samples, [0, 16384, -16384, 32767]);
}

#[test]
fn rgb_psd_composite_import_and_corrupt_file_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let value = service
        .request(Request::CreateProject {
            name: "PSD".into(),
            width: 64,
            height: 64,
            fps: 24,
            duration: 24,
        })
        .unwrap();
    let id = serde_json::from_value(value["project"].clone()).unwrap();
    let mut bytes = b"8BPS".to_vec();
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&[0; 6]);
    bytes.extend_from_slice(&3u16.to_be_bytes());
    bytes.extend_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(&8u16.to_be_bytes());
    bytes.extend_from_slice(&3u16.to_be_bytes());
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(&0u16.to_be_bytes());
    bytes.extend_from_slice(&[255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0]);
    service
        .workspace
        .atomic_write("sample.psd", &bytes)
        .unwrap();
    let result = wait(
        &service,
        service
            .artwork(ArtworkRequest::Import {
                project: id,
                expected_revision: 0,
                paths: vec!["sample.psd".into()],
                start: 0,
                source_fps: 24,
                looping: false,
            })
            .unwrap(),
    );
    assert!(!result["warnings"].as_array().unwrap().is_empty());
    let p = service.snapshot(id).unwrap();
    assert_eq!(p.rasters.len(), 1);
    let decoded = astral_render::decode_bitmap(&p.bitmaps[0]).unwrap();
    assert_eq!(&decoded.pixels[..4], &[255, 0, 0, 255]);
    service.workspace.atomic_write("bad.psd", b"8BPS").unwrap();
    let job = service
        .artwork(ArtworkRequest::Import {
            project: id,
            expected_revision: p.revision,
            paths: vec!["bad.psd".into()],
            start: 0,
            source_fps: 24,
            looping: false,
        })
        .unwrap();
    let job = serde_json::from_value(job["job"].clone()).unwrap();
    for _ in 0..500 {
        let v = service.request(Request::JobStatus { job }).unwrap();
        if v["state"] == "failed" {
            assert_eq!(service.snapshot(id).unwrap().revision, p.revision);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("Corrupt PSD job did not fail");
}

#[test]
fn editable_mouth_cues_keep_schema_and_text_fit_is_non_mutating() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let mut p = Project::new("Voice", 640, 360, 24, 48);
    let c = stick_character(
        "Speaker".into(),
        Point::new(100., 200.),
        [240, 190, 120, 255],
    );
    let character = c.id;
    p.characters.push(c);
    let samples: Vec<_> = (0..48000)
        .map(|i| {
            if (12000..36000).contains(&i) {
                (i as f32 * 0.1).sin() * 0.3
            } else {
                0.
            }
        })
        .collect();
    service
        .workspace
        .atomic_write(
            "voice.wav",
            &astral::recording::wav(&samples, 24000, 1).unwrap(),
        )
        .unwrap();
    let asset = Asset {
        id: Id::new_v4(),
        name: "Test speech".into(),
        path: "voice.wav".into(),
        kind: "audio".into(),
        tags: vec![],
    };
    let clip = AudioClip {
        id: Id::new_v4(),
        name: asset.name.clone(),
        asset: asset.id,
        start: 0,
        offset_seconds: 0.,
        duration_seconds: 2.,
        gain: 1.,
        fade_in: 0.,
        fade_out: 0.,
        looping: false,
    };
    let clip_id = clip.id;
    p.assets.push(asset);
    p.audio.push(clip);
    let project = p.id;
    service.insert(p).unwrap();
    service
        .artwork(ArtworkRequest::LipSync {
            project,
            expected_revision: 0,
            clip: clip_id,
            character,
            text: "Make it move".into(),
            threshold: None,
        })
        .unwrap();
    let p = service.snapshot(project).unwrap();
    assert_eq!(p.schema, 8);
    assert_eq!(p.mouths[0].keys[0].shape, MouthShape::Rest);
    assert!(p.mouths[0].keys.iter().any(|k| k.shape != MouthShape::Rest));
    assert_eq!(p.mouths[0].keys.last().unwrap().shape, MouthShape::Rest);
    let d = Dialogue {
        id: Id::new_v4(),
        name: "Overflow".into(),
        text: "Editable words need enough room to remain readable in the final exported frame."
            .into(),
        speaker: String::new(),
        start: 0,
        end: 47,
        position: Point::new(300., 100.),
        size: Point::new(180., 35.),
        font_size: 24.,
        color: [30, 30, 30, 255],
        text_color: [255; 4],
        appearance: Default::default(),
        style: DialogueStyle::Caption,
        character: None,
        typewriter: 0.,
        formatting: Default::default(),
    };
    let mut candidate = p.clone();
    candidate.dialogues.push(d.clone());
    assert_eq!(astral_render::review_text(&candidate, 0, 47).len(), 1);
    let (fit, _) = astral_render::fit_and_place(&candidate, d.id).unwrap();
    assert_eq!(candidate.dialogues[0].size.y, 35.);
    candidate.dialogues[0] = fit;
    assert!(astral_render::review_text(&candidate, 0, 47).is_empty());
}

#[test]
fn tracking_previews_real_sequence_motion_without_mutating_camera() {
    let dir = tempfile::tempdir().unwrap();
    let service = Service::new(dir.path()).unwrap();
    let result = service
        .request(Request::CreateProject {
            name: "Tracking".into(),
            width: 128,
            height: 64,
            fps: 24,
            duration: 12,
        })
        .unwrap();
    let project = serde_json::from_value(result["project"].clone()).unwrap();
    let mut paths = vec![];
    for frame in 0..3 {
        let path = format!("frame{frame}.png");
        let image = image::RgbaImage::from_fn(128, 64, |x, y| {
            let x = x as i32 - frame * 4;
            image::Rgba(if (20..48).contains(&x) && (18..48).contains(&y) {
                let value = ((x * 73 + y as i32 * 37 + x * y as i32) % 200 + 30) as u8;
                [value, 255 - value, 100, 255]
            } else {
                [0, 0, 0, 255]
            })
        });
        image.save(dir.path().join(&path)).unwrap();
        paths.push(path);
    }
    wait(
        &service,
        service
            .artwork(ArtworkRequest::Import {
                project,
                expected_revision: 0,
                paths,
                start: 0,
                source_fps: 24,
                looping: false,
            })
            .unwrap(),
    );
    let p = service.snapshot(project).unwrap();
    let result = wait(
        &service,
        service
            .artwork(ArtworkRequest::Track {
                project,
                expected_revision: p.revision,
                layer: p.rasters[0].id,
                start: 0,
                end: 2,
                point: Point::new(32., 32.),
                apply: false,
            })
            .unwrap(),
    );
    assert_eq!(service.snapshot(project).unwrap().revision, p.revision);
    assert_eq!(result["points"][0]["source"]["x"], 32.);
    assert_eq!(result["points"][1]["source"]["x"], 36.);
    assert_eq!(result["points"][2]["source"]["x"], 40.);
    assert_eq!(result["plan"]["commands"].as_array().unwrap().len(), 3);
}
