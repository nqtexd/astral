//! Convert the established evaluator's bounded output to standalone native rigs and vector cels.
use super::*;
use std::collections::BTreeMap;
pub fn convert(p: &mut Project, report: &Value) -> Result<Vec<String>> {
    let frames = report["samples"]
        .as_array()
        .context("Missing evaluated source frames")?;
    ensure!(
        !frames.is_empty() && frames.len() <= 12000,
        "Invalid evaluated frame count"
    );
    p.schema = p.schema.max(9);
    p.duration = report["duration"]
        .as_u64()
        .context("Missing evaluated duration")? as u32;
    p.camera.clear();
    let mut warnings: Vec<String> = report["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let mut rigs: BTreeMap<u64, (Character, BTreeMap<u64, usize>)> = BTreeMap::new();
    let layer = DrawingLayer::new("Imported animation · editable points");
    let mut point_count = 0;
    let mut images = BTreeMap::new();
    for (key, source) in report["images"].as_object().into_iter().flatten() {
        let data = source
            .as_str()
            .and_then(|s| s.split_once(","))
            .context("Invalid embedded image")?
            .1;
        ensure!(
            data.len() <= 64 * 1024 * 1024,
            "Source image exceeds byte budget"
        );
        let bytes = STANDARD.decode(data)?;
        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode()?;
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png)?;
        let bitmap = Bitmap {
            id: Id::new_v4(),
            name: "Imported image".into(),
            width: image.width(),
            height: image.height(),
            png: STANDARD.encode(png.into_inner()).into(),
        };
        images.insert(key.clone(), bitmap.id);
        p.bitmaps.push(bitmap);
    }
    for frame in frames {
        let tick = frame["frame"].as_u64().context("Missing source tick")? as u32;
        ensure!(tick < p.duration, "Source tick outside duration");
        for source in frame["rigs"].as_array().into_iter().flatten() {
            let uid = source["uid"]
                .as_u64()
                .context("Invalid source object identity")?;
            let joints = source["joints"]
                .as_array()
                .context("Missing source joints")?;
            let root: Point =
                serde_json::from_value(source.get("root").cloned().unwrap_or_else(|| {
                    joints
                        .iter()
                        .find(|j| j["id"].as_u64() == Some(0))
                        .map(|j| j["position"].clone())
                        .unwrap_or(json!({"x":0,"y":0}))
                }))?;
            if let std::collections::btree_map::Entry::Vacant(entry) = rigs.entry(uid) {
                let mut c = stick_character(
                    source["name"].as_str().unwrap_or("Imported rig").into(),
                    Point::default(),
                    [50, 150, 220, 255],
                );
                c.artwork_only = true;
                c.joints.clear();
                c.keys.clear();
                let mut map = BTreeMap::new();
                let mut pending: Vec<_> = joints.iter().collect();
                while !pending.is_empty() {
                    let before = pending.len();
                    let mut next = vec![];
                    for j in pending {
                        let id = j["id"].as_u64().context("Invalid source joint ID")?;
                        let parent = j["parent"].as_u64();
                        if parent.is_some_and(|id| !map.contains_key(&id)) {
                            next.push(j);
                            continue;
                        }
                        let position: Point =
                            serde_json::from_value::<Point>(j["position"].clone())? - root;
                        let index = c.joints.len();
                        c.joints.push(Joint {
                            name: format!("joint_{id}"),
                            parent: parent.and_then(|id| map.get(&id).copied()),
                            position,
                        });
                        map.insert(id, index);
                    }
                    ensure!(next.len() < before, "Source rig has unresolved parents");
                    pending = next;
                }
                entry.insert((c, map));
            }
            let (c, map) = rigs.get_mut(&uid).unwrap();
            ensure!(
                joints.len() == map.len()
                    && joints
                        .iter()
                        .all(|j| j["id"].as_u64().is_some_and(|id| map.contains_key(&id))),
                "Source rig topology changes require separate native rigs"
            );
            let mut pose = Pose {
                root,
                joints: c.joints.iter().map(|j| j.position).collect(),
            };
            for j in joints {
                pose.joints[map[&j["id"].as_u64().unwrap()]] =
                    serde_json::from_value::<Point>(j["position"].clone())? - root;
            }
            c.keys.push(PoseKey {
                frame: tick,
                pose,
                easing: Easing::Linear,
            });
        }
        let mut strokes = vec![];
        for source in frame["strokes"].as_array().into_iter().flatten() {
            let points: Vec<Point> = serde_json::from_value(source["points"].clone())?;
            if points.len() < 2 {
                continue;
            }
            point_count += points.len();
            ensure!(point_count <= 2_000_000, "Imported vector budget exceeded");
            let skin = source["uid"]
                .as_u64()
                .and_then(|uid| rigs.get(&uid))
                .map(|(c, map)| {
                    let selected: Vec<usize> = if let (Some(a), Some(b)) = (
                        source["from"].as_u64().and_then(|id| map.get(&id)),
                        source["to"].as_u64().and_then(|id| map.get(&id)),
                    ) {
                        vec![*a, *b]
                    } else {
                        (0..c.joints.len().min(64)).collect()
                    };
                    StrokeSkin {
                        character: c.id,
                        rest: selected
                            .iter()
                            .map(|i| {
                                let pose = &c.keys.last().unwrap().pose;
                                pose.root + pose.joints[*i]
                            })
                            .collect(),
                        joints: selected,
                    }
                });
            strokes.push(InkStroke {
                points,
                color: serde_json::from_value(source["color"].clone())?,
                width: source["width"].as_f64().unwrap_or(1.) as f32,
                filled: source["filled"].as_bool().unwrap_or(false),
                bitmap: source["bitmap"]
                    .as_str()
                    .and_then(|key| images.get(key).copied()),
                curved: false,
                style: BrushStyle::Solid,
                pressures: vec![],
                skin,
            });
        }
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: layer.id,
            frame: tick,
            strokes,
        });
        if let Ok(center) = serde_json::from_value(frame["camera"]["center"].clone()) {
            p.camera.push(CameraKey {
                frame: tick,
                center,
                zoom: frame["camera"]["zoom"].as_f64().unwrap_or(1.) as f32,
            });
        }
        if tick == 0
            && let Ok(background) = serde_json::from_value(frame["background"].clone())
        {
            p.background = background;
        }
    }
    p.layers.push(layer);
    p.characters.extend(rigs.into_values().map(|(c, _)| c));
    warnings.push("Source tween/IK/join/movieclip evaluation is sampled at project FPS into editable native cels and rig controls. Native skin bindings follow later pose edits. Source solver rules and subframe curves are retained in the archive, not reconstructed as identical solvers.".into());
    p.validate()?;
    Ok(warnings)
}
pub fn audio(p: &mut Project, report: &Value, workspace: &Workspace) -> Result<Vec<String>> {
    let source = &report["_styk"]["audio"];
    let mut assets = BTreeMap::new();
    let mut warnings = vec![];
    for (key, asset) in source["assets"].as_object().into_iter().flatten() {
        let decoded = (|| -> Result<Vec<u8>> {
            let mut data = vec![];
            for chunk in asset["chunks"].as_array().context("Audio chunks missing")? {
                let bytes = STANDARD.decode(
                    chunk["data_base64"]
                        .as_str()
                        .context("Invalid audio chunk")?,
                )?;
                ensure!(
                    data.len() + bytes.len() <= 64 * 1024 * 1024,
                    "Audio source exceeds 64 MiB"
                );
                data.extend_from_slice(&bytes);
            }
            astral_audio::normalize_wav(&data)
        })();
        match decoded {
            Ok(bytes) => {
                let id = Id::new_v4();
                let path = format!("assets/legacy-audio/{id}.wav");
                workspace.atomic_write(&path, &bytes)?;
                let seconds = astral_audio::wav_duration(&bytes)?;
                p.assets.push(Asset {
                    id,
                    name: asset["name"].as_str().unwrap_or("Imported audio").into(),
                    path,
                    kind: "audio".into(),
                    tags: vec!["legacy".into()],
                });
                assets.insert(key.clone(), (id, seconds));
            }
            Err(e) => warnings.push(format!("Audio {key} remains archived: {e}")),
        }
    }
    let solo = source["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|t| t["solo"].as_bool().unwrap_or(false));
    for track in source["tracks"].as_array().into_iter().flatten() {
        let muted = track["mute"].as_bool().unwrap_or(false)
            || (solo && !track["solo"].as_bool().unwrap_or(false));
        for clip in track["clips"].as_array().into_iter().flatten() {
            let Some((asset, seconds)) = clip["assetId"].as_str().and_then(|id| assets.get(id))
            else {
                continue;
            };
            let start = (clip["start"].as_f64().unwrap_or(0.) * p.fps as f64)
                .round()
                .max(0.) as u32;
            let offset = clip["offset"].as_f64().unwrap_or(0.);
            let duration = clip["duration"].as_f64().unwrap_or(*seconds - offset);
            ensure!(
                duration > 0.
                    && duration <= 600.
                    && offset >= 0.
                    && offset + duration <= seconds + 0.001,
                "Invalid source audio timing"
            );
            p.duration = p
                .duration
                .max(start + (duration * p.fps as f64).ceil() as u32);
            p.audio.push(AudioClip {
                id: Id::new_v4(),
                name: track["name"].as_str().unwrap_or("Imported audio").into(),
                asset: *asset,
                start,
                offset_seconds: offset,
                duration_seconds: duration,
                gain: if muted {
                    0.
                } else {
                    (clip["gain"].as_f64().unwrap_or(1.) * track["gain"].as_f64().unwrap_or(1.))
                        .clamp(0., 16.) as f32
                },
                fade_in: clip["fadeIn"].as_f64().unwrap_or(0.) as f32,
                fade_out: clip["fadeOut"].as_f64().unwrap_or(0.) as f32,
                looping: false,
                speed: 1.,
            });
        }
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrated_rig_moves_its_artwork_and_preserves_frame_edits() {
        let sample = |frame, x| json!({"frame":frame,"camera":{"center":{"x":32,"y":32},"zoom":1},"rigs":[{"uid":7,"name":"Source","joints":[{"id":0,"parent":null,"position":{"x":x,"y":10}},{"id":1,"parent":0,"position":{"x":x+20,"y":10}}]}],"strokes":[{"uid":7,"from":0,"to":1,"points":[{"x":x,"y":10},{"x":x+20,"y":10}],"color":[20,40,60,255],"width":3,"filled":false}]});
        let mut p = Project::new("Import", 64, 64, 24, 10);
        convert(
            &mut p,
            &json!({"duration":2,"samples":[sample(0,10),sample(1,20)],"warnings":[]}),
        )
        .unwrap();
        assert_eq!(p.camera.len(), 2);
        assert_eq!(p.cels.len(), 2);
        assert!(p.characters[0].artwork_only);
        assert_eq!(p.characters[0].keys[0].pose.root, Point::new(10., 10.));
        assert_eq!(p.characters[0].joints[0].position, Point::default());
        let c = p.characters[0].id;
        let stroke = p.cels[0].strokes[0].clone();
        let scene = evaluate(&p, 0.);
        assert!(stroke.world_points(&scene)[1].distance(Point::new(30., 10.)) < 0.001);
        let mut pose = p.characters[0].keys[0].pose.clone();
        pose.joints[1] = Point::new(0., 20.);
        let mut session = Session::new(p).unwrap();
        session
            .apply(Batch {
                project: session.project.id,
                expected_revision: 0,
                commands: vec![astral_core::Command::SetPose {
                    character: c,
                    frame: 0,
                    pose,
                    easing: Easing::Linear,
                }],
            })
            .unwrap();
        assert!(
            stroke.world_points(&evaluate(&session.project, 0.))[1].distance(Point::new(10., 30.))
                < 0.001
        );
        assert!(
            session.project.cels[1].strokes[0].world_points(&evaluate(&session.project, 1.))[1]
                .distance(Point::new(40., 10.))
                < 0.001
        );
        session.undo(session.project.id, 1).unwrap();
        assert!(
            stroke.world_points(&evaluate(&session.project, 0.))[1].distance(Point::new(30., 10.))
                < 0.001
        );
        let encoded = serde_json::to_vec(&session.project).unwrap();
        let restored: Project = serde_json::from_slice(&encoded).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.cels.len(), 2);
    }
}
