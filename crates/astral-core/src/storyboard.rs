//! Editable preproduction data. Stored in metadata so older projects remain readable.
use crate::*;
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const KEY: &str = "astral.storyboard.v1";
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Storyboard {
    pub logline: String,
    pub characters: Vec<StoryCharacter>,
    pub shots: Vec<StoryShot>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct StoryCharacter {
    pub id: Id,
    pub name: String,
    pub role: String,
    pub appearance: String,
    pub motivation: String,
    pub rig: Option<Id>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CharacterState {
    pub position: Point,
    /// -1 faces left, 1 faces right.
    pub facing: i32,
    pub condition: String,
    pub props: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShotCharacter {
    pub character: Id,
    pub entry: CharacterState,
    pub exit: CharacterState,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct StoryShot {
    pub id: Id,
    pub title: String,
    pub start: u32,
    pub end: u32,
    pub location: String,
    pub action: String,
    pub camera: String,
    pub audio: String,
    /// Explain deliberate time jumps, reversals or changes in condition/props.
    pub transition: String,
    pub cast: Vec<ShotCharacter>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ContinuityIssue {
    pub shot: Id,
    pub character: Option<Id>,
    pub kind: String,
    pub message: String,
}
pub fn read(p: &Project) -> Result<Storyboard> {
    p.metadata
        .get(KEY)
        .map(|v| serde_json::from_value(v.clone()).map_err(Into::into))
        .unwrap_or_else(|| Ok(Storyboard::default()))
}
pub fn validate(p: &Project, b: &Storyboard) -> Result<()> {
    ensure!(
        b.logline.len() <= 8192 && b.characters.len() <= 128 && b.shots.len() <= 512,
        "Storyboard exceeds limits"
    );
    let text = |s: &str| s.len() <= 8192;
    let mut ids = BTreeSet::new();
    for c in &b.characters {
        ensure!(
            ids.insert(c.id)
                && !c.name.trim().is_empty()
                && [&c.name, &c.role, &c.appearance, &c.motivation]
                    .iter()
                    .all(|s| text(s)),
            "Invalid storyboard character"
        );
        ensure!(
            c.rig.is_none_or(|id| p.character(id).is_some()),
            "Storyboard rig is missing"
        );
    }
    let cast_ids = ids.clone();
    let mut previous = None;
    for shot in &b.shots {
        ensure!(
            ids.insert(shot.id) && !shot.title.trim().is_empty(),
            "Duplicate or unnamed storyboard shot"
        );
        ensure!(
            shot.start <= shot.end
                && shot.end < p.duration
                && previous.is_none_or(|end| shot.start > end),
            "Shots must be ordered, non-overlapping and inside the project"
        );
        ensure!(
            [
                &shot.title,
                &shot.location,
                &shot.action,
                &shot.camera,
                &shot.audio,
                &shot.transition
            ]
            .iter()
            .all(|s| text(s)),
            "Shot text exceeds limits"
        );
        let mut seen = BTreeSet::new();
        for c in &shot.cast {
            ensure!(
                cast_ids.contains(&c.character) && seen.insert(c.character),
                "Shot refers to a missing or duplicate cast member"
            );
            for state in [&c.entry, &c.exit] {
                ensure!(
                    state.position.finite()
                        && [-1, 1].contains(&state.facing)
                        && text(&state.condition)
                        && state.props.len() <= 32
                        && state
                            .props
                            .iter()
                            .all(|s| !s.trim().is_empty() && s.len() <= 256),
                    "Invalid character continuity state"
                );
            }
        }
        previous = Some(shot.end);
    }
    Ok(())
}
pub fn analyze(b: &Storyboard) -> Vec<ContinuityIssue> {
    let mut issues = vec![];
    let mut last: BTreeMap<Id, (&StoryShot, &CharacterState)> = BTreeMap::new();
    for shot in &b.shots {
        let mut add = |character, kind: &str, message: String| {
            issues.push(ContinuityIssue {
                shot: shot.id,
                character,
                kind: kind.into(),
                message,
            })
        };
        if shot.location.trim().is_empty() || shot.action.trim().is_empty() {
            add(
                None,
                "incomplete_shot",
                "Add the location and story action before animating.".into(),
            );
        }
        for c in &shot.cast {
            if let Some((previous, state)) = last.get(&c.character)
                && shot.transition.trim().is_empty()
            {
                if state.condition != c.entry.condition {
                    add(
                        Some(c.character),
                        "condition_change",
                        format!(
                            "Condition changes after '{}'; explain it in the transition.",
                            previous.title
                        ),
                    );
                }
                let props = |s: &CharacterState| s.props.iter().cloned().collect::<BTreeSet<_>>();
                if props(state) != props(&c.entry) {
                    add(
                        Some(c.character),
                        "prop_change",
                        format!(
                            "Props change after '{}'; plan the pickup or loss.",
                            previous.title
                        ),
                    );
                }
                if previous.location == shot.location
                    && state.position.distance(c.entry.position) > 80.
                {
                    add(
                        Some(c.character),
                        "position_jump",
                        "Character jumps within the same location without a transition.".into(),
                    );
                }
                if state.facing != c.entry.facing {
                    add(
                        Some(c.character),
                        "screen_direction",
                        "Facing reverses across the cut; plan a turn or motivated camera reversal."
                            .into(),
                    );
                }
            }
            if c.entry.condition == "dead"
                && c.exit.condition != "dead"
                && shot.transition.trim().is_empty()
            {
                add(
                    Some(c.character),
                    "revival",
                    "Defeated character returns without explanation.".into(),
                );
            }
            last.insert(c.character, (shot, &c.exit));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn board_roundtrip_undo_and_atomic_validation() {
        let p = Project::new("Story", 1920, 1080, 60, 600);
        let mut s = Session::new(p).unwrap();
        let id = s.project.id;
        let board = Storyboard {
            logline: "Escape".into(),
            ..Default::default()
        };
        s.apply(Batch {
            project: id,
            expected_revision: 0,
            commands: vec![Command::SetStoryboard {
                board: board.clone(),
            }],
        })
        .unwrap();
        assert_eq!(read(&s.project).unwrap().logline, "Escape");
        s.undo(id, 1).unwrap();
        assert!(read(&s.project).unwrap().logline.is_empty());
        let mut bad = board;
        bad.characters.push(StoryCharacter {
            id: Id::new_v4(),
            name: "Ren".into(),
            role: "Lead".into(),
            appearance: "Scarf".into(),
            motivation: "Escape".into(),
            rig: Some(Id::new_v4()),
        });
        let before = serde_json::to_value(&s.project).unwrap();
        assert!(
            s.apply(Batch {
                project: id,
                expected_revision: s.project.revision,
                commands: vec![Command::SetStoryboard { board: bad }]
            })
            .is_err()
        );
        assert_eq!(before, serde_json::to_value(&s.project).unwrap());
    }
}
