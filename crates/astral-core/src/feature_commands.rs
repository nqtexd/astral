use crate::*;
use anyhow::{Context, Result, bail, ensure};

pub(crate) fn execute(p: &mut Project, command: Command, created: &mut Vec<Id>) -> Result<()> {
    match command {
        Command::AddGraphic { graphic } => {
            created.push(graphic.id);
            p.graphics.push(graphic);
        }
        Command::ReplaceGraphic { graphic } => {
            let g = p
                .graphics
                .iter_mut()
                .find(|g| g.id == graphic.id)
                .context("Graphic missing")?;
            *g = graphic;
        }
        Command::SetGraphicKey { graphic, key } => {
            let g = p
                .graphics
                .iter_mut()
                .find(|g| g.id == graphic)
                .context("Graphic missing")?;
            if let Some(k) = g.keys.iter_mut().find(|k| k.frame == key.frame) {
                *k = key
            } else {
                g.keys.push(key);
                g.keys.sort_by_key(|k| k.frame);
            }
        }
        Command::AddDialogue { dialogue } => {
            created.push(dialogue.id);
            p.dialogues.push(dialogue);
        }
        Command::ReplaceDialogue { dialogue } => {
            let d = p
                .dialogues
                .iter_mut()
                .find(|d| d.id == dialogue.id)
                .context("Dialogue missing")?;
            *d = dialogue;
        }
        Command::TrainMotion {
            character,
            name,
            smoothing,
        } => {
            let model = train_character(p, character, smoothing, name)?;
            created.push(model.id);
            p.learned_motions.retain(|m| m.character != character);
            p.learned_motions.push(model);
        }
        Command::TrainDrawing {
            sources,
            name,
            smoothing,
        } => {
            let model = train_drawing(p, &sources, smoothing, name)?;
            for pair in model.sources.windows(2) {
                let next = p.drawings.iter().find(|d| d.id == pair[1]).unwrap().start;
                let source = p.drawings.iter_mut().find(|d| d.id == pair[0]).unwrap();
                source.end = next - 1;
            }
            created.push(model.id);
            p.drawing_motions.push(model);
        }
        Command::BakeMotion { motion } => {
            let model = p
                .learned_motions
                .iter()
                .find(|m| m.id == motion)
                .context("Learned motion missing")?
                .clone();
            ensure!(
                model.end - model.start <= 2000,
                "Bake at most 2001 frames at once"
            );
            let c = p.character(model.character).unwrap();
            ensure!(!c.locked, "Unlock character before baking");
            let keys = (model.start..=model.end)
                .map(|frame| PoseKey {
                    frame,
                    pose: sample_learned(p, c, frame as f64).unwrap(),
                    easing: Easing::Linear,
                })
                .collect::<Vec<_>>();
            let c = p
                .characters
                .iter_mut()
                .find(|c| c.id == model.character)
                .unwrap();
            c.keys
                .retain(|k| k.frame < model.start || k.frame > model.end);
            c.keys.extend(keys);
            c.keys.sort_by_key(|k| k.frame);
            p.learned_motions.retain(|m| m.id != motion);
        }
        Command::BakeDrawing { motion } => {
            let model = p
                .drawing_motions
                .iter()
                .find(|m| m.id == motion)
                .context("Learned stroke missing")?
                .clone();
            ensure!(
                model.end - model.start <= 2000,
                "Bake at most 2001 frames at once"
            );
            let final_hold = p
                .drawings
                .iter()
                .find(|d| Some(&d.id) == model.sources.last())
                .map_or(model.end, |d| d.end);
            let drawings = (model.start..=model.end)
                .map(|frame| Drawing {
                    id: Id::new_v4(),
                    name: format!("{} {frame}", model.name),
                    start: frame,
                    end: if frame == model.end {
                        final_hold
                    } else {
                        frame
                    },
                    points: sample_drawing(p, &model, frame as f64),
                    color: model.color,
                    width: model.width,
                })
                .collect::<Vec<_>>();
            created.extend(drawings.iter().map(|d| d.id));
            p.drawings.retain(|d| !model.sources.contains(&d.id));
            p.drawings.extend(drawings);
            p.drawing_motions.retain(|m| m.id != motion);
        }
        _ => bail!("Not a graphics or learning command"),
    }
    Ok(())
}
