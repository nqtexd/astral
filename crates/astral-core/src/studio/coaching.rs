use super::*;
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MotionIssue {
    pub kind: String,
    pub character: Id,
    pub joint: Option<String>,
    pub frame: u32,
    pub message: String,
    pub commands: Vec<Command>,
}
pub fn analyze(
    p: &Project,
    start: u32,
    end: u32,
    characters: &[Id],
    tolerance: f32,
) -> Result<Vec<MotionIssue>> {
    frame_range(p, start, end)?;
    ensure!(
        tolerance.is_finite() && (0.03..=0.5).contains(&tolerance),
        "Review tolerance must be 3–50 percent"
    );
    let selected: Vec<_> = p
        .characters
        .iter()
        .filter(|c| c.visible && (characters.is_empty() || characters.contains(&c.id)))
        .collect();
    ensure!(selected.len() <= 64, "Review at most 64 characters at once");
    ensure!(
        (end - start + 1) as usize * selected.iter().map(|c| c.joints.len()).sum::<usize>()
            <= 1_000_000,
        "Choose a shorter review range"
    );
    for id in characters {
        ensure!(
            selected.iter().any(|c| c.id == *id),
            "Review character missing or hidden"
        );
    }
    let mut samples: BTreeMap<Id, Vec<Pose>> = selected.iter().map(|c| (c.id, vec![])).collect();
    for frame in start..=end {
        for c in evaluate(p, frame as f64).characters {
            if let Some(poses) = samples.get_mut(&c.id) {
                poses.push(c.pose);
            }
        }
    }
    let mut issues = vec![];
    for c in selected {
        let poses = &samples[&c.id];
        let torso = c
            .joints
            .get(1)
            .and_then(|j| j.parent.map(|i| j.position.distance(c.joints[i].position)))
            .unwrap_or(65.)
            .max(1.);
        for (joint, def) in c.joints.iter().enumerate() {
            if let Some(parent) = def.parent {
                let rest = def.position.distance(c.joints[parent].position);
                if rest < 0.01 {
                    continue;
                }
                if let Some((offset, pose)) = poses.iter().enumerate().find(|(_, pose)| {
                    (pose.joints[joint].distance(pose.joints[parent]) / rest - 1.).abs() > tolerance
                }) {
                    let mut fixed = pose.clone();
                    constrain_bones(c, &mut fixed);
                    issues.push(MotionIssue{kind:"limb_length".into(),character:c.id,joint:Some(def.name.clone()),frame:start+offset as u32,message:format!("Possible {} length drift. Intentional stretch can be kept; the suggestion restores rest lengths.",def.name),commands:if c.locked{vec![]}else{vec![Command::SetBoneConstraints{character:c.id,enabled:true},Command::SetPose{character:c.id,frame:start+offset as u32,pose:fixed,easing:Easing::Smooth}]}});
                }
            }
        }
        for i in 1..poses.len().saturating_sub(1) {
            let speed = poses[i].root.distance(poses[i - 1].root);
            let next = poses[i + 1].root.distance(poses[i].root);
            if speed > torso * 0.6 && speed > next.max(1.) * 3. {
                let mut fixed = poses[i].clone();
                fixed.root = poses[i - 1].root.lerp(poses[i + 1].root, 0.5);
                issues.push(MotionIssue{kind:"position_jump".into(),character:c.id,joint:None,frame:start+i as u32,message:"Possible position jump. Review the before/after pose; a deliberate teleport or impact may be intentional.".into(),commands:if c.locked{vec![]}else{vec![Command::SetPose{character:c.id,frame:start+i as u32,pose:fixed,easing:Easing::Smooth}]}});
                break;
            }
        }
        for name in ["left_foot", "right_foot"] {
            if let Some(joint) = c.joints.iter().position(|j| j.name == name) {
                let points: Vec<_> = poses.iter().map(|p| p.root + p.joints[joint]).collect();
                let floor = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
                let mut count = 0;
                for i in 1..points.len() {
                    let before = points[i - 1];
                    let point = points[i];
                    if point.y >= floor - torso * 0.06
                        && (point.y - before.y).abs() < torso * 0.035
                        && (point.x - before.x).abs() > torso * 0.025
                    {
                        count += 1;
                    } else {
                        count = 0;
                    }
                    if count >= 3 {
                        let first = start + (i - count) as u32;
                        let last = start + i as u32;
                        let target = points[i - count];
                        issues.push(MotionIssue{kind:"foot_slide".into(),character:c.id,joint:Some(name.into()),frame:first,message:format!("Possible {name} slide near the ground. Check whether this is a planted foot or a deliberate skid."),commands:if c.locked{vec![]}else{vec![Command::SetBoneConstraints{character:c.id,enabled:true},Command::SetContact{character:c.id,contact:Contact{start:first,end:last,joint,target}}]}});
                        break;
                    }
                }
            }
        }
        let keys: Vec<_> = c
            .keys
            .iter()
            .filter(|k| k.frame >= start && k.frame <= end)
            .collect();
        if keys.len() >= 4 {
            let mut gaps: Vec<_> = keys.windows(2).map(|k| k[1].frame - k[0].frame).collect();
            gaps.sort_unstable();
            let median = gaps[gaps.len() / 2];
            if median >= 5
                && let Some(pair) = keys
                    .windows(2)
                    .find(|k| k[1].frame - k[0].frame < median / 4)
            {
                issues.push(MotionIssue{kind:"uneven_timing".into(),character:c.id,joint:None,frame:pair[0].frame,message:"A short pose interval sits among much longer intervals. Check the timing preview; impacts may intentionally use short intervals.".into(),commands:if c.locked{vec![]}else{vec![Command::SetKeyEasing{character:c.id,frame:pair[0].frame,easing:Easing::Smooth}]}});
            }
        }
    }
    issues.truncate(64);
    Ok(issues)
}

