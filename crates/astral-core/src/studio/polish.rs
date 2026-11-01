use super::*;
/// Build a reviewed proposal; never rewrite authored anchors or declared contacts.
pub fn polish(
    p: &Project,
    character: Id,
    start: u32,
    end: u32,
    strength: f32,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    ensure!(
        end - start <= 1200 && strength.is_finite() && (0.0..=1.).contains(&strength),
        "Polish a shot of at most 1201 frames"
    );
    let c = p.character(character).context("Character missing")?;
    ensure!(!c.locked, "Character is locked");
    let mut smooth = p.clone();
    for key in &mut smooth
        .characters
        .iter_mut()
        .find(|c| c.id == character)
        .unwrap()
        .keys
    {
        if key.frame >= start && key.frame <= end && !matches!(key.easing, Easing::Hold) {
            key.easing = Easing::Flow;
        }
    }
    let anchors: BTreeSet<_> = c
        .keys
        .iter()
        .map(|k| k.frame)
        .chain(
            crate::staging::cues(p)?
                .into_iter()
                .filter(|cue| cue.attacker == character || cue.target == character)
                .map(|cue| cue.frame),
        )
        .collect();
    let poses: Vec<Pose> = (start..=end)
        .map(|frame| {
            evaluate(
                if anchors.contains(&frame) { p } else { &smooth },
                frame as f64,
            )
            .characters
            .into_iter()
            .find(|c| c.id == character)
            .map(|c| c.pose)
            .context("Character hidden in polish range")
        })
        .collect::<Result<_>>()?;
    let props = !crate::interactions::read(p)?.is_empty();
    let mut out = StudioPlan::named("Polish motion");
    let mut contacts = 0;
    for (joint, def) in c
        .joints
        .iter()
        .enumerate()
        .filter(|(_, j)| j.name.ends_with("foot"))
    {
        let points: Vec<_> = poses
            .iter()
            .map(|pose| pose.root + pose.joints[joint])
            .collect();
        let floor = points.iter().map(|q| q.y).fold(f32::NEG_INFINITY, f32::max);
        let tolerance = (c.thickness * 0.12).max(0.6);
        let mut window = None;
        for i in 0..=points.len() {
            let grounded = i < points.len()
                && points[i].y >= floor - c.thickness * 0.6
                && (i == 0 || points[i].distance(points[i - 1]) <= tolerance);
            if grounded && window.is_none() {
                window = Some(i);
            }
            if (!grounded || i == points.len()) && window.is_some() {
                let first = window.take().unwrap();
                let last = i.saturating_sub(1);
                if last >= first + 2 {
                    let target = points[first];
                    let safe =
                        (first..=last).all(|offset| !anchors.contains(&(start + offset as u32)));
                    let overlap = c.contacts.iter().any(|v| {
                        v.joint == joint
                            && v.start <= start + last as u32
                            && v.end >= start + first as u32
                    });
                    if safe && !overlap {
                        out.commands.push(Command::SetContact {
                            character,
                            contact: Contact {
                                start: start + first as u32,
                                end: start + last as u32,
                                joint,
                                target,
                            },
                        });
                        contacts += 1;
                    }
                }
            }
        }
        let _ = def;
    }
    let chest = c.joints.iter().position(|j| j.name == "chest");
    for (offset, source) in poses.iter().enumerate() {
        let frame = start + offset as u32;
        let mut pose = source.clone();
        if !anchors.contains(&frame)
            && !props
            && let Some(chest) = chest
        {
            let previous = &poses[offset.saturating_sub(1)];
            let next = &poses[(offset + 1).min(poses.len() - 1)];
            let acceleration = next.root.x - 2. * pose.root.x + previous.root.x;
            let lean = (-acceleration * 0.025 * strength).clamp(-0.12, 0.12);
            let pivot = pose.joints[chest];
            for j in 0..pose.joints.len() {
                let mut ancestor = Some(j);
                let mut upper = false;
                while let Some(index) = ancestor {
                    if index == chest {
                        upper = true;
                        break;
                    }
                    ancestor = c.joints[index].parent;
                }
                if upper && j != chest {
                    let q = pose.joints[j] - pivot;
                    pose.joints[j] = pivot
                        + Point::new(
                            q.x * lean.cos() - q.y * lean.sin(),
                            q.x * lean.sin() + q.y * lean.cos(),
                        );
                }
            }
        }
        let easing = c
            .keys
            .iter()
            .find(|k| k.frame == frame)
            .map_or(Easing::Flow, |k| {
                if matches!(k.easing, Easing::Hold) {
                    Easing::Hold
                } else {
                    Easing::Flow
                }
            });
        out.commands.push(Command::SetPose {
            character,
            frame,
            pose,
            easing,
        });
    }
    out.warnings.push(format!("Proposes {contacts} planted-foot ranges and bounded upper-body follow-through. Authored/contact anchors are preserved. Inspect moving previews; this is not a physics or acting generator."));
    if props {
        out.warnings.push("Declared prop interactions detected: secondary body changes were disabled to protect those contacts.".into());
    }
    out.validate(p)?;
    Ok(out)
}
