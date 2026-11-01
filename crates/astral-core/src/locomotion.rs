//! Distance-driven gait: stance feet cancel root travel, swing feet clear the floor.
use crate::*;
/// Match translation to rig size and clip duration instead of defaulting runs to zero travel.
pub fn natural_distance(c: &Character, kind: ActionKind, start: u32, end: u32, fps: u32) -> f32 {
    let leg = c
        .joints
        .iter()
        .find(|j| j.name == "right_foot")
        .map_or(94., |j| j.position.y.abs())
        .max(10.);
    let speed = match kind {
        ActionKind::Run => 3.0,
        ActionKind::Walk => 1.2,
        _ => 0.,
    };
    leg * speed * end.saturating_sub(start) as f32 / fps.max(1) as f32
}
pub fn gait(c: &Character, p: &mut Pose, travel: f32, total: f32, intensity: f32, run: bool) {
    if total.abs() < 0.01 {
        return;
    }
    let direction = total.signum();
    let rest = p.clone();
    let leg = c
        .joints
        .iter()
        .find(|j| j.name == "right_foot")
        .map_or(94., |j| j.position.y.abs())
        .max(10.);
    let stride = leg * if run { 0.9 } else { 0.60 };
    let phase = travel.abs() / (2. * stride);
    let stance = if run { 0.40 } else { 0.60 };
    let blend = (travel.abs() / (stride * 0.25)).clamp(0., 1.);
    let bob = (phase * std::f32::consts::TAU * 2.).sin().abs()
        * leg
        * if run { 0.06 } else { 0.025 }
        * intensity;
    let compression = leg * 0.10;
    p.root.y += (compression - bob) * blend;
    for (foot, offset) in [("left_foot", 0.), ("right_foot", 0.5)] {
        let Some(i) = c.joints.iter().position(|j| j.name == foot) else {
            continue;
        };
        let q = (phase + offset).rem_euclid(1.);
        let (x, lift) = if q < stance {
            (stride * stance - 2. * stride * q, 0.)
        } else {
            let t = (q - stance) / (1. - stance);
            (
                -stride * stance + 2. * stride * stance * ease(&Easing::Smooth, t),
                (t * std::f32::consts::PI).sin() * leg * if run { 0.38 } else { 0.20 },
            )
        };
        let local = Point::new(
            direction * x,
            c.joints[i].position.y - compression - lift + bob,
        );
        let target = p.root + rest.joints[i].lerp(local, blend);
        let _ = crate::staging::aim_limb(c, p, foot, target, -direction);
    }
    // The torso leans into a run; counter-swing targets follow the moved shoulders.
    let angle = direction * if run { 0.20 } else { 0.05 } * blend;
    for name in [
        "chest",
        "head",
        "left_elbow",
        "left_hand",
        "right_elbow",
        "right_hand",
    ] {
        if let Some(i) = c.joints.iter().position(|j| j.name == name) {
            let v = p.joints[i];
            p.joints[i] = Point::new(
                v.x * angle.cos() - v.y * angle.sin(),
                v.x * angle.sin() + v.y * angle.cos(),
            );
        }
    }
    let shoulder_shift = c
        .joints
        .iter()
        .position(|j| j.name == "chest")
        .map_or(Point::default(), |i| p.joints[i] - rest.joints[i]);
    // Counter-swing, articulated elbows, and a small forward torso inclination.
    let swing = (phase * std::f32::consts::TAU).sin() * blend;
    for (name, sign) in [("left_hand", 1.), ("right_hand", -1.)] {
        if let Some(i) = c.joints.iter().position(|j| j.name == name) {
            let target = p.root
                + rest.joints[i]
                + shoulder_shift
                + Point::new(
                    sign * swing * leg * 0.35 * direction,
                    -leg * if run { 0.4 } else { 0.08 } * blend,
                );
            let mid = c.joints[i].parent.unwrap();
            let anchor = c.joints[mid].parent.unwrap();
            let before = p.clone();
            let hand = before.joints[i] - before.joints[anchor];
            let elbow = before.joints[mid] - before.joints[anchor];
            let bend = (hand.x * elbow.y - hand.y * elbow.x).signum();
            let _ = crate::staging::aim_limb(
                c,
                p,
                name,
                target,
                if bend == 0. { direction } else { bend },
            );
            // At zero travel, keep the original arm exactly. Blend angles rather
            // than points so neither the elbow nor the forearm changes length.
            let solved = p.clone();
            for joint in [mid, i] {
                let parent = c.joints[joint].parent.unwrap();
                let a = before.joints[joint] - before.joints[parent];
                let b = solved.joints[joint] - solved.joints[parent];
                let aa = a.y.atan2(a.x);
                let delta = (b.y.atan2(b.x) - aa + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                let angle = aa + delta * blend;
                let length = a.distance(Point::default());
                p.joints[joint] =
                    p.joints[parent] + Point::new(angle.cos() * length, angle.sin() * length);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gait_start_does_not_snap_and_natural_speed_respects_fps() {
        let c = stick_character("Runner".into(), Point::new(200., 520.), [255; 4]);
        let mut pose = c.keys[0].pose.clone();
        gait(&c, &mut pose, 0., 600., 1., true);
        assert_eq!(pose.root, c.keys[0].pose.root);
        for i in [3, 4, 5, 6] {
            assert!(
                pose.joints[i].distance(c.keys[0].pose.joints[i]) < 0.001,
                "arm snapped at startup"
            );
        }
        let a = natural_distance(&c, ActionKind::Run, 0, 120, 60);
        let b = natural_distance(&c, ActionKind::Run, 0, 48, 24);
        assert!((a - b).abs() < 0.001);
        assert!(a / 2. > 250.);
        assert!(natural_distance(&c, ActionKind::Walk, 0, 120, 60) < a / 2.);
    }
    #[test]
    fn stance_is_planted_and_legs_keep_length() {
        let c = stick_character("Walker".into(), Point::new(200., 300.), [255; 4]);
        let mut previous = None;
        // Left foot is in full stance after the short startup blend.
        for distance in 20..55 {
            let mut pose = c.keys[0].pose.clone();
            pose.root.x += distance as f32;
            gait(&c, &mut pose, distance as f32, 240., 1., false);
            let foot = pose.root + pose.joints[8];
            if let Some(last) = previous {
                assert!(
                    foot.distance(last) < 0.02,
                    "stance slid: {:?} {:?}",
                    foot,
                    last
                );
            }
            previous = Some(foot);
            for i in [7, 8, 9, 10] {
                let parent = c.joints[i].parent.unwrap();
                assert!(
                    (pose.joints[i].distance(pose.joints[parent])
                        - c.joints[i].position.distance(c.joints[parent].position))
                    .abs()
                        < 0.01
                );
            }
        }
    }
}
