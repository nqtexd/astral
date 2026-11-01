use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub type Id = Uuid;
pub type Color = [u8; 4];

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn lerp(self, b: Self, t: f32) -> Self {
        Self::new(self.x + (b.x - self.x) * t, self.y + (b.y - self.y) * t)
    }
    pub fn distance(self, b: Self) -> f32 {
        (self.x - b.x).hypot(self.y - b.y)
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}
impl std::ops::Add for Point {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y)
    }
}
impl std::ops::Sub for Point {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub position: Point,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Pose {
    pub root: Point,
    pub joints: Vec<Point>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    /// Continuous, time-aware root and articulated joint interpolation for pose keys.
    Flow,
    Linear,
    Hold,
    #[default]
    Smooth,
    Bezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoseKey {
    pub frame: u32,
    pub pose: Pose,
    #[serde(default)]
    pub easing: Easing,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Contact {
    pub start: u32,
    pub end: u32,
    pub joint: usize,
    pub target: Point,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Character {
    pub id: Id,
    pub name: String,
    pub color: Color,
    pub thickness: f32,
    pub joints: Vec<Joint>,
    pub keys: Vec<PoseKey>,
    #[serde(default)]
    pub contacts: Vec<Contact>,
    #[serde(default)]
    pub secondary_motion: f32,
    #[serde(default)]
    pub preserve_bones: bool,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
}
fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Idle,
    Walk,
    Run,
    Jump,
    Turn,
    Strike,
    Recoil,
    Fall,
}
impl ActionKind {
    pub const ALL: [Self; 8] = [
        Self::Idle,
        Self::Walk,
        Self::Run,
        Self::Jump,
        Self::Turn,
        Self::Strike,
        Self::Recoil,
        Self::Fall,
    ];
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ActionClip {
    pub id: Id,
    pub character: Id,
    pub kind: ActionKind,
    pub start: u32,
    pub end: u32,
    pub intensity: f32,
    pub distance: f32,
    pub direction: f32,
    #[serde(default)]
    pub markers: BTreeMap<String, u32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Impact,
    Trail,
    Particles,
    Smoke,
    Sparks,
    Glow,
    Blur,
    CameraShake,
    Flash,
    SpeedLines,
    Dust,
    Shockwave,
}
impl EffectKind {
    pub const ALL: [Self; 12] = [
        Self::Impact,
        Self::Trail,
        Self::Particles,
        Self::Smoke,
        Self::Sparks,
        Self::Glow,
        Self::Blur,
        Self::CameraShake,
        Self::Flash,
        Self::SpeedLines,
        Self::Dust,
        Self::Shockwave,
    ];
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Effect {
    pub id: Id,
    pub name: String,
    pub kind: EffectKind,
    pub start: u32,
    pub end: u32,
    pub position: Point,
    pub character: Option<Id>,
    pub joint: Option<usize>,
    pub color: Color,
    pub intensity: f32,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CameraKey {
    pub frame: u32,
    pub center: Point,
    pub zoom: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AudioClip {
    pub id: Id,
    pub name: String,
    pub asset: Id,
    pub start: u32,
    pub offset_seconds: f64,
    pub duration_seconds: f64,
    pub gain: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub looping: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Asset {
    pub id: Id,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Drawing {
    pub id: Id,
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub points: Vec<Point>,
    pub color: Color,
    pub width: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct NamedPose {
    pub id: Id,
    pub name: String,
    pub joint_names: Vec<String>,
    pub pose: Pose,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub schema: u32,
    pub id: Id,
    pub revision: u64,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: u32,
    pub background: Color,
    pub characters: Vec<Character>,
    pub actions: Vec<ActionClip>,
    pub effects: Vec<Effect>,
    pub audio: Vec<AudioClip>,
    pub camera: Vec<CameraKey>,
    pub drawings: Vec<Drawing>,
    #[serde(default)]
    pub layers: Vec<crate::DrawingLayer>,
    #[serde(default)]
    pub cels: Vec<crate::DrawingCel>,
    pub assets: Vec<Asset>,
    pub poses: Vec<NamedPose>,
    #[serde(default)]
    pub graphics: Vec<crate::Graphic>,
    #[serde(default)]
    pub bindings: Vec<crate::GraphicBinding>,
    #[serde(default)]
    pub dialogues: Vec<crate::Dialogue>,
    #[serde(default)]
    pub learned_motions: Vec<crate::LearnedMotion>,
    #[serde(default)]
    pub drawing_motions: Vec<crate::DrawingMotion>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub markers: BTreeMap<String, u32>,
}

impl Project {
    pub fn new(name: impl Into<String>, width: u32, height: u32, fps: u32, duration: u32) -> Self {
        Self {
            schema: 2,
            id: Id::new_v4(),
            revision: 0,
            name: name.into(),
            width,
            height,
            fps,
            duration,
            background: [23, 25, 36, 255],
            characters: vec![],
            actions: vec![],
            effects: vec![],
            audio: vec![],
            camera: vec![],
            drawings: vec![],
            layers: vec![],
            cels: vec![],
            assets: vec![],
            poses: vec![],
            graphics: vec![],
            bindings: vec![],
            dialogues: vec![],
            learned_motions: vec![],
            drawing_motions: vec![],
            metadata: BTreeMap::new(),
            markers: BTreeMap::new(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.schema, 1..=7),
            "Unsupported Astral schema {}",
            self.schema
        );
        ensure!(
            (1..=8192).contains(&self.width) && (1..=8192).contains(&self.height),
            "Canvas must be 1–8192 pixels"
        );
        ensure!(
            (1..=240).contains(&self.fps) && (1..=1_000_000).contains(&self.duration),
            "Invalid frame rate or duration"
        );
        ensure!(self.name.len() <= 4096, "Project name too long");
        if !self.graphics.is_empty()
            || !self.dialogues.is_empty()
            || !self.learned_motions.is_empty()
            || !self.drawing_motions.is_empty()
        {
            ensure!(
                self.schema >= 2,
                "Graphics and learned animation require Astral project schema 2"
            );
        }
        if self.characters.iter().any(|c| c.preserve_bones)
            || self.effects.iter().any(|e| {
                matches!(
                    e.kind,
                    EffectKind::Flash
                        | EffectKind::SpeedLines
                        | EffectKind::Dust
                        | EffectKind::Shockwave
                )
            })
        {
            ensure!(
                self.schema >= 5,
                "Rig constraints and new impact presets require schema 5"
            );
        }
        ensure!(
            self.markers.len() <= 512
                && self
                    .markers
                    .iter()
                    .all(|(name, frame)| !name.trim().is_empty()
                        && name.len() <= 128
                        && *frame < self.duration),
            "Invalid action markers"
        );
        if !self.markers.is_empty() {
            ensure!(self.schema >= 5, "Timeline markers require schema 5");
        }
        crate::studio::validate_library(self)?;
        crate::storyboard::validate(self, &crate::storyboard::read(self)?)?;
        crate::staging::validate_cues(self, &crate::staging::cues(self)?)?;
        crate::interactions::validate(self, &crate::interactions::read(self)?)?;
        let mut ids = BTreeSet::new();
        if !self.layers.is_empty() || !self.cels.is_empty() {
            ensure!(
                self.schema >= 3,
                "Drawing cels require Astral project schema 3"
            );
        }
        crate::cels::validate(self)?;
        for id in self
            .characters
            .iter()
            .map(|v| v.id)
            .chain(self.actions.iter().map(|v| v.id))
            .chain(self.effects.iter().map(|v| v.id))
            .chain(self.audio.iter().map(|v| v.id))
            .chain(self.assets.iter().map(|v| v.id))
            .chain(self.drawings.iter().map(|v| v.id))
            .chain(self.layers.iter().map(|v| v.id))
            .chain(self.cels.iter().map(|v| v.id))
            .chain(self.poses.iter().map(|v| v.id))
            .chain(self.graphics.iter().map(|v| v.id))
            .chain(self.dialogues.iter().map(|v| v.id))
            .chain(self.learned_motions.iter().map(|v| v.id))
            .chain(self.drawing_motions.iter().map(|v| v.id))
        {
            ensure!(ids.insert(id), "Duplicate entity ID {id}");
        }
        let range = |a: u32, b: u32| a <= b && b < self.duration;
        for c in &self.characters {
            ensure!(
                !c.joints.is_empty() && c.joints.len() <= 4096,
                "Invalid rig size"
            );
            ensure!(
                c.thickness.is_finite()
                    && (0.1..=200.).contains(&c.thickness)
                    && c.secondary_motion.is_finite()
                    && c.secondary_motion.abs() <= 100.,
                "Invalid character appearance"
            );
            let mut names = BTreeSet::new();
            for (i, j) in c.joints.iter().enumerate() {
                ensure!(
                    j.position.finite() && j.parent.is_none_or(|p| p < i) && names.insert(&j.name),
                    "Rig must have unique names and parents before children"
                );
            }
            ensure!(!c.keys.is_empty(), "Character needs a pose");
            for (i, k) in c.keys.iter().enumerate() {
                ensure!(
                    k.frame < self.duration && (i == 0 || c.keys[i - 1].frame < k.frame),
                    "Pose keys must be ordered and unique"
                );
                validate_pose(&k.pose, c.joints.len())?;
                validate_easing(&k.easing)?;
            }
            for v in &c.contacts {
                ensure!(
                    range(v.start, v.end) && v.joint < c.joints.len() && v.target.finite(),
                    "Invalid contact"
                );
            }
        }
        for a in &self.actions {
            ensure!(
                self.character(a.character).is_some() && range(a.start, a.end) && a.start < a.end,
                "Invalid action range or character"
            );
            ensure!(
                [a.intensity, a.distance, a.direction]
                    .iter()
                    .all(|v| v.is_finite())
                    && a.intensity.abs() <= 10.
                    && a.distance.abs() <= 100000.
                    && a.direction.abs() <= 1.,
                "Invalid action parameters"
            );
            for f in a.markers.values() {
                ensure!(*f >= a.start && *f <= a.end, "Marker outside action");
            }
        }
        for e in &self.effects {
            ensure!(
                range(e.start, e.end)
                    && e.position.finite()
                    && e.intensity.is_finite()
                    && (0.0..=10.).contains(&e.intensity),
                "Invalid effect"
            );
            if let Some(id) = e.character {
                let c = self
                    .character(id)
                    .ok_or_else(|| anyhow::anyhow!("Effect character missing"))?;
                ensure!(
                    e.joint.is_none_or(|j| j < c.joints.len()),
                    "Effect joint missing"
                );
            } else {
                ensure!(e.joint.is_none(), "Joint attachment requires a character");
            }
        }
        for (i, k) in self.camera.iter().enumerate() {
            ensure!(
                k.frame < self.duration
                    && k.center.finite()
                    && k.zoom.is_finite()
                    && (0.05..=20.).contains(&k.zoom)
                    && (i == 0 || self.camera[i - 1].frame < k.frame),
                "Invalid camera key"
            );
        }
        for a in &self.assets {
            let p = std::path::Path::new(&a.path);
            ensure!(
                !a.path.is_empty()
                    && !p.is_absolute()
                    && !a.path.contains(':')
                    && !a.path.contains('\\')
                    && p.components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                "Assets require portable relative paths"
            );
        }
        for a in &self.audio {
            ensure!(
                self.assets.iter().any(|v| v.id == a.asset) && a.start < self.duration,
                "Audio asset missing or start outside project"
            );
            ensure!(
                [
                    a.offset_seconds,
                    a.duration_seconds,
                    a.gain as f64,
                    a.fade_in as f64,
                    a.fade_out as f64
                ]
                .iter()
                .all(|v| v.is_finite())
                    && a.offset_seconds >= 0.
                    && a.duration_seconds > 0.
                    && a.duration_seconds <= 3600.
                    && (0.0..=16.).contains(&a.gain)
                    && a.fade_in >= 0.
                    && a.fade_out >= 0.,
                "Invalid audio clip"
            );
        }
        for d in &self.drawings {
            ensure!(
                range(d.start, d.end)
                    && d.points.len() <= 100000
                    && d.points.iter().all(|p| p.finite())
                    && d.width.is_finite()
                    && (0.1..=200.).contains(&d.width),
                "Invalid drawing"
            );
        }
        for p in &self.poses {
            validate_pose(&p.pose, p.joint_names.len())?;
        }
        crate::validate_graphics(self)?;
        crate::validate_learning(self)?;
        Ok(())
    }
    pub fn character(&self, id: Id) -> Option<&Character> {
        self.characters.iter().find(|c| c.id == id)
    }
}
pub fn validate_pose(p: &Pose, n: usize) -> Result<()> {
    ensure!(
        p.root.finite() && p.joints.len() == n && p.joints.iter().all(|p| p.finite()),
        "Pose must match the rig and contain finite positions"
    );
    Ok(())
}
pub(crate) fn validate_easing(e: &Easing) -> Result<()> {
    if let Easing::Bezier { x1, y1, x2, y2 } = e {
        ensure!(
            [x1, y1, x2, y2].iter().all(|v| v.is_finite())
                && (0.0..=1.).contains(x1)
                && (0.0..=1.).contains(x2),
            "Invalid Bezier easing"
        );
    }
    Ok(())
}

pub fn stick_character(name: String, root: Point, color: Color) -> Character {
    let defs = [
        ("hips", None, 0., 0.),
        ("chest", Some(0), 0., -65.),
        ("head", Some(1), 0., -102.),
        ("left_elbow", Some(1), -38., -39.),
        ("left_hand", Some(3), -53., -2.),
        ("right_elbow", Some(1), 38., -39.),
        ("right_hand", Some(5), 53., -2.),
        ("left_knee", Some(0), -22., 48.),
        ("left_foot", Some(7), -29., 94.),
        ("right_knee", Some(0), 22., 48.),
        ("right_foot", Some(9), 29., 94.),
    ];
    let joints: Vec<_> = defs
        .into_iter()
        .map(|(name, parent, x, y)| Joint {
            name: name.into(),
            parent,
            position: Point::new(x, y),
        })
        .collect();
    let pose = Pose {
        root,
        joints: joints.iter().map(|j| j.position).collect(),
    };
    Character {
        id: Id::new_v4(),
        name,
        color,
        thickness: 9.,
        joints,
        keys: vec![PoseKey {
            frame: 0,
            pose,
            easing: Easing::Smooth,
        }],
        contacts: vec![],
        secondary_motion: 0.,
        preserve_bones: false,
        visible: true,
        locked: false,
    }
}
