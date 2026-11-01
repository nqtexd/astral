use super::*;
use crate::staging::{ContactCue, aim_limb, world_joint};
#[allow(clippy::too_many_arguments)]
pub(super) fn fight(
    p: &Project,
    first: Id,
    second: Id,
    start: u32,
    beats: &[FightBeat],
    style: MovementStyle,
    impacts: bool,
    overwrite: bool,
) -> Result<StudioPlan> {
    ensure!(first != second, "Choose two different characters");
    let a = editable_character(p, first)?;
    let b = editable_character(p, second)?;
    ensure!(
        start < p.duration && !beats.is_empty() && beats.len() <= 32,
        "Choose 1–32 beats inside the timeline"
    );
    let mut total = 0u32;
    for beat in beats {
        ensure!(
            [first, second].contains(&beat.attacker)
                && (6..=240).contains(&beat.frames)
                && beat.strength.is_finite()
                && (0.1..=3.).contains(&beat.strength),
            "Invalid fight beat"
        );
        total = total
            .checked_add(beat.frames)
            .context("Timeline overflow")?;
    }
    ensure!(total <= 1200, "Limit one sequence to 1200 frames");
    let last = start.checked_add(total - 1).context("Timeline overflow")?;
    ensure!(last < 1_000_000, "Timeline limit exceeded");
    let mut out = StudioPlan::named("Contact-aware choreography");
    extend_to(&mut out, p, last + 1)?;
    let mut expanded = p.clone();
    expanded.duration = expanded.duration.max(last + 1);
    for c in [a, b] {
        ensure!(
            !c.contacts.iter().any(|k| k.start <= last && k.end >= start),
            "Adjust existing foot locks in this range first"
        );
        let actions: Vec<_> = p
            .actions
            .iter()
            .filter(|v| v.character == c.id && v.start <= last && v.end >= start)
            .collect();
        ensure!(
            overwrite
                || (actions.is_empty()
                    && !c.keys.iter().any(|k| k.frame > start && k.frame <= last)),
            "Range contains motion; enable replacement"
        );
        for action in actions {
            out.commands.push(Command::Remove { id: action.id });
        }
        if overwrite {
            let mut keys: Vec<_> = c
                .keys
                .iter()
                .filter(|k| k.frame < start || k.frame > last)
                .cloned()
                .collect();
            if keys.is_empty() {
                keys.push(PoseKey {
                    frame: start,
                    pose: pose_at(p, c.id, start)?,
                    easing: Easing::Linear,
                });
            }
            out.commands.push(Command::SetPoseKeys {
                character: c.id,
                keys,
            });
        }
        out.commands.push(Command::SetBoneConstraints {
            character: c.id,
            enabled: true,
        });
    }
    let mut current = BTreeMap::from([
        (first, pose_at(p, first, start)?),
        (second, pose_at(p, second, start)?),
    ]);
    let mut cues = crate::staging::cues(p)?;
    cues.retain(|c| {
        !(c.frame >= start && c.frame <= last && [first, second].contains(&c.attacker))
    });
    let mut cursor = start;
    for (index, beat) in beats.iter().enumerate() {
        let actor = if beat.attacker == first { a } else { b };
        let defender = if actor.id == first { b } else { a };
        let base = current[&actor.id].clone();
        let victim = current[&defender.id].clone();
        let dir = if victim.root.x >= base.root.x {
            1.
        } else {
            -1.
        };
        let control = if beat.action == FightAction::Kick {
            "right_foot"
        } else {
            "right_hand"
        };
        let end = actor
            .joints
            .iter()
            .position(|j| j.name == control)
            .context("Preset needs named hand/foot controls")?;
        let mid = actor.joints[end].parent.context("Missing limb parent")?;
        let anchor = actor.joints[mid].parent.context("Missing limb anchor")?;
        let reach = actor.joints[anchor]
            .position
            .distance(actor.joints[mid].position)
            + actor.joints[mid]
                .position
                .distance(actor.joints[end].position);
        let target =
            world_joint(&victim, defender, "chest").context("Preset needs a chest control")?;
        let dy = target.y - (base.root.y + base.joints[anchor].y);
        ensure!(
            dy.abs() < reach * 0.98,
            "Target too high/low for this strike; change blocking or choose a different effector"
        );
        let horizontal = ((reach * 0.96).powi(2) - dy * dy).max(1.).sqrt();
        let mut contact_root = base.root;
        contact_root.x = target.x - base.joints[anchor].x - dir * horizontal;
        let minimum = (actor.thickness + defender.thickness) * 0.5 + 24.;
        ensure!(
            dir * (victim.root.x - contact_root.x) >= minimum,
            "Strike would overlap torsos; adjust height or rig spacing"
        );
        let hit_offset = ((beat.frames - 1) as f32 * 0.48).round() as u32;
        let contact = cursor + hit_offset;
        let hold = (beat.frames / 12).clamp(1, 3);
        let release = (hit_offset + hold).min(beat.frames - 2);
        let strike = !matches!(beat.action, FightAction::Dodge | FightAction::Knockback);
        for offset in 0..beat.frames {
            let fraction = offset as f32 / (beat.frames - 1) as f32;
            let mut pose = base.clone();
            let mut other = victim.clone();
            if strike {
                let approach = ease(&Easing::Smooth, (fraction / 0.34).clamp(0., 1.));
                pose.root = base.root.lerp(contact_root, approach);
                if fraction < 0.34 {
                    let travel = pose_distance(base.root, pose.root);
                    crate::locomotion::gait(
                        actor,
                        &mut pose,
                        travel,
                        contact_root.x - base.root.x,
                        0.7,
                        false,
                    );
                }
                let rest = pose.root + base.joints[end];
                let windup = rest + Point::new(-dir * 28. * beat.strength, -12.);
                let goal = if offset < hit_offset {
                    let t = offset as f32 / hit_offset.max(1) as f32;
                    if t < 0.45 {
                        rest.lerp(windup, t / 0.45)
                    } else {
                        windup.lerp(target, ((t - 0.45) / 0.55).powf(1.7))
                    }
                } else if offset <= release {
                    target
                } else {
                    target.lerp(
                        rest,
                        ease(
                            &Easing::Smooth,
                            (offset - release) as f32 / (beat.frames - 1 - release) as f32,
                        ),
                    )
                };
                let error = aim_limb(
                    actor,
                    &mut pose,
                    control,
                    goal,
                    if control == "right_foot" { -dir } else { dir },
                )?;
                if offset == hit_offset {
                    ensure!(
                        error < 1.,
                        "Strike target remains unreachable; adjust blocking"
                    );
                }
                let reaction = ease(
                    &Easing::Smooth,
                    offset.saturating_sub(release) as f32 / (beat.frames - 1 - release) as f32,
                );
                other.root.x += dir * 42. * beat.strength * reaction;
                // Recoil begins after contact; target stays still through the impact hold.
                let angle = dir * 0.16 * reaction;
                for point in &mut other.joints {
                    let old = *point;
                    point.x = old.x * angle.cos() - old.y * angle.sin();
                    point.y = old.x * angle.sin() + old.y * angle.cos();
                }
            } else if beat.action == FightAction::Dodge {
                pose.root.x -= dir * 60. * beat.strength * (fraction * std::f32::consts::PI).sin();
                pose.root.y += 25. * (fraction * std::f32::consts::PI).sin();
            } else {
                pose.root.x -= dir * 120. * beat.strength * fraction;
                pose.root.y -= (fraction * std::f32::consts::PI).sin() * 40.;
            }
            constrain_bones(actor, &mut pose);
            constrain_bones(defender, &mut other);
            for (id, value) in [(actor.id, pose.clone()), (defender.id, other.clone())] {
                out.commands.push(Command::SetPose {
                    character: id,
                    frame: cursor + offset,
                    pose: value,
                    easing: Easing::Linear,
                });
            }
            if offset == beat.frames - 1 {
                current.insert(actor.id, pose);
                current.insert(defender.id, other);
            }
        }
        if !strike {
            out.markers.insert(
                format!("beat_{}_{:?} @{}", index + 1, beat.action, start),
                contact,
            );
        }
        if strike {
            let name = format!("beat_{}_contact @{}", index + 1, start);
            out.markers.insert(name.clone(), contact);
            cues.push(ContactCue {
                name,
                frame: contact,
                attacker: actor.id,
                target: defender.id,
                effector: control.into(),
                target_joint: "chest".into(),
                tolerance: 2.,
            });
            if impacts {
                let accents = impact(
                    &expanded,
                    defender.id,
                    "chest",
                    contact,
                    if beat.strength > 1.4 {
                        ImpactPack::Heavy
                    } else {
                        ImpactPack::Light
                    },
                )?;
                out.commands.extend(accents.commands);
            }
        }
        cursor += beat.frames;
    }
    out.commands.push(Command::SetContactCues { cues });
    let mut markers = p.markers.clone();
    markers.extend(out.markers.clone());
    out.commands.push(Command::SetMarkers { markers });
    out.warnings.push(format!("{:?} contact choreography: impacts follow the defender's chest, reactions follow the impact hold. Review torso envelopes and a moving preview; this is not a collision simulation.",style));
    Ok(out)
}
fn pose_distance(a: Point, b: Point) -> f32 {
    b.x - a.x
}
