use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphicKind {
    Cloud,
    Rectangle,
    Ellipse,
    Star,
    Arrow,
    Ring,
    Burst,
    LowerThird,
}
impl GraphicKind {
    pub const ALL: [Self; 8] = [
        Self::Cloud,
        Self::Rectangle,
        Self::Ellipse,
        Self::Star,
        Self::Arrow,
        Self::Ring,
        Self::Burst,
        Self::LowerThird,
    ];
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphicLayer {
    #[default]
    Background,
    Foreground,
}

/// A graphic's keys become local offsets from a joint or the midpoint of a bone.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GraphicBinding {
    pub graphic: Id,
    pub character: Id,
    pub joint: String,
    /// When present, place at the bone midpoint and align a vertical graphic to it.
    pub to_joint: Option<String>,
}
pub fn bound_graphic_key(
    g: &Graphic,
    binding: &GraphicBinding,
    c: &Character,
    pose: &Pose,
    frame: f64,
) -> GraphicKey {
    let mut key = g.at(frame);
    let from = c
        .joints
        .iter()
        .position(|j| j.name == binding.joint)
        .unwrap();
    let mut point = pose.root + pose.joints[from];
    let mut angle = 0.;
    if let Some(to) = binding
        .to_joint
        .as_ref()
        .and_then(|name| c.joints.iter().position(|j| &j.name == name))
    {
        let end = pose.root + pose.joints[to];
        let d = end - point;
        angle = d.y.atan2(d.x) - std::f32::consts::FRAC_PI_2;
        point = point.lerp(end, 0.5);
    }
    let offset = key.position;
    key.position = point
        + Point::new(
            offset.x * angle.cos() - offset.y * angle.sin(),
            offset.x * angle.sin() + offset.y * angle.cos(),
        );
    key.rotation += angle;
    key
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GraphicKey {
    pub frame: u32,
    pub position: Point,
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
    #[serde(default)]
    pub easing: Easing,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Graphic {
    /// Optional closed vector contours in local coordinates. Opposite winding cuts holes.
    /// Overrides the built-in kind's outline; animated transforms and bindings still apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contours: Vec<Vec<Point>>,
    pub id: Id,
    pub name: String,
    pub kind: GraphicKind,
    pub start: u32,
    pub end: u32,
    pub size: Point,
    pub color: Color,
    #[serde(default)]
    pub layer: GraphicLayer,
    #[serde(default)]
    pub text: String,
    pub keys: Vec<GraphicKey>,
    #[serde(default)]
    pub draw_on: bool,
}
impl Graphic {
    pub fn at(&self, frame: f64) -> GraphicKey {
        let i = self
            .keys
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let a = &self.keys[i];
        let mut out = a.clone();
        if let Some(b) = self.keys.get(i + 1) {
            let t = ease(
                &a.easing,
                ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32,
            );
            out.position = a.position.lerp(b.position, t);
            out.scale = a.scale + (b.scale - a.scale) * t;
            out.rotation = a.rotation + (b.rotation - a.rotation) * t;
            out.opacity = a.opacity + (b.opacity - a.opacity) * t;
        }
        out
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DialogueStyle {
    #[default]
    Speech,
    Thought,
    Caption,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DialogueAppearance {
    pub border_color: Color,
    pub border_width: f32,
    pub corner_radius: f32,
    pub speaker_color: Color,
}
impl Default for DialogueAppearance {
    fn default() -> Self {
        Self {
            border_color: [167, 139, 250, 255],
            border_width: 2.,
            corner_radius: 20.,
            speaker_color: [111, 74, 179, 255],
        }
    }
}
impl DialogueAppearance {
    pub fn dark() -> Self {
        Self {
            border_color: [65, 69, 78, 255],
            border_width: 1.,
            corner_radius: 12.,
            speaker_color: [193, 200, 212, 255],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Dialogue {
    pub id: Id,
    pub name: String,
    pub text: String,
    #[serde(default)]
    pub speaker: String,
    pub start: u32,
    pub end: u32,
    pub position: Point,
    pub size: Point,
    pub font_size: f32,
    pub color: Color,
    pub text_color: Color,
    #[serde(default)]
    pub appearance: DialogueAppearance,
    #[serde(default)]
    pub style: DialogueStyle,
    #[serde(default)]
    pub character: Option<Id>,
    /// Graphemes per second; zero displays all text immediately.
    #[serde(default)]
    pub typewriter: f32,
    #[serde(default)]
    pub formatting: TextFormatting,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextFont {
    #[default]
    Regular,
    Bold,
    Italic,
    BoldItalic,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TextFormatting {
    pub font: TextFont,
    pub align: TextAlign,
    pub line_height: f32,
    pub padding: f32,
}
impl Default for TextFormatting {
    fn default() -> Self {
        Self {
            font: TextFont::Regular,
            align: TextAlign::Left,
            line_height: 1.4,
            padding: 16.,
        }
    }
}

pub fn validate_graphics(p: &Project) -> Result<()> {
    ensure!(
        p.bindings.is_empty() || p.schema >= 6,
        "Live graphic bindings require schema 6"
    );
    let mut bound = std::collections::BTreeSet::new();
    for b in &p.bindings {
        ensure!(
            bound.insert(b.graphic) && p.graphics.iter().any(|g| g.id == b.graphic),
            "Missing or multiply-bound graphic"
        );
        ensure!(
            p.character(b.character)
                .is_some_and(|c| c.joints.iter().any(|j| j.name == b.joint)
                    && b.to_joint
                        .as_ref()
                        .is_none_or(|name| c.joints.iter().any(|j| &j.name == name))),
            "Graphic binding references missing rig/joints"
        );
    }

    for d in &p.dialogues {
        ensure!(
            d.appearance.border_width.is_finite()
                && (0.0..=20.).contains(&d.appearance.border_width)
                && d.appearance.corner_radius.is_finite()
                && (0.0..=200.).contains(&d.appearance.corner_radius),
            "Invalid dialogue border or corner radius"
        );
        if d.formatting != TextFormatting::default() {
            ensure!(p.schema >= 4, "Rich text requires Astral schema 4");
        }
        ensure!(
            d.formatting.line_height.is_finite()
                && (1.0..=2.5).contains(&d.formatting.line_height)
                && d.formatting.padding.is_finite()
                && (0.0..=1000.).contains(&d.formatting.padding)
                && d.formatting.padding * 2. < d.size.x,
            "Invalid text formatting"
        );
    }
    for g in &p.graphics {
        if !g.contours.is_empty() {
            ensure!(p.schema >= 7, "Custom vector contours require schema 7");
            ensure!(
                g.contours.len() <= 512 && g.contours.iter().map(Vec::len).sum::<usize>() <= 16384,
                "Vector artwork exceeds contour limits"
            );
            ensure!(
                g.contours.iter().all(|c| c.len() >= 3
                    && c.iter()
                        .all(|v| v.finite() && v.x.abs() <= 8192. && v.y.abs() <= 8192.)),
                "Vector contours need at least three finite points within canvas limits"
            );
        }

        ensure!(
            g.start <= g.end
                && g.end < p.duration
                && g.size.finite()
                && (1.0..=8192.).contains(&g.size.x)
                && (1.0..=8192.).contains(&g.size.y)
                && g.text.len() <= 4096
                && !g.keys.is_empty()
                && g.keys.len() <= 10000,
            "Invalid graphic dimensions, text or range"
        );
        for (i, k) in g.keys.iter().enumerate() {
            ensure!(
                k.frame >= g.start
                    && k.frame <= g.end
                    && (i == 0 || g.keys[i - 1].frame < k.frame)
                    && k.position.finite()
                    && k.scale.is_finite()
                    && (0.01..=20.).contains(&k.scale)
                    && k.rotation.is_finite()
                    && k.opacity.is_finite()
                    && (0.0..=1.).contains(&k.opacity),
                "Graphic keys must be ordered, finite and inside the clip"
            );
            crate::model::validate_easing(&k.easing)?;
        }
    }
    for d in &p.dialogues {
        ensure!(
            d.appearance.border_width.is_finite()
                && (0.0..=20.).contains(&d.appearance.border_width)
                && d.appearance.corner_radius.is_finite()
                && (0.0..=200.).contains(&d.appearance.corner_radius),
            "Invalid dialogue border or corner radius"
        );
        ensure!(
            d.start <= d.end
                && d.end < p.duration
                && d.position.finite()
                && d.size.finite()
                && ((if d.style == DialogueStyle::Caption
                    && d.color[3] == 0
                    && d.appearance.border_width == 0.
                {
                    1.
                } else {
                    100.
                })..=8192.)
                    .contains(&d.size.x)
                && ((if d.style == DialogueStyle::Caption
                    && d.color[3] == 0
                    && d.appearance.border_width == 0.
                {
                    1.
                } else {
                    60.
                })..=8192.)
                    .contains(&d.size.y)
                && d.font_size.is_finite()
                && (8.0..=200.).contains(&d.font_size)
                && d.typewriter.is_finite()
                && (0.0..=200.).contains(&d.typewriter)
                && d.text.len() <= 4096
                && d.speaker.len() <= 256,
            "Invalid dialogue text, dimensions or timing"
        );
        ensure!(
            d.character.is_none_or(|id| p.character(id).is_some()),
            "Dialogue speaker character is missing"
        );
    }
    Ok(())
}
