//! Reproducible, editable README demonstrations. Every preview is rendered by Astral.
use anyhow::Result;
use astral::{
    artwork::ArtworkRequest,
    service::{Request, Service},
};
use astral_core::{studio::*, *};
fn key(frame: u32, x: f32, y: f32, scale: f32, rotation: f32) -> GraphicKey {
    GraphicKey {
        frame,
        position: Point::new(x, y),
        scale,
        rotation,
        opacity: 1.,
        easing: Easing::Flow,
    }
}
#[allow(clippy::too_many_arguments)]
fn shape(
    p: &mut Project,
    name: &str,
    kind: GraphicKind,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: Color,
) {
    p.graphics.push(Graphic {
        id: Id::new_v4(),
        name: name.into(),
        kind,
        start: 0,
        end: p.duration - 1,
        size: Point::new(w, h),
        color,
        layer: GraphicLayer::Background,
        text: String::new(),
        keys: vec![key(0, x, y, 1., 0.)],
        draw_on: false,
        contours: vec![],
    });
}
fn title(p: &mut Project, name: &str, subtitle: &str) {
    p.dialogues.push(Dialogue {
        id: Id::new_v4(),
        name: "Title".into(),
        text: name.into(),
        speaker: subtitle.into(),
        start: 0,
        end: p.duration - 1,
        position: Point::new(42., 28.),
        size: Point::new(p.width as f32 - 84., 86.),
        font_size: 29.,
        color: [0; 4],
        text_color: [238, 242, 248, 255],
        appearance: DialogueAppearance {
            border_width: 0.,
            speaker_color: [117, 141, 168, 255],
            ..DialogueAppearance::dark()
        },
        style: DialogueStyle::Caption,
        character: None,
        typewriter: 0.,
        formatting: TextFormatting {
            font: TextFont::Bold,
            padding: 0.,
            ..Default::default()
        },
    });
}
fn apply(p: Project, commands: Vec<Command>) -> Result<Project> {
    let mut session = Session::new(p)?;
    session.apply(Batch {
        project: session.project.id,
        expected_revision: session.project.revision,
        commands,
    })?;
    Ok(session.project)
}
fn save(service: &Service, p: Project, name: &str) -> Result<Id> {
    p.validate()?;
    let id = service.insert(p)?;
    service.request(Request::SaveProject {
        project: id,
        path: format!("{name}.astral"),
    })?;
    Ok(id)
}
fn main() -> Result<()> {
    let destination = std::env::args()
        .nth(1)
        .unwrap_or("docs/showcase/projects".into());
    let service = Service::new(destination)?;
    let mut p = Project::new("Contact / 01", 960, 540, 60, 360);
    p.schema = 8;
    p.background = [10, 15, 25, 255];
    shape(
        &mut p,
        "Arena",
        GraphicKind::Ellipse,
        480.,
        452.,
        740.,
        62.,
        [18, 29, 44, 255],
    );
    for x in (60..960).step_by(70) {
        shape(
            &mut p,
            "Arena grid",
            GraphicKind::Rectangle,
            x as f32,
            465.,
            1.,
            94.,
            [23, 37, 53, 255],
        );
    }
    for (y, width) in [(421., 760.), (450., 810.), (495., 900.)] {
        shape(
            &mut p,
            "Floor line",
            GraphicKind::Rectangle,
            480.,
            y,
            width,
            1.,
            [35, 51, 69, 255],
        );
    }
    shape(
        &mut p,
        "Horizon",
        GraphicKind::Rectangle,
        480.,
        419.,
        780.,
        2.,
        [62, 111, 138, 255],
    );
    shape(
        &mut p,
        "Orbital halo",
        GraphicKind::Ring,
        740.,
        190.,
        145.,
        145.,
        [22, 40, 60, 255],
    );
    title(&mut p, "CONTACT / 01", "ASTRAL MOTION STUDY · 60 FPS");
    let mut first = stick_character("Aster".into(), Point::new(365., 325.), [110, 230, 211, 255]);
    let mut second = stick_character("Ember".into(), Point::new(570., 325.), [255, 137, 108, 255]);
    first.thickness = 8.;
    second.thickness = 8.;
    let a = first.id;
    let b = second.id;
    p.characters = vec![first, second];
    let beats = [
        (a, FightAction::Punch, 36),
        (b, FightAction::Dodge, 24),
        (b, FightAction::Counter, 36),
        (a, FightAction::Kick, 42),
        (b, FightAction::Punch, 30),
        (a, FightAction::Knockback, 48),
    ]
    .into_iter()
    .map(|(attacker, action, frames)| FightBeat {
        attacker,
        action,
        frames,
        strength: 1.4,
    })
    .collect();
    let sequence = plan(
        &p,
        StudioOperation::Fight {
            first: a,
            second: b,
            start: 36,
            beats,
            style: MovementStyle::AnimeFight,
            impacts: true,
            overwrite: false,
        },
    )?;
    p = apply(p, sequence.commands)?;
    let sound = "assets/contact-impact.wav";
    service
        .workspace
        .atomic_write(sound, &astral_audio::impact_wav()?)?;
    let asset = Asset {
        id: Id::new_v4(),
        name: "Contact impact".into(),
        path: sound.into(),
        kind: "audio".into(),
        tags: vec!["original".into(), "synthetic".into()],
    };
    for cue in staging::cues(&p)? {
        p.audio.push(AudioClip {
            id: Id::new_v4(),
            name: "Impact".into(),
            asset: asset.id,
            start: cue.frame,
            offset_seconds: 0.,
            duration_seconds: 0.25,
            gain: 0.55,
            fade_in: 0.002,
            fade_out: 0.05,
            looping: false,
        });
    }
    p.assets.push(asset);
    save(&service, p, "contact")?;
    let mut p = Project::new("Soft / 02", 640, 360, 24, 96);
    p.schema = 8;
    p.background = [13, 19, 29, 255];
    shape(
        &mut p,
        "Plate",
        GraphicKind::Ellipse,
        320.,
        292.,
        380.,
        28.,
        [21, 33, 47, 255],
    );
    title(&mut p, "SOFT / 02", "RASTER ARTWORK · ANIMATED PINS");
    let image = image::RgbaImage::from_fn(240, 160, |x, y| {
        let inside = x > 8 && x < 232 && y > 8 && y < 152;
        let ring = ((x as f32 - 120.).hypot(y as f32 - 80.) - 43.).abs() < 7.;
        let stripe = (x + y * 2) % 48 < 7;
        image::Rgba(if !inside {
            [0; 4]
        } else if ring {
            [246, 237, 211, 255]
        } else if stripe {
            [211, 125, 101, 255]
        } else {
            [73 + (y / 4) as u8, 158 + (x / 8) as u8, 148, 255]
        })
    });
    let bitmap = astral::artwork::bitmap("Original studio tile".into(), image)?;
    let pins = [(0., 0.), (1., 0.), (0., 1.), (1., 1.), (0.5, 0.5)]
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| MeshPin {
            id: Id::new_v4(),
            name: format!("Pin {}", index + 1),
            rest: Point::new(x, y),
            keys: [0, 24, 48, 72, 95]
                .into_iter()
                .enumerate()
                .map(|(i, frame)| PinKey {
                    frame,
                    offset: Point::new(
                        if index == 4 {
                            [0., 0.18, 0., -0.18, 0.][i]
                        } else {
                            0.
                        },
                        if index < 2 {
                            [0., -0.12, 0., 0.12, 0.][i]
                        } else {
                            0.
                        },
                    ),
                })
                .collect(),
        })
        .collect();
    p.rasters.push(RasterLayer {
        id: Id::new_v4(),
        name: "Waving tile".into(),
        start: 0,
        end: 95,
        sources: vec![bitmap.id],
        source_fps: 24,
        looping: false,
        size: Point::new(240., 160.),
        keys: [(0, -0.10), (24, 0.12), (48, -0.10), (72, 0.12), (95, -0.10)]
            .into_iter()
            .map(|(frame, rotation)| RasterKey {
                frame,
                position: Point::new(320., 222.),
                scale: Point::new(1., 1.),
                rotation,
                opacity: 1.,
                easing: Easing::Smooth,
            })
            .collect(),
        cels: vec![],
        mesh: Some(DeformMesh {
            columns: 20,
            rows: 14,
            pins,
        }),
        layer: GraphicLayer::Foreground,
        visible: true,
    });
    p.bitmaps.push(bitmap);
    let mut stress = p.clone();
    stress.id = Id::new_v4();
    stress.name = "Raster coverage benchmark".into();
    stress.width = 1920;
    stress.height = 1080;
    stress.duration = 240;
    stress.graphics.clear();
    stress.dialogues.clear();
    stress.rasters[0].end = 239;
    stress.rasters[0].size = Point::new(1800., 960.);
    for key in &mut stress.rasters[0].keys {
        key.position = Point::new(960., 540.);
    }
    // Keep this throughput corpus moving for its entire ten seconds; hold reuse
    // is demonstrated separately by the dialogue study.
    for key in &mut stress.rasters[0].keys {
        key.frame = (key.frame as f64 * 239. / 95.).round() as u32;
    }
    if let Some(mesh) = &mut stress.rasters[0].mesh {
        for pin in &mut mesh.pins {
            for key in &mut pin.keys {
                key.frame = (key.frame as f64 * 239. / 95.).round() as u32;
            }
        }
    }
    save(&service, stress, "raster-stress")?;
    save(&service, p, "soft")?;
    let mut p = Project::new("Voice / 03", 640, 360, 24, 192);
    p.schema = 8;
    p.background = [15, 22, 31, 255];
    title(&mut p, "VOICE / 03", "OFFLINE VOICE · EDITABLE MOUTH CUES");
    let mut character = stick_character(
        "Narrator".into(),
        Point::new(155., 230.),
        [241, 197, 137, 255],
    );
    character.thickness = 7.;
    let character_id = character.id;
    p.characters.push(character);
    let spoken = "Welcome to Astral. Draw your first frame, shape the movement, and make it yours.";
    let bytes = astral::voice::synthesize(spoken, &Default::default())?;
    let seconds = astral_audio::wav_duration(&bytes)?;
    p.duration = ((seconds + 0.5) * 24.).ceil() as u32;
    for d in &mut p.dialogues {
        d.end = p.duration - 1;
    }
    let path = "assets/voice-study.wav";
    service.workspace.atomic_write(path, &bytes)?;
    let asset = Asset {
        id: Id::new_v4(),
        name: "Windows offline voice".into(),
        path: path.into(),
        kind: "audio".into(),
        tags: vec!["synthetic_speech".into()],
    };
    let clip = AudioClip {
        id: Id::new_v4(),
        name: asset.name.clone(),
        asset: asset.id,
        start: 0,
        offset_seconds: 0.,
        duration_seconds: seconds,
        gain: 1.,
        fade_in: 0.01,
        fade_out: 0.02,
        looping: false,
    };
    let clip_id = clip.id;
    p.assets.push(asset);
    p.audio.push(clip);
    p.dialogues.push(Dialogue {
        id: Id::new_v4(),
        name: "Editable transcript".into(),
        text: spoken.into(),
        speaker: "ASTRAL".into(),
        start: 0,
        end: p.duration - 1,
        position: Point::new(284., 130.),
        size: Point::new(304., 180.),
        font_size: 21.,
        color: [25, 35, 48, 255],
        text_color: [231, 236, 244, 255],
        appearance: DialogueAppearance::dark(),
        style: DialogueStyle::Caption,
        character: None,
        typewriter: 0.,
        formatting: Default::default(),
    });
    let id = service.insert(p)?;
    let p = service.snapshot(id)?;
    service.artwork(ArtworkRequest::LipSync {
        project: id,
        expected_revision: p.revision,
        clip: clip_id,
        character: character_id,
        text: spoken.into(),
        threshold: Some(0.025),
    })?;
    let mut voice = service.snapshot(id)?;
    for mouth in &mut voice.mouths {
        mouth.color = [24, 31, 41, 255];
        mouth.size = Point::new(8., 7.);
        mouth.offset = Point::new(0., 6.);
    }
    service.request(Request::ApplyCommands {
        batch: Batch {
            project: id,
            expected_revision: voice.revision,
            commands: voice
                .mouths
                .into_iter()
                .map(|track| Command::PutMouthTrack { track })
                .collect(),
        },
    })?;
    service.request(Request::SaveProject {
        project: id,
        path: "voice.astral".into(),
    })?;
    if std::env::args().any(|v| v == "--render") {
        for name in ["contact", "soft", "voice"] {
            let p = service.workspace.load(format!("{name}.astral"))?;
            let project =
                service.open_local_project(&service.workspace.path(format!("{name}.astral"))?)?;
            service.workspace.atomic_write(
                format!("{name}-still.png"),
                &astral_render::png(&p, if name == "contact" { 113. } else { 24. }, 640, 360)?,
            )?;
            let value = service.request(Request::ExportMedia {
                project,
                spec: astral::export::ExportSpec {
                    path: format!("{name}.gif"),
                    width: Some(640),
                    height: Some(360),
                    fps: Some(24),
                    renderer: astral::export::RenderBackend::Gpu,
                    ..Default::default()
                },
            })?;
            let job = serde_json::from_value(value["job"].clone())?;
            loop {
                let value = service.request(Request::JobStatus { job })?;
                match value["state"].as_str() {
                    Some("completed") => {
                        println!("{name}: {}", value["metrics"]);
                        break;
                    }
                    Some("failed" | "cancelled") => anyhow::bail!("{value}"),
                    _ => std::thread::sleep(std::time::Duration::from_millis(100)),
                }
            }
        }
    }
    println!(
        "Created contact.astral, soft.astral and voice.astral in {}",
        service.workspace.root().display()
    );
    Ok(())
}
