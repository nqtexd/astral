//! Bounded offline orchestration. Creative agents can supply their own storyboards;
//! this starter director makes seeded procedural scenes, not LLM-generated stories.
use crate::{storyboard::*, studio::*, *};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const HISTORY: &str = "astral.director.recent_seeds.v1";

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AutoMode {
    #[default]
    Create,
    Improve,
    Continue,
    Inspect,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AutoSpec {
    pub mode: AutoMode,
    pub start: u32,
    pub end: u32,
    /// Empty means all visible rigs for Inspect/Improve. Continue requires selected rigs.
    pub characters: Vec<Id>,
    /// Explicit seeds reproduce the choreography. Omit to avoid recent move patterns.
    pub seed: Option<u64>,
    /// Only rest-length corrections are automatic; ambiguous acting/skids stay suggestions.
    pub max_repair_attempts: u32,
}
impl Default for AutoSpec {
    fn default() -> Self {
        Self {
            mode: AutoMode::Create,
            start: 0,
            end: 143,
            characters: vec![],
            seed: None,
            max_repair_attempts: 2,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AutoResult {
    pub plan: StudioPlan,
    pub mode: AutoMode,
    pub seed: Option<u64>,
    pub repair_rounds: u32,
    pub diagnostics_before: Vec<MotionIssue>,
    pub diagnostics_after: Vec<MotionIssue>,
    pub review: crate::staging::Review,
    pub quality_gate_passed: bool,
    pub pipeline: Vec<String>,
}

fn append(s: &mut Session, out: &mut StudioPlan, commands: Vec<Command>) -> Result<()> {
    if !commands.is_empty() {
        s.apply(Batch {
            project: s.project.id,
            expected_revision: s.project.revision,
            commands: commands.clone(),
        })?;
        out.commands.extend(commands);
    }
    Ok(())
}

pub fn plan(p: &Project, spec: AutoSpec) -> Result<AutoResult> {
    ensure!(
        spec.start < spec.end && spec.end < 1_000_000 && spec.end - spec.start < 1200,
        "Choose 2-1200 frames inside the timeline limit"
    );
    ensure!(
        spec.max_repair_attempts <= 3 && spec.characters.len() <= 8,
        "At most three repair rounds and eight selected rigs"
    );
    ensure!(
        spec.mode == AutoMode::Create || spec.mode == AutoMode::Continue || spec.end < p.duration,
        "Review range outside project"
    );
    let mut session = Session::new(p.clone())?;
    let mut out = StudioPlan {
        name: format!("Auto {:?}", spec.mode),
        ..Default::default()
    };
    let mut selected = spec.characters.clone();
    let mut seed = None;
    if spec.end >= p.duration {
        append(
            &mut session,
            &mut out,
            vec![Command::SetProject {
                name: None,
                duration: Some(spec.end + 1),
                background: None,
            }],
        )?;
    }
    match spec.mode {
        AutoMode::Create => {
            ensure!(
                p.characters.is_empty() && crate::storyboard::read(p)?.shots.is_empty(),
                "Auto Create needs a project without existing rigs/story shots; use Improve or Continue on existing work"
            );
            ensure!(
                p.width >= 480 && p.height >= 320,
                "Use a canvas at least 480 - 320 for a two-character scene"
            );
            ensure!(
                (48..=960).contains(&(spec.end - spec.start + 1)),
                "Auto Create needs 48-960 frames"
            );
            let mut recent: Vec<u64> = p
                .metadata
                .get(HISTORY)
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()?
                .unwrap_or_default();
            let base = spec.seed.unwrap_or_else(|| Id::new_v4().as_u128() as u64);
            let value = if spec.seed.is_some() {
                base
            } else {
                (0..8)
                    .map(|i| base.wrapping_add(i))
                    .find(|v| !recent.iter().rev().take(4).any(|r| r % 8 == v % 8))
                    .unwrap_or(base)
            };
            seed = Some(value);
            recent.push(value);
            if recent.len() > 16 {
                recent.remove(0);
            }
            let names = [
                ("Ember", "Tide"),
                ("Rook", "Lark"),
                ("Sol", "Vale"),
                ("Ash", "Frost"),
            ][value as usize % 4];
            let floor = p.height as f32 * 0.82;
            let mut a = stick_character(
                names.0.into(),
                Point::new(p.width as f32 * 0.36, floor - 94.),
                [116, 180, 225, 255],
            );
            let mut b = stick_character(
                names.1.into(),
                Point::new(p.width as f32 * 0.64, floor - 94.),
                [230, 139, 117, 255],
            );
            a.preserve_bones = true;
            b.preserve_bones = true;
            selected = vec![a.id, b.id];
            if spec.start > 0 {
                for c in [&mut a, &mut b] {
                    let pose = c.keys[0].pose.clone();
                    c.keys[0].pose.root = Point::new(-6000., -6000.);
                    c.keys[0].easing = Easing::Hold;
                    c.keys.push(PoseKey {
                        frame: spec.start,
                        pose,
                        easing: Easing::Smooth,
                    });
                }
            }
            append(
                &mut session,
                &mut out,
                vec![
                    Command::AddRig {
                        character: a.clone(),
                    },
                    Command::AddRig {
                        character: b.clone(),
                    },
                ],
            )?;
            let patterns = [
                [
                    FightAction::Punch,
                    FightAction::Dodge,
                    FightAction::Counter,
                    FightAction::Knockback,
                ],
                [
                    FightAction::Kick,
                    FightAction::Dodge,
                    FightAction::Punch,
                    FightAction::Counter,
                ],
                [
                    FightAction::Dodge,
                    FightAction::Punch,
                    FightAction::Counter,
                    FightAction::Kick,
                ],
                [
                    FightAction::Counter,
                    FightAction::Dodge,
                    FightAction::Kick,
                    FightAction::Punch,
                ],
                [
                    FightAction::Punch,
                    FightAction::Counter,
                    FightAction::Dodge,
                    FightAction::Kick,
                ],
                [
                    FightAction::Kick,
                    FightAction::Punch,
                    FightAction::Dodge,
                    FightAction::Counter,
                ],
                [
                    FightAction::Dodge,
                    FightAction::Kick,
                    FightAction::Punch,
                    FightAction::Knockback,
                ],
                [
                    FightAction::Counter,
                    FightAction::Kick,
                    FightAction::Dodge,
                    FightAction::Punch,
                ],
            ];
            let duration = spec.end - spec.start + 1;
            let beats: Vec<_> = patterns[value as usize % 8]
                .iter()
                .enumerate()
                .map(|(i, action)| FightBeat {
                    attacker: selected[(i + (value as usize / 8) % 2) % 2],
                    action: *action,
                    frames: duration / 4 + u32::from((i as u32) < duration % 4),
                    strength: 0.85 + ((value >> (i * 3)) % 5) as f32 * 0.1,
                })
                .collect();
            let motion = crate::studio::plan(
                &session.project,
                StudioOperation::Fight {
                    first: a.id,
                    second: b.id,
                    start: spec.start,
                    beats: beats.clone(),
                    style: MovementStyle::Natural,
                    impacts: true,
                    overwrite: false,
                },
            )?;
            out.warnings.extend(motion.warnings);
            append(&mut session, &mut out, motion.commands)?;
            let mut board = Storyboard {
                logline: format!(
                    "{} and {} test their timing in a four-beat spar; a feint creates the opening for the final reply.",
                    names.0, names.1
                ),
                characters: vec![],
                shots: vec![],
            };
            for (c, role) in [(&a, "Initiator"), (&b, "Counterfighter")] {
                board.characters.push(StoryCharacter {
                    id: c.id,
                    name: c.name.clone(),
                    role: role.into(),
                    appearance: "Editable native stick rig".into(),
                    motivation: "Win a controlled spar without injury".into(),
                    rig: Some(c.id),
                });
            }
            let mut cursor = spec.start;
            for (i, beat) in beats.iter().enumerate() {
                let last = cursor + beat.frames - 1;
                let mut cast = vec![];
                for id in &selected {
                    let entry = pose_at(&session.project, *id, cursor)?;
                    let exit = pose_at(&session.project, *id, last)?;
                    let state = |position| CharacterState {
                        position,
                        facing: if *id == a.id { 1 } else { -1 },
                        condition: "sparring".into(),
                        props: vec![],
                    };
                    cast.push(ShotCharacter {
                        character: *id,
                        entry: state(entry.root),
                        exit: state(exit.root),
                    });
                }
                board.shots.push(StoryShot {
                    id: Id::new_v4(),
                    title: format!("{} - {:?}", i + 1, beat.action),
                    start: cursor,
                    end: last,
                    location: "Sparring floor".into(),
                    action: format!(
                        "{} performs {:?}; the partner reacts and both recover.",
                        session.project.character(beat.attacker).unwrap().name,
                        beat.action
                    ),
                    camera: "Continuous two-character framing".into(),
                    audio: "Impact markers ready for separate sound clips".into(),
                    transition:
                        "Continuous spar; preserve each fighter's identity and screen side.".into(),
                    cast,
                });
                cursor = last + 1;
            }
            let mut finish = vec![
                Command::SetStoryboard { board },
                Command::SetDirectorHistory { seeds: recent },
            ];
            for frame in (spec.start..=spec.end)
                .step_by((p.fps / 2).max(1) as usize)
                .chain(std::iter::once(spec.end))
            {
                let x = pose_at(&session.project, a.id, frame)?.root;
                let y = pose_at(&session.project, b.id, frame)?.root;
                finish.push(Command::SetCamera {
                    key: CameraKey {
                        frame,
                        center: Point::new((x.x + y.x) * 0.5, floor - 130.),
                        zoom: 1.,
                    },
                });
            }
            append(&mut session, &mut out, finish)?;
            out.warnings.push("Seeded offline choreography, not an embedded language model. An external creative agent can replace the editable storyboard and beats. Add sound with the audio tools; no soundtrack is fabricated.".into());
        }
        AutoMode::Continue => {
            ensure!(
                !selected.is_empty() && spec.start > 0,
                "Continue needs selected rigs and a preceding frame"
            );
            ensure!(
                spec.end - spec.start <= p.fps * 3,
                "Continue at most three seconds, then inspect"
            );
            for id in &selected {
                let c = p.character(*id).context("Continue rig missing")?;
                ensure!(
                    !c.locked
                        && !p.actions.iter().any(|a| a.character == *id
                            && a.start <= spec.end
                            && a.end >= spec.start)
                        && !c
                            .contacts
                            .iter()
                            .any(|a| a.start <= spec.end && a.end >= spec.start)
                        && !c
                            .keys
                            .iter()
                            .any(|k| k.frame > spec.start && k.frame <= spec.end),
                    "Continue range contains existing motion/locks; choose an empty range"
                );
                let base = pose_at(p, *id, spec.start)?;
                let previous = pose_at(p, *id, spec.start - 1)?;
                let leg = c
                    .joints
                    .iter()
                    .find(|j| j.name == "right_foot")
                    .map_or(94., |j| j.position.y.abs())
                    .max(10.);
                ensure!(
                    (base.root.y - previous.root.y).abs() < leg * 0.03
                        && ["left_foot", "right_foot"].iter().all(|name| c
                            .joints
                            .iter()
                            .position(|j| j.name == *name)
                            .is_some_and(|i| base.joints[i].y > leg * 0.6)),
                    "Continue currently supports grounded locomotion; use authored keys for airborne/vertical motion"
                );
                let velocity = (base.root.x - previous.root.x)
                    .clamp(-leg * 3. / p.fps as f32, leg * 3. / p.fps as f32);
                let mut keys: Vec<_> = c
                    .keys
                    .iter()
                    .filter(|k| k.frame < spec.start || k.frame > spec.end)
                    .cloned()
                    .collect();
                for frame in spec.start..=spec.end {
                    let n = (frame - spec.start) as f32;
                    let duration = (spec.end - spec.start) as f32;
                    let u = n / duration;
                    let distance = velocity * n * (1. - u * 0.5);
                    let mut pose = base.clone();
                    pose.root.x += distance;
                    crate::locomotion::gait(
                        c,
                        &mut pose,
                        distance,
                        velocity * duration * 0.5,
                        1.,
                        velocity.abs() * p.fps as f32 > leg * 1.8,
                    );
                    keys.push(PoseKey {
                        frame,
                        pose,
                        easing: Easing::Linear,
                    });
                }
                keys.sort_by_key(|k| k.frame);
                append(
                    &mut session,
                    &mut out,
                    vec![Command::SetPoseKeys {
                        character: *id,
                        keys,
                    }],
                )?;
            }
            out.warnings.push("Continuation extrapolates horizontal travel into a decelerating walk/run. It does not invent story events, infer airborne physics or overwrite existing motion.".into());
        }
        AutoMode::Improve | AutoMode::Inspect => {}
    }
    let before = crate::studio::analyze(&session.project, spec.start, spec.end, &selected, 0.12)?;
    let mut after = before.clone();
    let mut rounds = 0;
    if spec.mode != AutoMode::Inspect {
        for _ in 0..spec.max_repair_attempts {
            let repairs: Vec<_> = after
                .iter()
                .filter(|i| i.kind == "limb_length")
                .flat_map(|i| i.commands.clone())
                .collect();
            if repairs.is_empty() {
                break;
            }
            append(&mut session, &mut out, repairs)?;
            rounds += 1;
            let next =
                crate::studio::analyze(&session.project, spec.start, spec.end, &selected, 0.12)?;
            let remaining = next.iter().filter(|i| i.kind == "limb_length").count();
            after = next;
            if remaining == 0 {
                break;
            }
        }
    }
    let mut review = crate::staging::review(&session.project, spec.start, spec.end, 1)?;
    review.revision = p
        .revision
        .checked_add(u64::from(!out.commands.is_empty()))
        .context("Revision overflow")?;
    let passed = review.contacts.iter().all(|c| c.passed)
        && review.overlaps.is_empty()
        && review.interactions.iter().all(|c| c.passed)
        && after.iter().all(|i| i.kind != "limb_length");
    out.warnings.push(format!("Geometric gate: {}. {} declared contacts, {} spacing issues, {} failed prop interactions. No artistic score is inferred.",if passed{"passed"}else{"failed - fix before accepting"},review.contacts.len(),review.overlaps.len(),review.interactions.iter().filter(|c|!c.passed).count()));
    out.diagnostics = after.clone();
    out.warnings.push("Foot slides, root jumps and timing may be intentional. They remain explicit suggestions; no arbitrary 0-100 artistic scores are assigned. Inspect a moving preview before export.".into());
    Ok(AutoResult {
        plan: out,
        mode: spec.mode,
        seed,
        repair_rounds: rounds,
        diagnostics_before: before,
        diagnostics_after: after,
        review,
        quality_gate_passed: passed,
        pipeline: vec![
            "plan".into(),
            "editable motion".into(),
            "inspect".into(),
            format!("bounded repairs: {rounds}"),
            "review before export".into(),
        ],
    })
}
