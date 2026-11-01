use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    SetDirectorHistory {
        seeds: Vec<u64>,
    },
    /// Replace exactly one drawing frame, preserving the surrounding exposures.
    SetFrameDrawing {
        layer: Id,
        frame: u32,
        strokes: Vec<InkStroke>,
    },
    SetSceneInteractions {
        cues: Vec<crate::interactions::InteractionCue>,
    },
    SetGraphicBindings {
        bindings: Vec<GraphicBinding>,
    },
    SetStoryboard {
        board: crate::storyboard::Storyboard,
    },
    SetContactCues {
        cues: Vec<crate::staging::ContactCue>,
    },
    SetMarkers {
        markers: std::collections::BTreeMap<String, u32>,
    },
    SetCels {
        layer: Id,
        cels: Vec<DrawingCel>,
    },
    SetBoneConstraints {
        character: Id,
        enabled: bool,
    },
    SetPoseKeys {
        character: Id,
        keys: Vec<PoseKey>,
    },
    SetKeyEasing {
        character: Id,
        frame: u32,
        easing: Easing,
    },
    ReplaceEffect {
        effect: Effect,
    },
    ReplaceAudio {
        clip: AudioClip,
    },
    SetFrameRate {
        fps: u32,
    },
    SetLibrary {
        entries: Vec<crate::studio::LibraryEntry>,
    },
    AddLayer {
        layer: DrawingLayer,
    },
    SetLayer {
        layer: Id,
        name: Option<String>,
        visible: Option<bool>,
        locked: Option<bool>,
        opacity: Option<f32>,
    },
    MoveLayer {
        layer: Id,
        index: usize,
    },
    SetCel {
        layer: Id,
        frame: u32,
        strokes: Vec<InkStroke>,
    },
    DeleteCel {
        layer: Id,
        frame: u32,
    },
    MoveCel {
        layer: Id,
        from: u32,
        to: u32,
    },
    ShiftCels {
        layer: Id,
        start: u32,
        delta: i32,
    },
    SetExposure {
        layer: Id,
        frame: u32,
        frames: u32,
    },
    AddGraphic {
        graphic: Graphic,
    },
    ReplaceGraphic {
        graphic: Graphic,
    },
    SetGraphicKey {
        graphic: Id,
        key: GraphicKey,
    },
    AddDialogue {
        dialogue: Dialogue,
    },
    ReplaceDialogue {
        dialogue: Dialogue,
    },
    TrainMotion {
        character: Id,
        name: String,
        smoothing: f64,
    },
    TrainDrawing {
        sources: Vec<Id>,
        name: String,
        smoothing: f64,
    },
    BakeMotion {
        motion: Id,
    },
    BakeDrawing {
        motion: Id,
    },
    AddCharacter {
        name: String,
        position: Point,
        color: Color,
    },
    AddRig {
        character: Character,
    },
    SetPose {
        character: Id,
        frame: u32,
        pose: Pose,
        easing: Easing,
    },
    SetCharacter {
        character: Id,
        name: Option<String>,
        color: Option<Color>,
        visible: Option<bool>,
        locked: Option<bool>,
        secondary_motion: Option<f32>,
    },
    ApplyAction {
        clip: ActionClip,
    },
    ReplaceAction {
        clip: ActionClip,
    },
    AddEffect {
        effect: Effect,
    },
    AddAsset {
        asset: Asset,
    },
    AddAudio {
        clip: AudioClip,
    },
    SetCamera {
        key: CameraKey,
    },
    AddDrawing {
        drawing: Drawing,
    },
    ReplaceDrawing {
        drawing: Drawing,
    },
    SetContact {
        character: Id,
        contact: Contact,
    },
    ClearContacts {
        character: Id,
    },
    SavePose {
        character: Id,
        frame: u32,
        name: String,
    },
    ApplyPose {
        character: Id,
        pose: Id,
        frame: u32,
        blend: f32,
        joints: Vec<String>,
        mirror: bool,
    },
    Retime {
        start: u32,
        end: u32,
        new_end: u32,
    },
    Remove {
        id: Id,
    },
    SetProject {
        name: Option<String>,
        duration: Option<u32>,
        background: Option<Color>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Batch {
    pub project: Id,
    pub expected_revision: u64,
    pub commands: Vec<Command>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Receipt {
    pub project: Id,
    pub revision: u64,
    pub created_ids: Vec<Id>,
    pub warnings: Vec<String>,
}

/// History stores bounded immutable snapshots. Sparse command deltas can replace
/// snapshots without changing the public transaction contract.
pub struct Session {
    pub project: Project,
    undo: VecDeque<Project>,
    redo: Vec<Project>,
    history_bytes: usize,
}
impl Session {
    pub fn new(project: Project) -> Result<Self> {
        project.validate()?;
        Ok(Self {
            project,
            undo: VecDeque::new(),
            redo: vec![],
            history_bytes: 0,
        })
    }
    pub fn apply(&mut self, batch: Batch) -> Result<Receipt> {
        self.check(batch.project, batch.expected_revision)?;
        ensure!(
            !batch.commands.is_empty() && batch.commands.len() <= 10000,
            "Batch must have 1–10000 commands"
        );
        let mut next = self.project.clone();
        let mut created = vec![];
        for command in batch.commands {
            execute(&mut next, command, &mut created)?;
        }
        if !next.graphics.is_empty()
            || !next.dialogues.is_empty()
            || !next.learned_motions.is_empty()
            || !next.drawing_motions.is_empty()
        {
            next.schema = 2;
        }
        if !next.layers.is_empty() || !next.cels.is_empty() {
            next.schema = 3;
        }
        if next
            .dialogues
            .iter()
            .any(|d| d.formatting != TextFormatting::default())
        {
            next.schema = 4;
        }
        if next.characters.iter().any(|c| c.preserve_bones)
            || next.effects.iter().any(|e| {
                matches!(
                    e.kind,
                    EffectKind::Flash
                        | EffectKind::SpeedLines
                        | EffectKind::Dust
                        | EffectKind::Shockwave
                )
            })
        {
            next.schema = 5;
        }
        if !next.markers.is_empty() {
            next.schema = 5;
        }
        if !next.bindings.is_empty() {
            next.schema = 6;
        }
        if next.graphics.iter().any(|g| !g.contours.is_empty()) {
            next.schema = 7;
        }
        next.validate()?;
        next.revision = self
            .project
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Revision overflow"))?;
        let old = std::mem::replace(&mut self.project, next);
        self.push_history(old);
        self.redo.clear();
        Ok(self.receipt(created))
    }
    pub fn undo(&mut self, id: Id, revision: u64) -> Result<Receipt> {
        self.check(id, revision)?;
        let mut previous = self
            .undo
            .pop_back()
            .ok_or_else(|| anyhow::anyhow!("Nothing to undo"))?;
        self.history_bytes = self
            .history_bytes
            .saturating_sub(serde_json::to_vec(&previous)?.len());
        previous.revision = self.project.revision + 1;
        self.redo
            .push(std::mem::replace(&mut self.project, previous));
        Ok(self.receipt(vec![]))
    }
    pub fn redo(&mut self, id: Id, revision: u64) -> Result<Receipt> {
        self.check(id, revision)?;
        let mut next = self
            .redo
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Nothing to redo"))?;
        next.revision = self.project.revision + 1;
        let old = std::mem::replace(&mut self.project, next);
        self.push_history(old);
        Ok(self.receipt(vec![]))
    }
    fn push_history(&mut self, p: Project) {
        self.history_bytes += serde_json::to_vec(&p).map_or(0, |v| v.len());
        self.undo.push_back(p);
        while self.undo.len() > 100 || self.history_bytes > 64 * 1024 * 1024 {
            if let Some(p) = self.undo.pop_front() {
                self.history_bytes = self
                    .history_bytes
                    .saturating_sub(serde_json::to_vec(&p).map_or(0, |v| v.len()));
            } else {
                break;
            }
        }
    }
    fn check(&self, id: Id, revision: u64) -> Result<()> {
        ensure!(id == self.project.id, "Project handle does not match");
        ensure!(
            revision == self.project.revision,
            "Stale revision: expected {}, received {revision}",
            self.project.revision
        );
        Ok(())
    }
    fn receipt(&self, created_ids: Vec<Id>) -> Receipt {
        Receipt {
            project: self.project.id,
            revision: self.project.revision,
            created_ids,
            warnings: vec![],
        }
    }
}

fn editable(p: &mut Project, id: Id) -> Result<&mut Character> {
    let c = p
        .characters
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow::anyhow!("Character {id} not found"))?;
    ensure!(!c.locked, "Character is locked");
    Ok(c)
}
fn set_key(c: &mut Character, key: PoseKey) {
    if let Some(k) = c.keys.iter_mut().find(|k| k.frame == key.frame) {
        *k = key
    } else {
        c.keys.push(key);
        c.keys.sort_by_key(|k| k.frame)
    }
}
fn execute(p: &mut Project, command: Command, created: &mut Vec<Id>) -> Result<()> {
    match command {
        Command::SetDirectorHistory { seeds } => {
            ensure!(seeds.len() <= 16, "Keep at most 16 recent director seeds");
            p.metadata.insert(
                crate::director::HISTORY.into(),
                serde_json::to_value(seeds)?,
            );
        }
        Command::SetSceneInteractions { cues } => {
            crate::interactions::validate(p, &cues)?;
            p.metadata
                .insert(crate::interactions::KEY.into(), serde_json::to_value(cues)?);
        }
        Command::SetGraphicBindings { bindings } => p.bindings = bindings,
        Command::SetStoryboard { board } => {
            crate::storyboard::validate(p, &board)?;
            p.metadata
                .insert(crate::storyboard::KEY.into(), serde_json::to_value(board)?);
        }
        Command::SetContactCues { cues } => {
            crate::staging::validate_cues(p, &cues)?;
            p.metadata
                .insert(crate::staging::KEY.into(), serde_json::to_value(cues)?);
        }
        cel @ (Command::SetFrameDrawing { .. }
        | Command::AddLayer { .. }
        | Command::SetLayer { .. }
        | Command::MoveLayer { .. }
        | Command::SetCel { .. }
        | Command::DeleteCel { .. }
        | Command::MoveCel { .. }
        | Command::ShiftCels { .. }
        | Command::SetExposure { .. }) => crate::cels::execute(p, cel, created)?,
        feature @ (Command::AddGraphic { .. }
        | Command::ReplaceGraphic { .. }
        | Command::SetGraphicKey { .. }
        | Command::AddDialogue { .. }
        | Command::ReplaceDialogue { .. }
        | Command::TrainMotion { .. }
        | Command::TrainDrawing { .. }
        | Command::BakeMotion { .. }
        | Command::BakeDrawing { .. }) => crate::feature_commands::execute(p, feature, created)?,
        Command::AddCharacter {
            name,
            position,
            color,
        } => {
            let c = stick_character(name, position, color);
            created.push(c.id);
            p.characters.push(c);
        }
        Command::AddRig { character } => {
            created.push(character.id);
            p.characters.push(character);
        }
        Command::SetPose {
            character,
            frame,
            pose,
            easing,
        } => {
            let pose = base_pose_for_target(p, character, frame, pose)?;
            let c = editable(p, character)?;
            validate_pose(&pose, c.joints.len())?;
            set_key(
                c,
                PoseKey {
                    frame,
                    pose,
                    easing,
                },
            );
            p.learned_motions.retain(|m| m.character != character);
        }
        Command::SetCharacter {
            character,
            name,
            color,
            visible,
            locked,
            secondary_motion,
        } => {
            let c = p
                .characters
                .iter_mut()
                .find(|c| c.id == character)
                .ok_or_else(|| anyhow::anyhow!("Character missing"))?;
            if c.locked {
                ensure!(locked == Some(false), "Unlock character before editing");
            }
            if let Some(v) = name {
                c.name = v
            }
            if let Some(v) = color {
                c.color = v
            }
            if let Some(v) = visible {
                c.visible = v
            }
            if let Some(v) = locked {
                c.locked = v
            }
            if let Some(v) = secondary_motion {
                c.secondary_motion = v
            }
        }
        Command::ApplyAction { clip } => {
            editable(p, clip.character)?;
            ensure!(
                !p.actions.iter().any(|a| a.character == clip.character
                    && a.start <= clip.end
                    && clip.start <= a.end),
                "Action clips on a character cannot overlap"
            );
            created.push(clip.id);
            p.actions.push(clip);
        }
        Command::ReplaceAction { clip } => {
            editable(p, clip.character)?;
            let index = p
                .actions
                .iter()
                .position(|a| a.id == clip.id)
                .ok_or_else(|| anyhow::anyhow!("Action not found"))?;
            ensure!(
                !p.actions.iter().enumerate().any(|(i, a)| i != index
                    && a.character == clip.character
                    && a.start <= clip.end
                    && clip.start <= a.end),
                "Action clips overlap"
            );
            p.actions[index] = clip;
        }
        Command::AddEffect { effect } => {
            created.push(effect.id);
            p.effects.push(effect);
        }
        Command::AddAsset { asset } => {
            created.push(asset.id);
            p.assets.push(asset);
        }
        Command::AddAudio { clip } => {
            created.push(clip.id);
            p.audio.push(clip);
        }
        Command::SetCamera { key } => {
            if let Some(k) = p.camera.iter_mut().find(|k| k.frame == key.frame) {
                *k = key
            } else {
                p.camera.push(key);
                p.camera.sort_by_key(|k| k.frame)
            }
        }
        Command::AddDrawing { drawing } => {
            created.push(drawing.id);
            p.drawings.push(drawing);
        }
        Command::ReplaceDrawing { drawing } => {
            let id = drawing.id;
            let original = p
                .drawings
                .iter_mut()
                .find(|d| d.id == id)
                .ok_or_else(|| anyhow::anyhow!("Drawing missing"))?;
            *original = drawing;
            p.drawing_motions.retain(|m| !m.sources.contains(&id));
        }
        Command::SetContact { character, contact } => {
            editable(p, character)?.contacts.push(contact);
        }
        Command::ClearContacts { character } => {
            editable(p, character)?.contacts.clear();
        }
        Command::SavePose {
            character,
            frame,
            name,
        } => {
            let c = p
                .character(character)
                .ok_or_else(|| anyhow::anyhow!("Character missing"))?;
            let joint_names = c.joints.iter().map(|j| j.name.clone()).collect();
            let pose = evaluate(p, frame as f64)
                .characters
                .into_iter()
                .find(|c| c.id == character)
                .ok_or_else(|| anyhow::anyhow!("Character is hidden"))?
                .pose;
            let id = Id::new_v4();
            created.push(id);
            p.poses.push(NamedPose {
                id,
                name,
                joint_names,
                pose,
            });
        }
        Command::ApplyPose {
            character,
            pose,
            frame,
            blend,
            joints,
            mirror,
        } => {
            ensure!(
                blend.is_finite() && (0.0..=1.).contains(&blend),
                "Blend must be 0–1"
            );
            let saved = p
                .poses
                .iter()
                .find(|v| v.id == pose)
                .ok_or_else(|| anyhow::anyhow!("Pose asset missing"))?
                .clone();
            let mut target = evaluate(p, frame as f64)
                .characters
                .into_iter()
                .find(|c| c.id == character)
                .ok_or_else(|| anyhow::anyhow!("Character unavailable"))?
                .pose;
            let c = editable(p, character)?;
            for (i, j) in c.joints.iter().enumerate() {
                if !joints.is_empty() && !joints.contains(&j.name) {
                    continue;
                }
                let source = if mirror {
                    if let Some(rest) = j.name.strip_prefix("left_") {
                        format!("right_{rest}")
                    } else if let Some(rest) = j.name.strip_prefix("right_") {
                        format!("left_{rest}")
                    } else {
                        j.name.clone()
                    }
                } else {
                    j.name.clone()
                };
                let index = saved
                    .joint_names
                    .iter()
                    .position(|n| n == &source)
                    .ok_or_else(|| anyhow::anyhow!("Incompatible rig: missing {source}"))?;
                let mut v = saved.pose.joints[index];
                if mirror {
                    v.x = -v.x
                }
                target.joints[i] = target.joints[i].lerp(v, blend);
            }
            let target = base_pose_for_target(p, character, frame, target)?;
            set_key(
                editable(p, character)?,
                PoseKey {
                    frame,
                    pose: target,
                    easing: Easing::Smooth,
                },
            );
            p.learned_motions.retain(|m| m.character != character);
        }
        Command::Retime {
            start,
            end,
            new_end,
        } => {
            ensure!(
                start < end && start < new_end && new_end < p.duration,
                "Invalid retime range"
            );
            let map = |f: u32| -> Result<u32> {
                let result = if f < start {
                    f as i64
                } else if f <= end {
                    start as i64
                        + (((f - start) as f64 * (new_end - start) as f64) / (end - start) as f64)
                            .round() as i64
                } else {
                    f as i64 + new_end as i64 - end as i64
                };
                ensure!(
                    result >= 0 && result < p.duration as i64,
                    "Retiming moves content outside project; extend duration first"
                );
                Ok(result as u32)
            };
            for c in &mut p.characters {
                ensure!(!c.locked, "Unlock all characters before project retiming");
                for k in &mut c.keys {
                    k.frame = map(k.frame)?;
                }
                for w in c.keys.windows(2) {
                    ensure!(w[0].frame < w[1].frame, "Retiming would merge pose keys");
                }
                for v in &mut c.contacts {
                    v.start = map(v.start)?;
                    v.end = map(v.end)?
                }
            }
            for a in &mut p.actions {
                a.start = map(a.start)?;
                a.end = map(a.end)?;
                for m in a.markers.values_mut() {
                    *m = map(*m)?
                }
            }
            for e in &mut p.effects {
                e.start = map(e.start)?;
                e.end = map(e.end)?
            }
            if p.metadata.contains_key(crate::storyboard::KEY) {
                let mut board = crate::storyboard::read(p)?;
                for shot in &mut board.shots {
                    shot.start = map(shot.start)?;
                    shot.end = map(shot.end)?;
                }
                p.metadata
                    .insert(crate::storyboard::KEY.into(), serde_json::to_value(board)?);
            }
            if p.metadata.contains_key(crate::staging::KEY) {
                let mut cues = crate::staging::cues(p)?;
                for cue in &mut cues {
                    cue.frame = map(cue.frame)?;
                }
                p.metadata
                    .insert(crate::staging::KEY.into(), serde_json::to_value(cues)?);
            }
            for marker in p.markers.values_mut() {
                *marker = map(*marker)?;
            }
            if p.metadata.contains_key(crate::interactions::KEY) {
                let mut cues = crate::interactions::read(p)?;
                for cue in &mut cues {
                    cue.start = map(cue.start)?;
                    cue.end = map(cue.end)?;
                }
                p.metadata
                    .insert(crate::interactions::KEY.into(), serde_json::to_value(cues)?);
            }
            for k in &mut p.camera {
                k.frame = map(k.frame)?
            }
            for d in &mut p.drawings {
                d.start = map(d.start)?;
                d.end = map(d.end)?
            }
            ensure!(
                p.layers.iter().all(|l| !l.locked),
                "Unlock drawing layers before project retiming"
            );
            for cel in &mut p.cels {
                cel.frame = map(cel.frame)?;
            }
            for a in &mut p.audio {
                a.start = map(a.start)?;
            } // preserve pitch and source duration
            for g in &mut p.graphics {
                g.start = map(g.start)?;
                g.end = map(g.end)?;
                for k in &mut g.keys {
                    k.frame = map(k.frame)?;
                }
            }
            for d in &mut p.dialogues {
                d.start = map(d.start)?;
                d.end = map(d.end)?;
            }
            // Refit learned motion against retimed authored examples, keeping IDs.
            let models = std::mem::take(&mut p.learned_motions);
            for m in models {
                let mut learned = train_character(p, m.character, m.model.regularization, m.name)?;
                learned.id = m.id;
                p.learned_motions.push(learned);
            }
            let drawings = std::mem::take(&mut p.drawing_motions);
            for m in drawings {
                let mut learned = train_drawing(p, &m.sources, m.model.regularization, m.name)?;
                learned.id = m.id;
                p.drawing_motions.push(learned);
            }
        }
        Command::Remove { id } => {
            if let Some(layer) = p.layers.iter().find(|l| l.id == id) {
                ensure!(!layer.locked, "Drawing layer is locked");
            }
            if let Some(cel) = p.cels.iter().find(|c| c.id == id) {
                ensure!(
                    p.layers.iter().any(|l| l.id == cel.layer && !l.locked),
                    "Drawing layer is locked"
                );
            }
            let count = p.characters.len()
                + p.actions.len()
                + p.effects.len()
                + p.audio.len()
                + p.drawings.len()
                + p.assets.len()
                + p.poses.len()
                + p.graphics.len()
                + p.dialogues.len()
                + p.learned_motions.len()
                + p.drawing_motions.len()
                + p.layers.len()
                + p.cels.len();
            if p.character(id).is_some() {
                editable(p, id)?;
                p.actions.retain(|a| a.character != id);
                p.effects.retain(|e| e.character != Some(id));
                p.dialogues.retain(|d| d.character != Some(id));
                p.learned_motions.retain(|m| m.character != id);
            }
            ensure!(
                !p.audio.iter().any(|a| a.asset == id),
                "Asset is referenced by audio"
            );
            if p.character(id).is_some() {
                let bound: std::collections::BTreeSet<_> = p
                    .bindings
                    .iter()
                    .filter(|b| b.character == id)
                    .map(|b| b.graphic)
                    .collect();
                p.graphics.retain(|g| !bound.contains(&g.id));
                p.bindings.retain(|b| b.character != id);
                let mut board = crate::storyboard::read(p)?;
                for c in &mut board.characters {
                    if c.rig == Some(id) {
                        c.rig = None;
                    }
                }
                if p.metadata.contains_key(crate::storyboard::KEY) {
                    p.metadata
                        .insert(crate::storyboard::KEY.into(), serde_json::to_value(board)?);
                }
                let cues: Vec<_> = crate::staging::cues(p)?
                    .into_iter()
                    .filter(|c| c.attacker != id && c.target != id)
                    .collect();
                if p.metadata.contains_key(crate::staging::KEY) {
                    p.metadata
                        .insert(crate::staging::KEY.into(), serde_json::to_value(cues)?);
                }
            }
            p.bindings.retain(|b| b.graphic != id);
            p.characters.retain(|v| v.id != id);
            p.actions.retain(|v| v.id != id);
            p.effects.retain(|v| v.id != id);
            p.audio.retain(|v| v.id != id);
            p.drawings.retain(|v| v.id != id);
            p.layers.retain(|l| l.id != id);
            p.cels.retain(|c| c.id != id && c.layer != id);
            p.assets.retain(|v| v.id != id);
            p.poses.retain(|v| v.id != id);
            p.graphics.retain(|v| v.id != id);
            p.dialogues.retain(|v| v.id != id);
            p.learned_motions.retain(|v| v.id != id);
            p.drawing_motions
                .retain(|v| v.id != id && !v.sources.contains(&id));
            let after = p.characters.len()
                + p.actions.len()
                + p.effects.len()
                + p.audio.len()
                + p.drawings.len()
                + p.assets.len()
                + p.poses.len()
                + p.graphics.len()
                + p.dialogues.len()
                + p.learned_motions.len()
                + p.drawing_motions.len()
                + p.layers.len()
                + p.cels.len();
            ensure!(after < count, "Entity not found");
        }
        Command::SetCels { layer, cels } => {
            ensure!(
                p.layers.iter().any(|l| l.id == layer && !l.locked),
                "Select an unlocked layer"
            );
            ensure!(
                cels.iter().all(|c| c.layer == layer),
                "Cels must belong to the selected layer"
            );
            p.cels.retain(|c| c.layer != layer);
            p.cels.extend(cels);
            p.cels.sort_by_key(|c| (c.layer, c.frame));
        }
        Command::SetBoneConstraints { character, enabled } => {
            editable(p, character)?.preserve_bones = enabled;
        }
        Command::SetPoseKeys { character, keys } => {
            ensure!(
                !keys.is_empty() && keys.len() <= 10000,
                "Expected 1–10000 pose keys"
            );
            editable(p, character)?.keys = keys;
            p.learned_motions.retain(|m| m.character != character);
        }
        Command::SetKeyEasing {
            character,
            frame,
            easing,
        } => {
            validate_easing(&easing)?;
            let c = editable(p, character)?;
            let key = c
                .keys
                .iter_mut()
                .find(|k| k.frame == frame)
                .ok_or_else(|| anyhow::anyhow!("Pose key not found"))?;
            key.easing = easing;
            p.learned_motions.retain(|m| m.character != character);
        }
        Command::ReplaceEffect { effect } => {
            let old = p
                .effects
                .iter_mut()
                .find(|e| e.id == effect.id)
                .ok_or_else(|| anyhow::anyhow!("Effect not found"))?;
            *old = effect;
        }
        Command::ReplaceAudio { clip } => {
            let old = p
                .audio
                .iter_mut()
                .find(|a| a.id == clip.id)
                .ok_or_else(|| anyhow::anyhow!("Audio clip not found"))?;
            *old = clip;
        }
        Command::SetMarkers { markers } => {
            p.markers = markers;
        }
        Command::SetFrameRate { fps } => {
            ensure!((1..=240).contains(&fps), "FPS must be 1–240");
            p.fps = fps;
        }
        Command::SetLibrary { entries } => {
            crate::studio::set_library(p, entries)?;
        }
        Command::SetProject {
            name,
            duration,
            background,
        } => {
            if let Some(v) = name {
                p.name = v
            }
            if let Some(v) = duration {
                p.duration = v
            }
            if let Some(v) = background {
                p.background = v
            }
        }
    }
    Ok(())
}

/// Pose commands describe the visible target pose. Remove additive action
/// contributions before storing a key so edits on an action don't double it.
fn base_pose_for_target(p: &Project, id: Id, frame: u32, mut target: Pose) -> Result<Pose> {
    let c = p
        .character(id)
        .ok_or_else(|| anyhow::anyhow!("Character missing"))?;
    validate_pose(&target, c.joints.len())?;
    let mut base = p.clone();
    base.actions.retain(|a| a.character != id);
    if let Some(c) = base.characters.iter_mut().find(|c| c.id == id) {
        c.contacts.clear();
        c.secondary_motion = 0.;
        c.visible = true;
    }
    let mut visible = p.clone();
    if let Some(c) = visible.characters.iter_mut().find(|c| c.id == id) {
        c.contacts.clear();
        c.visible = true;
    }
    let base = evaluate(&base, frame as f64)
        .characters
        .into_iter()
        .find(|c| c.id == id)
        .unwrap()
        .pose;
    let actual = evaluate(&visible, frame as f64)
        .characters
        .into_iter()
        .find(|c| c.id == id)
        .unwrap()
        .pose;
    target.root = target.root - (actual.root - base.root);
    for (i, j) in target.joints.iter_mut().enumerate() {
        *j = *j - (actual.joints[i] - base.joints[i]);
    }
    Ok(target)
}

pub fn fight_demo() -> Result<Project> {
    let mut session = Session::new(Project::new("First contact", 1920, 1080, 30, 300))?;
    let mut commands = vec![
        Command::AddCharacter {
            name: "Nova".into(),
            position: Point::new(650., 650.),
            color: [164, 139, 250, 255],
        },
        Command::AddCharacter {
            name: "Orion".into(),
            position: Point::new(1250., 650.),
            color: [74, 222, 198, 255],
        },
    ];
    session.apply(Batch {
        project: session.project.id,
        expected_revision: 0,
        commands: std::mem::take(&mut commands),
    })?;
    let ids: Vec<_> = session.project.characters.iter().map(|c| c.id).collect();
    for (i, id) in ids.into_iter().enumerate() {
        let dir = if i == 0 { 1. } else { -1. };
        for (kind, start, end, distance) in [
            (ActionKind::Run, 0, 89, 210.),
            (ActionKind::Strike, 90, 119, 0.),
            (ActionKind::Recoil, 120, 149, 0.),
            (ActionKind::Jump, 150, 209, 30.),
            (ActionKind::Strike, 210, 239, 0.),
            (ActionKind::Idle, 240, 299, 0.),
        ] {
            commands.push(Command::ApplyAction {
                clip: ActionClip {
                    id: Id::new_v4(),
                    character: id,
                    kind,
                    start,
                    end,
                    intensity: 1.,
                    distance,
                    direction: dir,
                    markers: std::collections::BTreeMap::from([(
                        "accent".into(),
                        (start + end) / 2,
                    )]),
                },
            });
        }
    }
    for frame in [104, 134, 224] {
        commands.push(Command::AddEffect {
            effect: Effect {
                id: Id::new_v4(),
                name: "Impact burst".into(),
                kind: EffectKind::Impact,
                start: frame,
                end: frame + 10,
                position: Point::new(960., 570.),
                character: None,
                joint: None,
                color: [255, 213, 116, 255],
                intensity: 1.5,
                seed: frame as u64,
            },
        });
        commands.push(Command::AddEffect {
            effect: Effect {
                id: Id::new_v4(),
                name: "Camera impulse".into(),
                kind: EffectKind::CameraShake,
                start: frame,
                end: frame + 5,
                position: Point::default(),
                character: None,
                joint: None,
                color: [255; 4],
                intensity: 0.3,
                seed: frame as u64,
            },
        });
    }
    commands.push(Command::SetCamera {
        key: CameraKey {
            frame: 0,
            center: Point::new(960., 540.),
            zoom: 1.,
        },
    });
    commands.push(Command::SetCamera {
        key: CameraKey {
            frame: 104,
            center: Point::new(960., 560.),
            zoom: 1.3,
        },
    });
    commands.push(Command::SetCamera {
        key: CameraKey {
            frame: 299,
            center: Point::new(960., 540.),
            zoom: 1.,
        },
    });
    session.apply(Batch {
        project: session.project.id,
        expected_revision: session.project.revision,
        commands,
    })?;
    Ok(session.project)
}
