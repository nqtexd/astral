use super::*;
const KEY: &str = "astral.studio_library.v1";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    #[default]
    Character,
    Motion,
    Symbol,
    Combo,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LibraryEntry {
    pub id: Id,
    pub name: String,
    pub kind: LibraryKind,
    pub duration: u32,
    pub origin: Point,
    pub figures: Vec<Character>,
    pub layers: Vec<DrawingLayer>,
    pub cels: Vec<DrawingCel>,
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub markers: BTreeMap<String, u32>,
}
pub fn library_entries(p: &Project) -> Result<Vec<LibraryEntry>> {
    p.metadata
        .get(KEY)
        .map(|v| serde_json::from_value(v.clone()).context("Invalid project library"))
        .unwrap_or_else(|| Ok(vec![]))
}
pub(crate) fn set_library(p: &mut Project, entries: Vec<LibraryEntry>) -> Result<()> {
    let old = p
        .metadata
        .insert(KEY.into(), serde_json::to_value(&entries)?);
    if let Err(error) = validate_library(p) {
        if let Some(old) = old {
            p.metadata.insert(KEY.into(), old);
        } else {
            p.metadata.remove(KEY);
        }
        return Err(error);
    }
    Ok(())
}
pub fn validate_library(p: &Project) -> Result<()> {
    let Some(value) = p.metadata.get(KEY) else {
        return Ok(());
    };
    ensure!(
        serde_json::to_vec(value)?.len() <= 4 * 1024 * 1024,
        "Library exceeds its 4 MiB budget"
    );
    let entries = library_entries(p)?;
    ensure!(entries.len() <= 64, "Library supports up to 64 entries");
    let mut ids = BTreeSet::new();
    for e in entries {
        ensure!(
            ids.insert(e.id) && !e.name.trim().is_empty() && e.name.len() <= 256,
            "Invalid library item identity/name"
        );
        ensure!(
            (1..=1201).contains(&e.duration) && e.origin.finite(),
            "Invalid library range"
        );
        ensure!(
            e.figures.len() <= 8 && e.layers.len() <= 8 && e.effects.len() <= 128,
            "Library item is too complex"
        );
        match e.kind {
            LibraryKind::Symbol => ensure!(!e.layers.is_empty(), "A symbol needs a drawing layer"),
            LibraryKind::Combo => ensure!(e.figures.len() >= 2, "A combo needs two characters"),
            _ => ensure!(
                e.figures.len() == 1,
                "Character/motion items need one character"
            ),
        }
        let mut test = Project::new("Library validation", p.width, p.height, p.fps, e.duration);
        test.schema = test.schema.max(5);
        test.characters = e.figures;
        test.layers = e.layers;
        test.cels = e.cels;
        test.effects = e.effects;
        test.markers = e.markers;
        test.validate()?;
    }
    Ok(())
}
pub(super) fn save_library(
    p: &Project,
    name: String,
    kind: LibraryKind,
    characters: &[Id],
    layer: Option<Id>,
    start: u32,
    end: u32,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    ensure!(
        !name.trim().is_empty() && name.len() <= 256,
        "Name the library item"
    );
    let duration = if kind == LibraryKind::Character {
        1
    } else {
        end - start + 1
    };
    let mut e = LibraryEntry {
        id: Id::new_v4(),
        name,
        kind,
        duration,
        origin: Point::default(),
        figures: vec![],
        layers: vec![],
        cels: vec![],
        effects: vec![],
        markers: BTreeMap::new(),
    };
    if kind == LibraryKind::Symbol {
        let id = layer.context("Select a drawing layer")?;
        let source = p
            .layers
            .iter()
            .find(|l| l.id == id)
            .context("Layer not found")?;
        e.layers.push(source.clone());
        e.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: id,
            frame: 0,
            strokes: p.cel_at(id, start).map_or(vec![], |c| c.strokes.clone()),
        });
        e.cels.extend(
            p.cels
                .iter()
                .filter(|c| c.layer == id && c.frame > start && c.frame <= end)
                .map(|c| {
                    let mut c = c.clone();
                    c.id = Id::new_v4();
                    c.frame -= start;
                    c
                }),
        );
        let points: Vec<_> = e
            .cels
            .iter()
            .flat_map(|c| &c.strokes)
            .flat_map(|s| &s.points)
            .collect();
        if !points.is_empty() {
            let (sx, sy) = points.iter().fold((0., 0.), |(x, y), p| (x + p.x, y + p.y));
            e.origin = Point::new(sx / points.len() as f32, sy / points.len() as f32);
        }
    } else {
        ensure!(
            !characters.is_empty() && characters.len() <= 8,
            "Select 1–8 characters"
        );
        ensure!(
            if kind == LibraryKind::Combo {
                characters.len() >= 2
            } else {
                characters.len() == 1
            },
            "Choose one character, or two for a combo"
        );
        let selected: BTreeSet<_> = characters.iter().copied().collect();
        ensure!(
            selected.len() == characters.len(),
            "Select distinct characters"
        );
        e.origin = pose_at(p, characters[0], start)?.root;
        for &id in characters {
            let c = p.character(id).context("Character not found")?;
            let mut saved = c.clone();
            saved.locked = false;
            saved.visible = true;
            saved.contacts.clear();
            saved.keys.clear();
            for frame in start..start + duration {
                let mut pose = pose_at(p, id, frame)?;
                pose.root = pose.root - e.origin;
                saved.keys.push(PoseKey {
                    frame: frame - start,
                    pose,
                    easing: Easing::Linear,
                });
            }
            e.figures.push(saved);
        }
        if kind == LibraryKind::Combo {
            for effect in p.effects.iter().filter(|e| {
                e.character.is_some_and(|id| selected.contains(&id))
                    && e.start <= end
                    && e.end >= start
            }) {
                let mut effect = effect.clone();
                effect.start = effect.start.max(start) - start;
                effect.end = effect.end.min(end) - start;
                e.effects.push(effect);
            }
        }
    }
    e.markers = p
        .markers
        .iter()
        .filter(|(_, f)| **f >= start && **f < start + duration)
        .map(|(name, f)| (name.clone(), f - start))
        .collect();
    let mut entries = library_entries(p)?;
    entries.push(e);
    let mut out = StudioPlan::named("Save reusable animation");
    out.commands.push(Command::SetLibrary { entries });
    if kind == LibraryKind::Combo {
        out.warnings.push("The combo stores editable motion and attached VFX. Audio cues stay separate in the project.".into());
    }
    Ok(out)
}
fn compatible(a: &Character, b: &Character) -> bool {
    a.joints.len() == b.joints.len()
        && b.joints.iter().all(|j| {
            a.joints.iter().any(|k| {
                k.name == j.name
                    && k.parent.map(|i| a.joints[i].name.as_str())
                        == j.parent.map(|i| b.joints[i].name.as_str())
            })
        })
}
fn retarget(source: &Character, target: &Character, pose: &Pose, mirror: bool) -> Result<Pose> {
    ensure!(
        compatible(source, target),
        "The target rig needs the same named controls and parent relationships"
    );
    let mut out = Pose {
        root: pose.root,
        joints: target.joints.iter().map(|j| j.position).collect(),
    };
    for (i, j) in target.joints.iter().enumerate() {
        let name = if mirror {
            if let Some(s) = j.name.strip_prefix("left_") {
                format!("right_{s}")
            } else if let Some(s) = j.name.strip_prefix("right_") {
                format!("left_{s}")
            } else {
                j.name.clone()
            }
        } else {
            j.name.clone()
        };
        let k = source
            .joints
            .iter()
            .position(|s| s.name == name)
            .context("Mirror counterpart missing")?;
        let mut v = pose.joints[k];
        if mirror {
            v.x = -v.x;
        }
        out.joints[i] = v;
    }
    constrain_bones(target, &mut out);
    Ok(out)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn insert_library(
    p: &Project,
    id: Id,
    character: Option<Id>,
    frame: u32,
    duration: u32,
    position: Point,
    mirror: bool,
) -> Result<StudioPlan> {
    ensure!(
        position.finite() && (1..=1201).contains(&duration),
        "Choose a finite position and 1–1201 frames"
    );
    let entries = library_entries(p)?;
    let e = entries
        .iter()
        .find(|e| e.id == id)
        .context("Library item not found")?;
    insert_entry(p, e, character, frame, duration, position, mirror)
}
#[allow(clippy::too_many_arguments)]
fn insert_entry(
    p: &Project,
    e: &LibraryEntry,
    character: Option<Id>,
    frame: u32,
    duration: u32,
    position: Point,
    mirror: bool,
) -> Result<StudioPlan> {
    let duration = if e.kind == LibraryKind::Character {
        1
    } else {
        duration
    };
    let end = frame
        .checked_add(duration - 1)
        .context("Timeline overflow")?;
    let mut out = StudioPlan::named("Insert library item");
    if e.kind == LibraryKind::Symbol {
        for old_layer in &e.layers {
            let mut layer = old_layer.clone();
            layer.id = Id::new_v4();
            layer.name = e.name.clone();
            layer.locked = false;
            let layer_id = layer.id;
            out.commands.push(Command::AddLayer { layer });
            let mut previous: Option<Vec<InkStroke>> = None;
            for offset in 0..duration {
                let old_frame = if duration == 1 {
                    0
                } else {
                    ((e.duration - 1) as f64 * offset as f64 / (duration - 1) as f64).floor() as u32
                };
                let mut strokes = e
                    .cels
                    .iter()
                    .filter(|c| c.layer == old_layer.id && c.frame <= old_frame)
                    .max_by_key(|c| c.frame)
                    .map_or(vec![], |c| c.strokes.clone());
                for s in &mut strokes {
                    for point in &mut s.points {
                        let mut v = *point - e.origin;
                        if mirror {
                            v.x = -v.x;
                        }
                        *point = position + v;
                    }
                }
                if previous.as_ref() != Some(&strokes) {
                    out.commands.push(Command::SetCel {
                        layer: layer_id,
                        frame: frame + offset,
                        strokes: strokes.clone(),
                    });
                    previous = Some(strokes);
                }
            }
            if end + 1 < p.duration {
                out.commands.push(Command::SetCel {
                    layer: layer_id,
                    frame: end + 1,
                    strokes: vec![],
                });
            }
        }
    } else {
        ensure!(
            character.is_none() || e.figures.len() == 1,
            "Insert a combo as new characters"
        );
        let mut remap = BTreeMap::new();
        for source in &e.figures {
            let mut temp = Project::new("Library sampling", p.width, p.height, p.fps, e.duration);
            temp.schema = temp.schema.max(5);
            temp.characters = vec![source.clone()];
            let target = if let Some(id) = character {
                editable_character(p, id)?.clone()
            } else {
                let mut c = source.clone();
                c.id = Id::new_v4();
                c.keys.clear();
                c.contacts.clear();
                c.locked = false;
                c.visible = true;
                c
            };
            let mut keys = vec![];
            for offset in 0..duration {
                let time = if duration == 1 {
                    0.
                } else {
                    (e.duration - 1) as f64 * offset as f64 / (duration - 1) as f64
                };
                let source_pose = evaluate(&temp, time).characters[0].pose.clone();
                let mut pose = retarget(source, &target, &source_pose, mirror)?;
                if mirror {
                    pose.root.x = -pose.root.x;
                }
                pose.root = position + pose.root;
                keys.push(PoseKey {
                    frame: frame + offset,
                    pose,
                    easing: Easing::Linear,
                });
            }
            if character.is_some() {
                out.commands.push(Command::SetBoneConstraints {
                    character: target.id,
                    enabled: true,
                });
                for key in keys {
                    out.commands.push(Command::SetPose {
                        character: target.id,
                        frame: key.frame,
                        pose: key.pose,
                        easing: key.easing,
                    });
                }
            } else {
                let mut target = target;
                target.keys = keys;
                target.preserve_bones = true;
                out.commands.push(Command::AddRig {
                    character: target.clone(),
                });
                remap.insert(source.id, target.id);
            }
        }
        for effect in &e.effects {
            let mut v = effect.clone();
            v.id = Id::new_v4();
            v.character = v.character.and_then(|id| remap.get(&id).copied());
            v.start = frame
                + if e.duration <= 1 {
                    0
                } else {
                    ((duration - 1) as f64 * effect.start as f64 / (e.duration - 1) as f64).round()
                        as u32
                };
            v.end = frame
                + if e.duration <= 1 {
                    0
                } else {
                    ((duration - 1) as f64 * effect.end as f64 / (e.duration - 1) as f64).round()
                        as u32
                };
            if v.character.is_none() {
                v.position = position + (v.position - e.origin);
            }
            out.commands.push(Command::AddEffect { effect: v });
        }
        out.warnings.push("Inserted figures hold their first pose before the insertion frame. Existing target keys outside the range are retained; adjacent interpolation may change.".into());
    }
    if !e.markers.is_empty() {
        let mut markers = p.markers.clone();
        for (i, (name, offset)) in e.markers.iter().enumerate() {
            let scaled = if e.duration <= 1 {
                0
            } else {
                ((duration - 1) as f64 * *offset as f64 / (e.duration - 1) as f64).round() as u32
            };
            markers.insert(
                format!(
                    "{} {} @{}",
                    name.chars().take(80).collect::<String>(),
                    i + 1,
                    frame
                ),
                frame + scaled,
            );
        }
        out.commands.push(Command::SetMarkers { markers });
    }
    extend_to(&mut out, p, end + 1)?;
    Ok(out)
}
pub fn rig_template(
    template: RigTemplate,
    name: String,
    position: Point,
    height: f32,
    color: Color,
) -> Result<Character> {
    ensure!(
        position.finite() && height.is_finite() && (20.0..=8000.).contains(&height),
        "Rig height must be 20–8000 pixels"
    );
    let mut c = stick_character(name, position, color);
    if template == RigTemplate::Quadruped {
        let positions = [
            (0., 0.),
            (100., -5.),
            (140., -50.),
            (80., 45.),
            (75., 95.),
            (120., 45.),
            (125., 95.),
            (-15., 45.),
            (-20., 95.),
            (25., 45.),
            (30., 95.),
        ];
        for (j, (x, y)) in c.joints.iter_mut().zip(positions) {
            j.position = Point::new(x, y);
        }
        c.joints.push(Joint {
            name: "tail_base".into(),
            parent: Some(0),
            position: Point::new(-45., -20.),
        });
        c.joints.push(Joint {
            name: "tail_tip".into(),
            parent: Some(11),
            position: Point::new(-95., -30.),
        });
    }
    if template == RigTemplate::Winged {
        for (name, parent, x, y) in [
            ("left_wing", 1, -40., -85.),
            ("left_wing_tip", 11, -130., -120.),
            ("right_wing", 1, 40., -85.),
            ("right_wing_tip", 13, 130., -120.),
        ] {
            c.joints.push(Joint {
                name: name.into(),
                parent: Some(parent),
                position: Point::new(x, y),
            });
        }
    }
    let low = c
        .joints
        .iter()
        .map(|j| j.position.y)
        .fold(f32::INFINITY, f32::min);
    let high = c
        .joints
        .iter()
        .map(|j| j.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let scale = height / (high - low).max(1.);
    for j in &mut c.joints {
        j.position.x *= scale;
        j.position.y *= scale;
    }
    c.keys[0].pose.joints = c.joints.iter().map(|j| j.position).collect();
    c.thickness = (height / 24.).clamp(1., 40.);
    c.preserve_bones = true;
    Ok(c)
}

pub(super) fn paste_entry(
    p: &Project,
    entry: LibraryEntry,
    character: Option<Id>,
    layer: Option<Id>,
    frame: u32,
    position: Point,
    overwrite: bool,
) -> Result<StudioPlan> {
    let mut test = p.clone();
    test.metadata
        .insert(KEY.into(), serde_json::to_value(vec![entry.clone()])?);
    validate_library(&test)?;
    let end = frame
        .checked_add(entry.duration - 1)
        .context("Paste exceeds timeline")?;
    ensure!(end < 1_000_000, "Paste exceeds timeline limit");
    if entry.kind == LibraryKind::Symbol
        && let Some(layer) = layer
    {
        ensure!(
            p.layers.iter().any(|l| l.id == layer && !l.locked),
            "Select an unlocked drawing layer"
        );
        ensure!(entry.layers.len() == 1, "Paste one symbol layer at a time");
        ensure!(
            overwrite
                || !p
                    .cels
                    .iter()
                    .any(|c| c.layer == layer && c.frame >= frame && c.frame <= end),
            "Paste would replace drawing keys"
        );
        let mut cels: Vec<_> = p
            .cels
            .iter()
            .filter(|c| c.layer == layer && (c.frame < frame || c.frame > end))
            .cloned()
            .collect();
        if end + 1 < p.duration && !cels.iter().any(|c| c.frame == end + 1) {
            cels.push(DrawingCel {
                id: Id::new_v4(),
                layer,
                frame: end + 1,
                strokes: p
                    .cel_at(layer, end + 1)
                    .map_or(vec![], |c| c.strokes.clone()),
            });
        }
        for mut cel in entry.cels {
            cel.id = Id::new_v4();
            cel.layer = layer;
            cel.frame += frame;
            for stroke in &mut cel.strokes {
                for point in &mut stroke.points {
                    *point = *point + (position - entry.origin);
                }
            }
            cels.push(cel);
        }
        cels.sort_by_key(|c| c.frame);
        let mut out = StudioPlan::named("Paste frame range");
        out.commands.push(Command::SetCels { layer, cels });
        extend_to(&mut out, p, end + 1)?;
        Ok(out)
    } else {
        if let Some(id) = character {
            let c = editable_character(p, id)?;
            ensure!(
                overwrite || !c.keys.iter().any(|k| k.frame >= frame && k.frame <= end),
                "Paste would replace pose keys"
            );
        }
        insert_entry(p, &entry, character, frame, entry.duration, position, false)
    }
}
