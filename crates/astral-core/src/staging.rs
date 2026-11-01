//! Contact-aware review, shared by the editor and MCP. Diagnostics never mutate a scene.
use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const KEY: &str = "astral.contact_cues.v1";
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ContactCue {
    pub name: String,
    pub frame: u32,
    pub attacker: Id,
    pub target: Id,
    pub effector: String,
    pub target_joint: String,
    pub tolerance: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContactResult {
    pub cue: ContactCue,
    pub distance: f32,
    pub passed: bool,
    pub point: Point,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpacingIssue {
    pub first: Id,
    pub second: Id,
    pub frame: u32,
    pub distance: f32,
    pub minimum: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub revision: u64,
    pub sampled_frames: usize,
    pub contacts: Vec<ContactResult>,
    pub overlaps: Vec<SpacingIssue>,
    pub continuity: Vec<crate::storyboard::ContinuityIssue>,
    pub interactions: Vec<crate::interactions::InteractionResult>,
    pub layout: Vec<crate::composition::LayoutIssue>,
    pub note: String,
}
pub fn cues(p: &Project) -> Result<Vec<ContactCue>> {
    p.metadata
        .get(KEY)
        .map(|v| serde_json::from_value(v.clone()).map_err(Into::into))
        .unwrap_or_else(|| Ok(vec![]))
}
pub fn validate_cues(p: &Project, cues: &[ContactCue]) -> Result<()> {
    ensure!(cues.len() <= 2048, "Too many contact cues");
    for cue in cues {
        ensure!(
            cue.frame < p.duration
                && cue.attacker != cue.target
                && cue.tolerance.is_finite()
                && (0.1..=100.).contains(&cue.tolerance)
                && cue.name.len() <= 256,
            "Invalid contact cue"
        );
        for (id, joint) in [
            (cue.attacker, &cue.effector),
            (cue.target, &cue.target_joint),
        ] {
            ensure!(
                p.character(id)
                    .is_some_and(|c| c.joints.iter().any(|j| &j.name == joint)),
                "Contact cue references a missing rig or joint"
            );
        }
    }
    Ok(())
}
pub fn world_joint(p: &Pose, c: &Character, name: &str) -> Option<Point> {
    c.joints
        .iter()
        .position(|j| j.name == name)
        .map(|i| p.root + p.joints[i])
}
/// Solve a limb without moving its torso. The caller must block the root within reach.
pub fn aim_limb(c: &Character, p: &mut Pose, name: &str, target: Point, bend: f32) -> Result<f32> {
    let end = c
        .joints
        .iter()
        .position(|j| j.name == name)
        .ok_or_else(|| anyhow::anyhow!("Missing effector"))?;
    let mid = c.joints[end]
        .parent
        .ok_or_else(|| anyhow::anyhow!("Missing limb parent"))?;
    let anchor = c.joints[mid]
        .parent
        .ok_or_else(|| anyhow::anyhow!("Missing limb anchor"))?;
    let l1 = c.joints[anchor].position.distance(c.joints[mid].position);
    let l2 = c.joints[mid].position.distance(c.joints[end].position);
    let (m, e) = solve_two_bone(p.joints[anchor], target - p.root, l1, l2, bend);
    p.joints[mid] = m;
    p.joints[end] = e;
    Ok((p.root + e).distance(target))
}
pub fn review(p: &Project, start: u32, end: u32, step: u32) -> Result<Review> {
    ensure!(
        start <= end && end < p.duration && step > 0,
        "Choose a valid scene review range and sample step"
    );
    ensure!(
        (end - start) / step <= 4096 && p.characters.len() <= 64,
        "Review at most 4097 samples and 64 rigs"
    );
    let relevant: Vec<_> = cues(p)?
        .into_iter()
        .filter(|c| c.frame >= start && c.frame <= end)
        .collect();
    let mut frames: BTreeSet<_> = (start..=end).step_by(step as usize).collect();
    frames.insert(end);
    frames.extend(relevant.iter().map(|c| c.frame));
    let mut contacts = vec![];
    let mut overlaps = BTreeMap::new();
    for frame in &frames {
        let scene = evaluate(p, *frame as f64);
        for cue in relevant.iter().filter(|c| c.frame == *frame) {
            let a = scene.characters.iter().find(|c| c.id == cue.attacker);
            let b = scene.characters.iter().find(|c| c.id == cue.target);
            if let (Some(a), Some(b)) = (a, b) {
                let point =
                    world_joint(&b.pose, p.character(b.id).unwrap(), &cue.target_joint).unwrap();
                let hand = world_joint(&a.pose, p.character(a.id).unwrap(), &cue.effector).unwrap();
                let distance = hand.distance(point);
                contacts.push(ContactResult {
                    cue: cue.clone(),
                    distance,
                    passed: distance <= cue.tolerance,
                    point,
                });
            } else {
                anyhow::bail!("Contact actor hidden at frame {}", frame);
            }
        }
        // Torso envelopes, rather than hands/feet: intended strike contact is allowed.
        for (i, a) in scene.characters.iter().enumerate() {
            if a.pose.root.x < scene.center.x - p.width as f32 / scene.zoom / 2. - 150.
                || a.pose.root.x > scene.center.x + p.width as f32 / scene.zoom / 2. + 150.
                || a.pose.root.y < scene.center.y - p.height as f32 / scene.zoom / 2. - 200.
                || a.pose.root.y > scene.center.y + p.height as f32 / scene.zoom / 2. + 200.
            {
                continue;
            }
            for b in scene.characters.iter().skip(i + 1) {
                let torso = |c: &EvaluatedCharacter| -> (Point, f32) {
                    let def = p.character(c.id).unwrap();
                    let chest = world_joint(&c.pose, def, "chest").unwrap_or(c.pose.root);
                    (
                        c.pose.root.lerp(chest, 0.5),
                        chest.distance(c.pose.root) * 0.25 + c.thickness * 0.5,
                    )
                };
                let (ac, ar) = torso(a);
                let (bc, br) = torso(b);
                let d = ac.distance(bc);
                let min = ar + br;
                if d < min {
                    overlaps.entry((a.id, b.id)).or_insert(SpacingIssue {
                        first: a.id,
                        second: b.id,
                        frame: *frame,
                        distance: d,
                        minimum: min,
                    });
                }
            }
        }
    }
    Ok(Review{layout:crate::composition::review(p,start,end,step)?,revision:p.revision,sampled_frames:frames.len(),contacts,overlaps:overlaps.into_values().collect(),continuity:crate::storyboard::analyze(&crate::storyboard::read(p)?),interactions:crate::interactions::review(p,start,end)?,note:"Declared prop contacts, opacity states and doorway passages are checked at every frame of their ranges. Torso envelopes and declared joint contacts are heuristic checks, not a physical collision simulation. Undeclared story actions cannot be inferred from dialogue. Review silhouettes and a moving preview before export.".into()})
}