pub(super) fn foot_lock(
    p: &Project,
    character: Id,
    start: u32,
    end: u32,
    joints: &[String],
    automatic: bool,
) -> Result<StudioPlan> {
    frame_range(p, start, end)?;
    let c = editable_character(p, character)?;
    ensure!(
        !joints.is_empty() && joints.len() <= 4,
        "Select 1–4 feet or end controls"
    );
    let indices: Vec<_> = joints
        .iter()
        .map(|name| {
            c.joints
                .iter()
                .position(|j| j.name == *name)
                .context("Foot control not found")
        })
        .collect::<Result<_>>()?;
    ensure!(
        indices.iter().all(|i| c.joints[*i]
            .parent
            .and_then(|j| c.joints[j].parent)
            .is_some()),
        "Foot locking needs a two-bone limb"
    );
    let mut source = p.clone();
    source
        .characters
        .iter_mut()
        .find(|c| c.id == character)
        .unwrap()
        .preserve_bones = true;
    source.schema = source.schema.max(5);
    let poses: Vec<_> = (start..=end)
        .map(|f| pose_at(&source, character, f))
        .collect::<Result<_>>()?;
    let mut out = StudioPlan::named(if automatic {
        "Detect planted feet"
    } else {
        "Lock planted feet"
    });
    out.commands.push(Command::SetBoneConstraints {
        character,
        enabled: true,
    });
    let mut contacts = vec![];
    for &joint in &indices {
        let points: Vec<_> = poses.iter().map(|p| p.root + p.joints[joint]).collect();
        if automatic {
            let floor = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
            let mid = c.joints[joint].parent.unwrap();
            let root = c.joints[mid].parent.unwrap();
            let length = c.joints[root].position.distance(c.joints[mid].position)
                + c.joints[mid].position.distance(c.joints[joint].position);
            let mut begin = None;
            for i in 0..=points.len() {
                let grounded = i < points.len()
                    && points[i].y >= floor - length * 0.045
                    && (i == 0 || (points[i].y - points[i - 1].y).abs() < length * 0.04);
                if grounded && begin.is_none() {
                    begin = Some(i);
                }
                if !grounded
                    && let Some(first) = begin.take()
                    && i - first >= 3
                {
                    let target = points[first];
                    let reachable = (first..i).all(|k| {
                        (poses[k].root + poses[k].joints[root]).distance(target) < length - 0.01
                    });
                    if reachable {
                        contacts.push(Contact {
                            start: start + first as u32,
                            end: start + i as u32 - 1,
                            joint,
                            target,
                        });
                    }
                }
            }
        } else {
            contacts.push(Contact {
                start,
                end,
                joint,
                target: points[0],
            });
        }
    }
    contacts.retain(|new| {
        !c.contacts
            .iter()
            .any(|old| old.joint == new.joint && old.start <= new.end && old.end >= new.start)
    });
    ensure!(
        !contacts.is_empty(),
        "No new reachable planted-foot ranges found; existing contacts were kept"
    );
    if !automatic {
        for (offset, original) in poses.iter().enumerate() {
            let mut fixed = original.clone();
            for _ in 0..8 {
                for contact in &contacts {
                    let mid = c.joints[contact.joint].parent.unwrap();
                    let root = c.joints[mid].parent.unwrap();
                    let reach = c.joints[root].position.distance(c.joints[mid].position)
                        + c.joints[mid]
                            .position
                            .distance(c.joints[contact.joint].position)
                        - 0.01;
                    let anchor = fixed.root + fixed.joints[root];
                    let d = anchor.distance(contact.target);
                    if d > reach {
                        let v = contact.target - anchor;
                        fixed.root =
                            fixed.root + Point::new(v.x / d * (d - reach), v.y / d * (d - reach));
                    }
                }
            }
            if fixed.root.distance(original.root) > 0.001 {
                out.commands.push(Command::SetPose {
                    character,
                    frame: start + offset as u32,
                    pose: fixed,
                    easing: Easing::Linear,
                });
            }
        }
        out.warnings.push("The body may move to keep the requested planted feet reachable. Review the preview before accepting.".into());
    }
    for contact in contacts {
        out.commands
            .push(Command::SetContact { character, contact });
    }
    Ok(out)
}
