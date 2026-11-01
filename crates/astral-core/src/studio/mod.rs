//! Bounded, deterministic editing proposals shared by desktop and agent clients.
use crate::*;
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod choreography;
mod coaching;
mod library;
mod polish;
mod timing;
pub use coaching::*;
pub use library::*;
pub use timing::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MovementStyle {
    #[default]
    Natural,
    Exaggerated,
    AnimeFight,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FightAction {
    #[default]
    Punch,
    Dodge,
    Counter,
    Kick,
    Knockback,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FightBeat {
    pub attacker: Id,
    pub action: FightAction,
    pub frames: u32,
    pub strength: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImpactPack {
    #[default]
    Light,
    Heavy,
    Energy,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RigTemplate {
    #[default]
    Human,
    Quadruped,
    Winged,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "task", rename_all = "snake_case")]
pub enum StudioOperation {
    PolishMotion {
        character: Id,
        start: u32,
        end: u32,
        strength: f32,
    },
    LayoutDialogue {
        dialogue: Id,
    },
    FlowMotion {
        character: Id,
        start: u32,
        end: u32,
    },
    Commands {
        name: String,
        commands: Vec<Command>,
    },
    Paste {
        entry: LibraryEntry,
        character: Option<Id>,
        layer: Option<Id>,
        frame: u32,
        position: Point,
        overwrite: bool,
    },
    Preset {
        character: Id,
        action: ActionKind,
        start: u32,
        end: u32,
        distance: f32,
        intensity: f32,
        direction: f32,
    },
    PoseBatch {
        character: Id,
        start: u32,
        end: u32,
        action: FrameEdit,
        target: u32,
        new_end: u32,
        delta: Point,
        overwrite: bool,
    },
    Marker {
        name: String,
        frame: Option<u32>,
    },
    SmartPose {
        character: Id,
        frame: u32,
        joint: String,
        target: Point,
        style: MovementStyle,
    },
    MotionBrush {
        character: Id,
        joint: String,
        path: Vec<Point>,
        start: u32,
        end: u32,
        keys: u32,
        easing: Easing,
        style: MovementStyle,
    },
    Fight {
        first: Id,
        second: Id,
        start: u32,
        beats: Vec<FightBeat>,
        style: MovementStyle,
        impacts: bool,
        overwrite: bool,
    },
    Impact {
        character: Id,
        joint: String,
        frame: u32,
        action: Option<Id>,
        pack: ImpactPack,
        sound: Option<Id>,
        sound_seconds: f64,
    },
    Analyze {
        start: u32,
        end: u32,
        characters: Vec<Id>,
        tolerance: f32,
    },
    Trajectory {
        character: Id,
        joint: String,
        start: u32,
        end: u32,
        step: u32,
    },
    Tween {
        character: Id,
        start: u32,
        end: u32,
        easing: Easing,
        preserve_bones: bool,
    },
    FootLock {
        character: Id,
        start: u32,
        end: u32,
        joints: Vec<String>,
        automatic: bool,
    },
    Mirror {
        character: Id,
        start: u32,
        end: u32,
    },
    FrameBatch {
        layer: Id,
        start: u32,
        end: u32,
        action: FrameEdit,
        target: u32,
        new_end: u32,
        delta: Point,
        overwrite: bool,
    },
    Timing {
        start: u32,
        end: u32,
        seconds: f32,
    },
    FrameRate {
        fps: u32,
        preserve_seconds: bool,
    },
    SaveLibrary {
        name: String,
        kind: LibraryKind,
        characters: Vec<Id>,
        layer: Option<Id>,
        start: u32,
        end: u32,
    },
    InsertLibrary {
        entry: Id,
        character: Option<Id>,
        frame: u32,
        duration: u32,
        position: Point,
        mirror: bool,
    },
    RemoveLibrary {
        entry: Id,
    },
    Rig {
        template: RigTemplate,
        name: String,
        position: Point,
        height: f32,
        color: Color,
    },
    Camera {
        start: u32,
        end: u32,
        character: Option<Id>,
        from: Point,
        to: Point,
        zoom_start: f32,
        zoom_end: f32,
    },
    EditAudio {
        clip: AudioClip,
    },
    EditEffect {
        effect: Effect,
    },
    Library,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TimedPoint {
    pub frame: u32,
    pub position: Point,
    pub speed: f32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct StudioPlan {
    pub name: String,
    pub commands: Vec<Command>,
    pub warnings: Vec<String>,
    pub markers: BTreeMap<String, u32>,
    pub diagnostics: Vec<MotionIssue>,
    pub path: Vec<TimedPoint>,
    pub library: Vec<LibraryEntry>,
}
impl StudioPlan {
    fn named(name: &str) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
    pub fn validate(&self, p: &Project) -> Result<()> {
        if !self.commands.is_empty() {
            let mut s = Session::new(p.clone())?;
            s.apply(Batch {
                project: p.id,
                expected_revision: p.revision,
                commands: self.commands.clone(),
            })?;
        }
        Ok(())
    }
}
pub fn plan(p: &Project, operation: StudioOperation) -> Result<StudioPlan> {
    let mut out = match operation {
        StudioOperation::PolishMotion {
            character,
            start,
            end,
            strength,
        } => polish::polish(p, character, start, end, strength)?,
        StudioOperation::LayoutDialogue { dialogue } => {
            let (next, warnings) = crate::composition::place_dialogue(p, dialogue)?;
            StudioPlan {
                name: "Place dialogue".into(),
                commands: vec![Command::ReplaceDialogue { dialogue: next }],
                warnings,
                ..Default::default()
            }
        }
        StudioOperation::FlowMotion {
            character,
            start,
            end,
        } => {
            ensure!(start <= end && end < p.duration, "Invalid flow range");
            let c = p.character(character).context("Character missing")?;
            let commands = c
                .keys
                .iter()
                .filter(|k| k.frame >= start && k.frame <= end)
                .map(|k| Command::SetPose {
                    character,
                    frame: k.frame,
                    pose: k.pose.clone(),
                    easing: Easing::Flow,
                })
                .collect();
            StudioPlan {name:"Flow through poses".into(),commands,warnings:vec!["Preserves authored keys and contacts; inspect in-betweens. Acting and weight still require authored anticipation and contact poses.".into()],..Default::default()}
        }
        StudioOperation::Commands { name, commands } => {
            ensure!(name.len() <= 256, "Plan name too long");
            StudioPlan {
                name,
                commands,
                ..Default::default()
            }
        }
        StudioOperation::Paste {
            entry,
            character,
            layer,
            frame,
            position,
            overwrite,
        } => paste_entry(p, entry, character, layer, frame, position, overwrite)?,
        StudioOperation::Preset {
            character,
            action,
            start,
            end,
            distance,
            intensity,
            direction,
        } => {
            frame_range(p, start, end)?;
            editable_character(p, character)?;
            ensure!(start < end, "Action needs at least two frames");
            let clip = ActionClip {
                id: Id::new_v4(),
                character,
                kind: action,
                start,
                end,
                distance,
                intensity,
                direction,
                markers: BTreeMap::from([("contact".into(), (start + end) / 2)]),
            };
            StudioPlan {
                name: format!("{:?} action", action),
                commands: vec![
                    Command::SetBoneConstraints {
                        character,
                        enabled: true,
                    },
                    Command::ApplyAction { clip },
                ],
                ..Default::default()
            }
        }
        StudioOperation::PoseBatch {
            character,
            start,
            end,
            action,
            target,
            new_end,
            delta,
            overwrite,
        } => pose_batch(
            p, character, start, end, action, target, new_end, delta, overwrite,
        )?,
        StudioOperation::Marker { name, frame } => {
            let mut markers = p.markers.clone();
            if let Some(frame) = frame {
                markers.insert(name, frame);
            } else {
                ensure!(markers.remove(&name).is_some(), "Marker not found");
            }
            StudioPlan {
                name: "Action marker".into(),
                commands: vec![Command::SetMarkers { markers }],
                ..Default::default()
            }
        }
        StudioOperation::SmartPose {
            character,
            frame,
            joint,
            target,
            style,
        } => {
            frame_range(p, frame, frame)?;
            let c = editable_character(p, character)?;
            let original = pose_at(p, character, frame)?;
            let (pose, warnings) = pose_with_goal(c, &original, &joint, target, style)?;
            StudioPlan {
                name: "Smart pose".into(),
                commands: vec![
                    Command::SetBoneConstraints {
                        character,
                        enabled: true,
                    },
                    Command::SetPose {
                        character,
                        frame,
                        pose,
                        easing: Easing::Smooth,
                    },
                ],
                warnings,
                ..Default::default()
            }
        }
        StudioOperation::MotionBrush {
            character,
            joint,
            path,
            start,
            end,
            keys,
            easing,
            style,
        } => motion_brush(p, character, &joint, &path, start, end, keys, easing, style)?,
        StudioOperation::Fight {
            first,
            second,
            start,
            beats,
            style,
            impacts,
            overwrite,
        } => choreography::fight(p, first, second, start, &beats, style, impacts, overwrite)?,
        StudioOperation::Impact {
            character,
            joint,
            frame,
            action,
            pack,
            sound,
            sound_seconds,
        } => {
            let frame = if let Some(id) = action {
                let a = p
                    .actions
                    .iter()
                    .find(|a| a.id == id)
                    .context("Action not found")?;
                ensure!(
                    a.character == character,
                    "Action belongs to another character"
                );
                a.markers
                    .get("contact")
                    .or(a.markers.get("accent"))
                    .copied()
                    .unwrap_or((a.start + a.end) / 2)
            } else {
                frame
            };
            let mut out = impact(p, character, &joint, frame, pack)?;
            if let Some(asset) = sound {
                ensure!(
                    p.assets.iter().any(|a| a.id == asset && a.kind == "audio"),
                    "Import a WAV asset first"
                );
                ensure!(
                    sound_seconds.is_finite() && (0.01..=10.).contains(&sound_seconds),
                    "Sound cue must be 0.01–10 seconds"
                );
                out.commands.push(Command::AddAudio {
                    clip: AudioClip {
                        id: Id::new_v4(),
                        name: "Impact cue".into(),
                        asset,
                        start: frame,
                        offset_seconds: 0.,
                        duration_seconds: sound_seconds,
                        gain: 0.8,
                        fade_in: 0.002,
                        fade_out: 0.03,
                        looping: false,
                    },
                });
                extend_to(
                    &mut out,
                    p,
                    frame.saturating_add((sound_seconds * p.fps as f64).ceil() as u32),
                )?;
            }
            out
        }
        StudioOperation::Analyze {
            start,
            end,
            characters,
            tolerance,
        } => {
            let mut out = StudioPlan::named("Motion review");
            out.diagnostics = analyze(p, start, end, &characters, tolerance)?;
            out.warnings.push("Heuristic coaching flags possible issues, including intentional stylization. Review every proposed correction.".into());
            out
        }
        StudioOperation::Trajectory {
            character,
            joint,
            start,
            end,
            step,
        } => {
            let mut out = StudioPlan::named("Ghost path");
            out.path = trajectory(p, character, &joint, start, end, step)?;
            out
        }
        StudioOperation::Tween {
            character,
            start,
            end,
            easing,
            preserve_bones,
        } => {
            frame_range(p, start, end)?;
            validate_curve(&easing)?;
            let c = editable_character(p, character)?;
            let mut out = StudioPlan::named("Motion curve");
            out.commands = c
                .keys
                .iter()
                .filter(|k| k.frame >= start && k.frame <= end)
                .map(|k| Command::SetKeyEasing {
                    character,
                    frame: k.frame,
                    easing: easing.clone(),
                })
                .collect();
            ensure!(!out.commands.is_empty(), "No pose keys in the chosen range");
            out.commands.push(Command::SetBoneConstraints {
                character,
                enabled: preserve_bones,
            });
            out
        }
        StudioOperation::FootLock {
            character,
            start,
            end,
            joints,
            automatic,
        } => foot_lock(p, character, start, end, &joints, automatic)?,
        StudioOperation::Mirror {
            character,
            start,
            end,
        } => {
            frame_range(p, start, end)?;
            let c = editable_character(p, character)?;
            let mut out = StudioPlan::named("Mirror poses");
            for key in c.keys.iter().filter(|k| k.frame >= start && k.frame <= end) {
                let pose = mirror_pose(c, &pose_at(p, character, key.frame)?)?;
                out.commands.push(Command::SetPose {
                    character,
                    frame: key.frame,
                    pose,
                    easing: key.easing.clone(),
                });
            }
            ensure!(
                !out.commands.is_empty(),
                "No authored poses in the chosen range"
            );
            out
        }
        StudioOperation::FrameBatch {
            layer,
            start,
            end,
            action,
            target,
            new_end,
            delta,
            overwrite,
        } => frame_batch(
            p, layer, start, end, action, target, new_end, delta, overwrite,
        )?,
        StudioOperation::Timing {
            start,
            end,
            seconds,
        } => retime_seconds(p, start, end, seconds)?,
        StudioOperation::FrameRate {
            fps,
            preserve_seconds,
        } => frame_rate(p, fps, preserve_seconds)?,
        StudioOperation::SaveLibrary {
            name,
            kind,
            characters,
            layer,
            start,
            end,
        } => save_library(p, name, kind, &characters, layer, start, end)?,
        StudioOperation::InsertLibrary {
            entry,
            character,
            frame,
            duration,
            position,
            mirror,
        } => insert_library(p, entry, character, frame, duration, position, mirror)?,
        StudioOperation::RemoveLibrary { entry } => {
            let mut entries = library_entries(p)?;
            let old = entries.len();
            entries.retain(|e| e.id != entry);
            ensure!(entries.len() < old, "Library item not found");
            StudioPlan {
                name: "Remove library item".into(),
                commands: vec![Command::SetLibrary { entries }],
                ..Default::default()
            }
        }
        StudioOperation::Rig {
            template,
            name,
            position,
            height,
            color,
        } => {
            let c = rig_template(template, name, position, height, color)?;
            StudioPlan {
                name: "Rig template".into(),
                commands: vec![Command::AddRig { character: c }],
                ..Default::default()
            }
        }
        StudioOperation::Camera {
            start,
            end,
            character,
            from,
            to,
            zoom_start,
            zoom_end,
        } => camera(p, start, end, character, from, to, zoom_start, zoom_end)?,
        StudioOperation::EditAudio { clip } => {
            let end = clip
                .start
                .checked_add((clip.duration_seconds * p.fps as f64).ceil() as u32)
                .context("Audio timing overflow")?;
            let mut out = StudioPlan {
                name: "Audio edit".into(),
                commands: vec![Command::ReplaceAudio { clip }],
                ..Default::default()
            };
            extend_to(&mut out, p, end)?;
            out
        }
        StudioOperation::EditEffect { effect } => StudioPlan {
            name: "Effect edit".into(),
            commands: vec![Command::ReplaceEffect { effect }],
            ..Default::default()
        },
        StudioOperation::Library => StudioPlan {
            name: "Project library".into(),
            library: library_entries(p)?,
            ..Default::default()
        },
    };
    if out.commands.iter().any(|c|matches!(c,Command::SetPose{character,..}|Command::SetPoseKeys{character,..}|Command::SetKeyEasing{character,..} if p.learned_motions.iter().any(|m|m.character==*character))){out.warnings.push("Editing these source poses replaces the character's learned interpolation. Authored poses remain editable.".into());}
    out.validate(p)?;
    Ok(out)
}

pub(super) fn frame_range(p: &Project, start: u32, end: u32) -> Result<()> {
    ensure!(
        start <= end && end < p.duration,
        "Choose a range inside the timeline"
    );
    ensure!(end - start <= 1200, "Limit this operation to 1201 frames");
    Ok(())
}
pub(super) fn editable_character(p: &Project, id: Id) -> Result<&Character> {
    let c = p.character(id).context("Character not found")?;
    ensure!(!c.locked, "Unlock the character first");
    Ok(c)
}
pub fn pose_at(p: &Project, id: Id, frame: u32) -> Result<Pose> {
    evaluate(p, frame as f64)
        .characters
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.pose)
        .context("Character is hidden")
}
pub(super) fn validate_curve(e: &Easing) -> Result<()> {
    crate::model::validate_easing(e)
}
pub(super) fn extend_to(out: &mut StudioPlan, p: &Project, duration: u32) -> Result<()> {
    ensure!(
        duration <= 1_000_000,
        "Operation exceeds the timeline limit"
    );
    if duration > p.duration {
        out.commands.insert(
            0,
            Command::SetProject {
                name: None,
                duration: Some(duration),
                background: None,
            },
        );
    }
    Ok(())
}
pub fn constrain_bones(c: &Character, pose: &mut Pose) {
    for (i, j) in c.joints.iter().enumerate() {
        if let Some(parent) = j.parent {
            let rest = j.position - c.joints[parent].position;
            let length = rest.distance(Point::default());
            let v = pose.joints[i] - pose.joints[parent];
            let d = v.distance(Point::default());
            let v = if d > 0.0001 {
                Point::new(v.x / d * length, v.y / d * length)
            } else {
                rest
            };
            pose.joints[i] = pose.joints[parent] + v;
        }
    }
}
fn descendants(c: &Character, node: usize) -> Vec<usize> {
    let mut set = BTreeSet::from([node]);
    for (i, j) in c.joints.iter().enumerate() {
        if j.parent.is_some_and(|p| set.contains(&p)) {
            set.insert(i);
        }
    }
    set.into_iter().collect()
}
pub fn pose_with_goal(
    c: &Character,
    original: &Pose,
    joint: &str,
    target: Point,
    style: MovementStyle,
) -> Result<(Pose, Vec<String>)> {
    ensure!(target.finite(), "Target must be finite");
    crate::model::validate_pose(original, c.joints.len())?;
    let mut pose = original.clone();
    let mut warnings = vec![];
    if joint == "root" {
        pose.root = target;
        return Ok((pose, warnings));
    }
    let end = c
        .joints
        .iter()
        .position(|j| j.name == joint)
        .context("Named joint not found")?;
    let Some(mid) = c.joints[end].parent else {
        pose.root = pose.root + (target - (pose.root + pose.joints[end]));
        return Ok((pose, warnings));
    };
    if c.joints[mid].parent.is_none() {
        let origin = pose.joints[mid];
        let v = target - pose.root - origin;
        let d = v.distance(Point::default());
        let rest = c.joints[end].position.distance(c.joints[mid].position);
        ensure!(d > 0.0001, "Move the control away from its parent");
        let next = origin + Point::new(v.x / d * rest, v.y / d * rest);
        let delta = next - pose.joints[end];
        for i in descendants(c, end) {
            pose.joints[i] = pose.joints[i] + delta;
        }
        return Ok((pose, warnings));
    }

    let root = c.joints[mid]
        .parent
        .context("Choose a hand, foot or two-bone end control")?;
    let goal = target - pose.root;
    let a = c.joints[root].position.distance(c.joints[mid].position);
    let b = c.joints[mid].position.distance(c.joints[end].position);
    ensure!(a > 0.01 && b > 0.01, "Rig has a zero-length limb");
    if let Some(parent) = c.joints[root].parent {
        let origin = pose.joints[parent];
        let arm = pose.joints[root] - origin;
        let desired = goal - origin;
        let angle = (desired.y.atan2(desired.x) - arm.y.atan2(arm.x) + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let lean = match style {
            MovementStyle::Natural => 0.06,
            MovementStyle::Exaggerated => 0.16,
            MovementStyle::AnimeFight => 0.24,
        };
        let rotate = (angle * 0.16).clamp(-lean, lean);
        let next = origin
            + Point::new(
                arm.x * rotate.cos() - arm.y * rotate.sin(),
                arm.x * rotate.sin() + arm.y * rotate.cos(),
            );
        let delta = next - pose.joints[root];
        for i in descendants(c, root) {
            pose.joints[i] = pose.joints[i] + delta;
        }
    }
    let origin = pose.joints[root];
    let v = goal - origin;
    let old = pose.joints[mid] - origin;
    let bend = if v.x * old.y - v.y * old.x >= 0. {
        1.
    } else {
        -1.
    };
    let (m, e) = solve_two_bone(origin, goal, a, b, bend);
    let old_mid = pose.joints[mid];
    let old_end = pose.joints[end];
    let end_desc: BTreeSet<_> = descendants(c, end).into_iter().collect();
    for i in descendants(c, mid) {
        if i != mid && !end_desc.contains(&i) {
            pose.joints[i] = pose.joints[i] + (m - old_mid);
        }
    }
    for &i in &end_desc {
        if i != end {
            pose.joints[i] = pose.joints[i] + (e - old_end);
        }
    }
    pose.joints[mid] = m;
    pose.joints[end] = e;
    if e.distance(goal) > 1. {
        warnings.push(
            "Target lies outside the limb's reachable arc; the suggestion preserves limb lengths."
                .into(),
        );
    }
    Ok((pose, warnings))
}
pub fn mirror_pose(c: &Character, source: &Pose) -> Result<Pose> {
    let mut pose = source.clone();
    for (i, j) in c.joints.iter().enumerate() {
        let name = if let Some(s) = j.name.strip_prefix("left_") {
            format!("right_{s}")
        } else if let Some(s) = j.name.strip_prefix("right_") {
            format!("left_{s}")
        } else {
            j.name.clone()
        };
        let other = c
            .joints
            .iter()
            .position(|v| v.name == name)
            .context("Rig lacks a mirror counterpart")?;
        pose.joints[i] = Point::new(-source.joints[other].x, source.joints[other].y);
    }
    constrain_bones(c, &mut pose);
    Ok(pose)
}
fn sample_line(points: &[Point], fraction: f32) -> Point {
    let lengths: Vec<_> = points.windows(2).map(|p| p[0].distance(p[1])).collect();
    let total: f32 = lengths.iter().sum();
    let target = fraction.clamp(0., 1.) * total;
    let mut sum = 0.;
    for (i, length) in lengths.iter().enumerate() {
        if sum + length >= target && *length > 0.00001 {
            return points[i].lerp(points[i + 1], (target - sum) / length);
        }
        sum += length;
    }
    *points.last().unwrap()
}
#[allow(clippy::too_many_arguments)]
fn motion_brush(
    p: &Project,
    character: Id,
    joint: &str,
    path: &[Point],
    start: u32,
    end: u32,
    keys: u32,
    easing: Easing,
    style: MovementStyle,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    ensure!(start < end, "Motion needs at least two frames");
    ensure!(
        (2..=512).contains(&path.len()) && path.iter().all(|p| p.finite()),
        "Draw a path with 2–512 finite points"
    );
    validate_curve(&easing)?;
    let c = editable_character(p, character)?;
    ensure!((2..=1201).contains(&keys), "Choose 2–1201 key poses");
    let keys = keys.min(end - start + 1);
    let mut out = StudioPlan::named("Motion brush");
    out.commands.push(Command::SetBoneConstraints {
        character,
        enabled: true,
    });
    for i in 0..keys {
        let f = start + ((end - start) as f64 * i as f64 / (keys - 1) as f64).round() as u32;
        let t = (f - start) as f32 / (end - start) as f32;
        let goal = sample_line(path, ease(&easing, t));
        let (pose, warnings) = pose_with_goal(c, &pose_at(p, character, f)?, joint, goal, style)?;
        out.warnings.extend(warnings);
        out.path.push(TimedPoint {
            frame: f,
            position: goal,
            speed: 0.,
        });
        out.commands.push(Command::SetPose {
            character,
            frame: f,
            pose,
            easing: Easing::Linear,
        });
    }
    out.warnings.sort();
    out.warnings.dedup();
    out.warnings.push("The selected range gains editable pose keys; interpolation next to its boundaries may change.".into());
    Ok(out)
}
pub fn trajectory(
    p: &Project,
    character: Id,
    joint: &str,
    start: u32,
    end: u32,
    step: u32,
) -> Result<Vec<TimedPoint>> {
    frame_range(p, start, end)?;
    ensure!(step > 0, "Step must be positive");
    let c = p.character(character).context("Character not found")?;
    let joint = if joint == "root" {
        None
    } else {
        Some(
            c.joints
                .iter()
                .position(|j| j.name == joint)
                .context("Joint not found")?,
        )
    };
    let step = step.max((end - start) / 240 + 1);
    let mut out = vec![];
    for frame in (start..=end)
        .step_by(step as usize)
        .chain(std::iter::once(end))
    {
        if out.last().is_some_and(|v: &TimedPoint| v.frame == frame) {
            continue;
        }
        let pose = pose_at(p, character, frame)?;
        let point = pose.root + joint.map_or(Point::default(), |j| pose.joints[j]);
        let speed = out.last().map_or(0., |v: &TimedPoint| {
            v.position.distance(point) * p.fps as f32 / (frame - v.frame) as f32
        });
        out.push(TimedPoint {
            frame,
            position: point,
            speed,
        });
    }
    Ok(out)
}
pub(super) fn impact(
    p: &Project,
    character: Id,
    joint: &str,
    frame: u32,
    pack: ImpactPack,
) -> Result<StudioPlan> {
    frame_range(p, frame, frame)?;
    let c = p.character(character).context("Character not found")?;
    let joint = c
        .joints
        .iter()
        .position(|j| j.name == joint)
        .context("Contact joint not found")?;
    let mut out = StudioPlan::named("Impact accents");
    let kinds: &[EffectKind] = match pack {
        ImpactPack::Light => &[
            EffectKind::Impact,
            EffectKind::Flash,
            EffectKind::SpeedLines,
        ],
        ImpactPack::Heavy => &[
            EffectKind::Impact,
            EffectKind::Flash,
            EffectKind::Dust,
            EffectKind::Shockwave,
            EffectKind::CameraShake,
        ],
        ImpactPack::Energy => &[
            EffectKind::Flash,
            EffectKind::Shockwave,
            EffectKind::Sparks,
            EffectKind::Trail,
            EffectKind::CameraShake,
        ],
    };
    for (i, &kind) in kinds.iter().enumerate() {
        out.commands.push(Command::AddEffect {
            effect: Effect {
                id: Id::new_v4(),
                name: format!("{kind:?} accent"),
                kind,
                start: frame,
                end: (frame + if kind == EffectKind::Flash { 3 } else { 12 }).min(p.duration - 1),
                position: Point::default(),
                character: Some(character),
                joint: Some(joint),
                color: if kind == EffectKind::Dust {
                    [153, 135, 114, 180]
                } else {
                    [255, 223, 155, 235]
                },
                intensity: if kind == EffectKind::Flash { 0.35 } else { 1. },
                seed: frame as u64 * 127 + i as u64,
            },
        });
    }
    out.markers.insert("contact".into(), frame);
    Ok(out)
}
#[allow(clippy::too_many_arguments)]
fn camera(
    p: &Project,
    start: u32,
    end: u32,
    character: Option<Id>,
    from: Point,
    to: Point,
    zoom_start: f32,
    zoom_end: f32,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    ensure!(from.finite() && to.finite(), "Camera points must be finite");
    let mut out = StudioPlan::named("Camera director");
    let steps = ((end - start) / 3).clamp(1, 240);
    for i in 0..=steps {
        let frame = start + ((end - start) as f64 * i as f64 / steps as f64).round() as u32;
        let t = if end == start {
            0.
        } else {
            (frame - start) as f32 / (end - start) as f32
        };
        let t = ease(&Easing::Smooth, t);
        let center = if let Some(id) = character {
            pose_at(p, id, frame)?.root + from.lerp(to, t)
        } else {
            from.lerp(to, t)
        };
        out.commands.push(Command::SetCamera {
            key: CameraKey {
                frame,
                center,
                zoom: zoom_start + (zoom_end - zoom_start) * t,
            },
        });
    }
    Ok(out)
}
