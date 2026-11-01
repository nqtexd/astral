use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrameEdit {
    #[default]
    Copy,
    Move,
    Stretch,
    Translate,
    Hold,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pose_batch(
    p: &Project,
    character: Id,
    start: u32,
    end: u32,
    action: FrameEdit,
    target: u32,
    new_end: u32,
    delta: Point,
    overwrite: bool,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    let c = editable_character(p, character)?;
    ensure!(delta.finite(), "Pose offset must be finite");
    let mut source: Vec<_> = c
        .keys
        .iter()
        .filter(|k| k.frame >= start && k.frame <= end)
        .cloned()
        .collect();
    ensure!(
        !source.is_empty(),
        "No authored poses in the selected range"
    );
    let (destination, destination_end) = match action {
        FrameEdit::Copy | FrameEdit::Move => (
            target,
            target.checked_add(end - start).context("Frame overflow")?,
        ),
        FrameEdit::Stretch | FrameEdit::Hold => (start, new_end),
        FrameEdit::Translate => (start, end),
    };
    ensure!(
        destination <= destination_end
            && destination_end < 1_000_000
            && destination_end - destination <= 1200,
        "Invalid pose destination"
    );
    let conflict = c.keys.iter().any(|k| {
        k.frame >= destination
            && k.frame <= destination_end
            && !(action != FrameEdit::Copy && k.frame >= start && k.frame <= end)
    });
    ensure!(
        overwrite || !conflict,
        "Destination contains pose keys; enable replacement"
    );
    let mut keys: Vec<_> = c
        .keys
        .iter()
        .filter(|k| {
            !(k.frame >= destination && k.frame <= destination_end)
                && !(action != FrameEdit::Copy && k.frame >= start && k.frame <= end)
        })
        .cloned()
        .collect();
    if action == FrameEdit::Hold {
        let mut key = source.remove(0);
        key.frame = start;
        key.easing = Easing::Hold;
        source = vec![key];
    }
    for mut key in source {
        key.frame = match action {
            FrameEdit::Copy | FrameEdit::Move => destination + key.frame - start,
            FrameEdit::Stretch => {
                ensure!(
                    end > start && new_end > start,
                    "Stretch needs at least two frames"
                );
                start
                    + ((key.frame - start) as f64 * (new_end - start) as f64 / (end - start) as f64)
                        .round() as u32
            }
            _ => key.frame,
        };
        if action == FrameEdit::Translate {
            key.pose.root = key.pose.root + delta;
        }
        keys.push(key);
    }
    if keys.iter().all(|k| k.frame > 0) {
        keys.push(c.keys[0].clone());
        keys.last_mut().unwrap().frame = 0;
    }
    keys.sort_by_key(|k| k.frame);
    ensure!(
        keys.windows(2).all(|w| w[0].frame < w[1].frame),
        "Retiming would merge pose keys"
    );
    let mut out = StudioPlan::named("Batch pose editing");
    out.commands.push(Command::SetPoseKeys { character, keys });
    extend_to(&mut out, p, destination_end + 1)?;
    out.warnings.push("Only authored pose keys change. Existing action clips and other tracks retain their timing; adjacent interpolation can change.".into());
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn frame_batch(
    p: &Project,
    layer: Id,
    start: u32,
    end: u32,
    action: FrameEdit,
    target: u32,
    new_end: u32,
    delta: Point,
    overwrite: bool,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    ensure!(
        p.layers.iter().any(|l| l.id == layer && !l.locked),
        "Select an unlocked drawing layer"
    );
    ensure!(delta.finite(), "Drawing offset must be finite");
    let mut source: Vec<_> = p
        .cels
        .iter()
        .filter(|c| c.layer == layer && c.frame >= start && c.frame <= end)
        .cloned()
        .collect();
    source.sort_by_key(|c| c.frame);
    if source.first().is_none_or(|c| c.frame != start) {
        source.insert(
            0,
            DrawingCel {
                id: Id::new_v4(),
                layer,
                frame: start,
                strokes: p.cel_at(layer, start).map_or(vec![], |c| c.strokes.clone()),
            },
        );
    }
    source.sort_by_key(|c| c.frame);
    let mut out = StudioPlan::named(match action {
        FrameEdit::Copy => "Copy frame range",
        FrameEdit::Move => "Move frame range",
        FrameEdit::Stretch => "Stretch drawing exposure",
        FrameEdit::Translate => "Move drawings together",
        FrameEdit::Hold => "Hold drawing",
    });
    let (destination, destination_end) = match action {
        FrameEdit::Copy | FrameEdit::Move => (
            target,
            target
                .checked_add(end - start)
                .context("Frame range overflow")?,
        ),
        FrameEdit::Stretch | FrameEdit::Hold => {
            ensure!(new_end >= start, "End must follow the first frame");
            (start, new_end)
        }
        FrameEdit::Translate => (start, end),
    };
    ensure!(
        destination_end < 1_000_000 && destination_end - destination <= 1200,
        "Limit the destination to 1201 frames"
    );
    let mut cels: Vec<_> = p
        .cels
        .iter()
        .filter(|c| c.layer == layer)
        .cloned()
        .collect();
    if matches!(
        action,
        FrameEdit::Copy | FrameEdit::Move | FrameEdit::Stretch
    ) {
        let conflicts = cels.iter().any(|c| {
            c.frame >= destination
                && c.frame <= destination_end
                && !(action != FrameEdit::Copy && c.frame >= start && c.frame <= end)
        });
        ensure!(
            overwrite || !conflicts,
            "Destination contains authored keys; enable replacement to continue"
        );
    }
    let restore = |frame: u32| DrawingCel {
        id: Id::new_v4(),
        layer,
        frame,
        strokes: p.cel_at(layer, frame).map_or(vec![], |c| c.strokes.clone()),
    };
    let clear_end = end.max(destination_end);
    if action != FrameEdit::Copy {
        cels.retain(|c| c.frame < start || c.frame > end);
    }
    cels.retain(|c| c.frame < destination || c.frame > destination_end);
    let mut inserted = vec![];
    match action {
        FrameEdit::Copy | FrameEdit::Move => {
            for mut cel in source {
                if action == FrameEdit::Copy {
                    cel.id = Id::new_v4();
                }
                cel.frame = destination + (cel.frame - start);
                inserted.push(cel);
            }
        }
        FrameEdit::Stretch => {
            ensure!(
                end > start && destination_end > start,
                "Stretch needs at least two frames"
            );
            for mut cel in source {
                cel.frame = start
                    + ((cel.frame - start) as f64 * (destination_end - start) as f64
                        / (end - start) as f64)
                        .round() as u32;
                inserted.push(cel);
            }
        }
        FrameEdit::Translate => {
            for mut cel in source {
                for stroke in &mut cel.strokes {
                    for point in &mut stroke.points {
                        *point = *point + delta;
                    }
                }
                inserted.push(cel);
            }
        }
        FrameEdit::Hold => {
            let mut first = source.remove(0);
            first.frame = start;
            inserted.push(first);
        }
    }
    let mut positions = BTreeSet::new();
    ensure!(
        inserted.iter().all(|c| positions.insert(c.frame)),
        "Retiming would merge drawing keys; choose a longer range"
    );
    if action == FrameEdit::Move && !(start >= destination && start <= destination_end) {
        cels.retain(|c| c.frame != start);
        cels.push(DrawingCel {
            id: Id::new_v4(),
            layer,
            frame: start,
            strokes: vec![],
        });
    }
    let restore_frames = if action == FrameEdit::Copy {
        vec![destination_end.saturating_add(1)]
    } else {
        vec![end.saturating_add(1), destination_end.saturating_add(1)]
    };
    for frame in restore_frames {
        if frame < p.duration
            && !(frame >= destination && frame <= destination_end)
            && !(action != FrameEdit::Copy && frame >= start && frame <= end)
            && !cels.iter().any(|c| c.frame == frame)
        {
            cels.push(restore(frame));
        }
    }
    if action == FrameEdit::Stretch && destination_end < end {
        cels.retain(|c| c.frame <= destination_end || c.frame > end);
        if end + 1 < p.duration && !cels.iter().any(|c| c.frame == destination_end + 1) {
            let mut next = restore(end + 1);
            next.frame = destination_end + 1;
            cels.push(next);
        }
    }
    cels.extend(inserted);
    cels.sort_by_key(|c| c.frame);
    out.commands.push(Command::SetCels { layer, cels });
    extend_to(&mut out, p, clear_end.max(destination_end) + 1)?;
    out.warnings.push("Only this drawing layer changes. Other animation, effects and audio tracks retain their timing.".into());
    Ok(out)
}
pub(super) fn retime_seconds(
    p: &Project,
    start: u32,
    end: u32,
    seconds: f32,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    ensure!(
        start < end && seconds.is_finite() && (0.05..=120.).contains(&seconds),
        "Choose a nonzero range and 0.05–120 seconds"
    );
    let count = (seconds * p.fps as f32).round().max(2.) as u32;
    let new_end = start.checked_add(count - 1).context("Timing overflow")?;
    let duration = p.duration as i64 + new_end as i64 - end as i64;
    ensure!(
        (2..=1_000_000).contains(&duration),
        "New timing exceeds project bounds"
    );
    let mut out = StudioPlan::named("Retime scene range");
    if duration as u32 > p.duration {
        out.commands.push(Command::SetProject {
            name: None,
            duration: Some(duration as u32),
            background: None,
        });
    }
    out.commands.push(Command::Retime {
        start,
        end,
        new_end,
    });
    if (duration as u32) < p.duration {
        out.commands.push(Command::SetProject {
            name: None,
            duration: Some(duration as u32),
            background: None,
        });
    }
    out.warnings.push("This is a ripple retime across the scene. Audio cue starts move; source sound duration and pitch stay unchanged.".into());
    Ok(out)
}
pub(super) fn frame_rate(p: &Project, fps: u32, preserve_seconds: bool) -> Result<StudioPlan> {
    ensure!((1..=240).contains(&fps), "FPS must be 1–240");
    let mut out = StudioPlan::named("Project frame rate");
    if preserve_seconds && fps != p.fps {
        let duration = (p.duration as f64 * fps as f64 / p.fps as f64).round() as u32;
        ensure!(
            (2..=1_000_000).contains(&duration) && p.duration >= 2,
            "Cannot preserve timing for this duration"
        );
        if duration > p.duration {
            out.commands.push(Command::SetProject {
                name: None,
                duration: Some(duration),
                background: None,
            });
        }
        out.commands.push(Command::SetFrameRate { fps });
        out.commands.push(Command::Retime {
            start: 0,
            end: p.duration - 1,
            new_end: duration - 1,
        });
        if duration < p.duration {
            out.commands.push(Command::SetProject {
                name: None,
                duration: Some(duration),
                background: None,
            });
        }
        out.warnings.push("Keys are remapped to preserve approximate duration. A rate that merges authored keys is rejected.".into());
    } else {
        out.commands.push(Command::SetFrameRate { fps });
        out.warnings.push(
            "Frame numbers are unchanged; playback duration and speed change with FPS.".into(),
        );
    }
    Ok(out)
}
