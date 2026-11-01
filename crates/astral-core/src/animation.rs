use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluatedCharacter {
    pub id: Id,
    pub name: String,
    pub pose: Pose,
    pub color: Color,
    pub thickness: f32,
    pub parents: Vec<Option<usize>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluatedScene {
    pub frame: f64,
    pub characters: Vec<EvaluatedCharacter>,
    pub center: Point,
    pub zoom: f32,
}

pub fn ease(e: &Easing, t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    match e {
        Easing::Linear | Easing::Flow => t,
        Easing::Hold => 0.,
        Easing::Smooth => t * t * (3. - 2. * t),
        Easing::Bezier { x1, y1, x2, y2 } => {
            let curve = |a: f32, b: f32, u: f32| {
                3. * (1. - u).powi(2) * u * a + 3. * (1. - u) * u * u * b + u.powi(3)
            };
            let (mut lo, mut hi) = (0., 1.);
            for _ in 0..20 {
                let m = (lo + hi) * 0.5;
                if curve(*x1, *x2, m) < t {
                    lo = m
                } else {
                    hi = m
                }
            }
            curve(*y1, *y2, (lo + hi) * 0.5)
        }
    }
}

pub fn evaluate(project: &Project, frame: f64) -> EvaluatedScene {
    let frame = frame.clamp(0., (project.duration - 1) as f64);
    let mut center = Point::new(project.width as f32 / 2., project.height as f32 / 2.);
    let mut zoom = 1.;
    if !project.camera.is_empty() {
        let i = project
            .camera
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let a = &project.camera[i];
        center = a.center;
        zoom = a.zoom;
        if let Some(b) = project.camera.get(i + 1) {
            let t = ((frame - a.frame as f64) / (b.frame - a.frame) as f64).clamp(0., 1.) as f32;
            center = a.center.lerp(b.center, t);
            zoom = a.zoom + (b.zoom - a.zoom) * t;
        }
    }
    for e in &project.effects {
        if e.kind == EffectKind::CameraShake && frame >= e.start as f64 && frame <= e.end as f64 {
            let t = (frame - e.start as f64) as f32;
            center.x += (t * 2.31 + e.seed as f32).sin() * e.intensity * 8.;
            center.y += (t * 3.17).cos() * e.intensity * 8.;
        }
    }
    let characters = project
        .characters
        .iter()
        .filter(|c| c.visible)
        .map(|c| {
            let i = c
                .keys
                .partition_point(|k| k.frame as f64 <= frame)
                .saturating_sub(1);
            let a = &c.keys[i];
            let mut pose = a.pose.clone();
            if let Some(b) = c.keys.get(i + 1) {
                let t = ease(
                    &a.easing,
                    ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32,
                );
                if matches!(a.easing, Easing::Flow) {
                    pose = flow_pose(c, i, t);
                } else {
                    pose.root = a.pose.root.lerp(b.pose.root, t);
                    for (j, p) in pose.joints.iter_mut().enumerate() {
                        *p = a.pose.joints[j].lerp(b.pose.joints[j], t);
                    }
                }
            }
            if let Some(learned) = sample_learned(project, c, frame) {
                pose = learned;
            }
            for clip in project.actions.iter().filter(|v| v.character == c.id) {
                if frame < clip.start as f64 {
                    continue;
                }
                let t = ((frame - clip.start as f64) / (clip.end - clip.start) as f64).clamp(0., 1.)
                    as f32;
                // Translation persists after the action; articulated motion only runs in its range.
                pose.root.x += clip.distance * clip.direction * t;
                if frame <= clip.end as f64
                    || matches!(clip.kind, ActionKind::Fall | ActionKind::Turn)
                {
                    animate_action(c, &mut pose, clip, t, project.fps);
                }
            }
            if c.secondary_motion != 0. {
                let t = frame as f32 / project.fps as f32;
                for (j, p) in pose.joints.iter_mut().enumerate() {
                    if j > 0 {
                        p.x +=
                            (t * 8. - j as f32 * 0.4).sin() * c.secondary_motion * (j as f32 / 10.);
                    }
                }
            }
            if c.preserve_bones {
                crate::studio::constrain_bones(c, &mut pose);
            }
            for contact in c
                .contacts
                .iter()
                .filter(|v| frame >= v.start as f64 && frame <= v.end as f64)
            {
                let target = contact.target - pose.root;
                let end = contact.joint;
                if let Some(mid) = c.joints[end].parent {
                    if let Some(root) = c.joints[mid].parent {
                        let origin = pose.joints[root];
                        let l1 = pose.joints[root].distance(pose.joints[mid]);
                        let l2 = pose.joints[mid].distance(pose.joints[end]);
                        let bend = if pose.joints[mid].x >= origin.x {
                            1.
                        } else {
                            -1.
                        };
                        let (m, e) = solve_two_bone(origin, target, l1, l2, bend);
                        pose.joints[mid] = m;
                        pose.joints[end] = e;
                    } else {
                        pose.joints[end] = target
                    }
                } else {
                    pose.joints[end] = target
                }
            }
            EvaluatedCharacter {
                id: c.id,
                name: c.name.clone(),
                pose,
                color: c.color,
                thickness: c.thickness,
                parents: c.joints.iter().map(|j| j.parent).collect(),
            }
        })
        .collect();
    EvaluatedScene {
        frame,
        characters,
        center,
        zoom,
    }
}

fn animate_action(c: &Character, p: &mut Pose, a: &ActionClip, t: f32, fps: u32) {
    let tau = std::f32::consts::TAU;
    let duration = (a.end - a.start) as f32 / fps as f32;
    let cycle = t * duration * if a.kind == ActionKind::Run { 3.0 } else { 1.8 } * tau;
    let v = a.intensity;
    let dir = a.direction;
    let mut offset = |name: &str, x: f32, y: f32| {
        if let Some(i) = c.joints.iter().position(|j| j.name == name) {
            p.joints[i].x += x * v * dir;
            p.joints[i].y += y * v;
        }
    };
    match a.kind {
        ActionKind::Idle => {
            offset("chest", 0., (cycle * 0.4).sin() * 2.);
            offset("head", 0., (cycle * 0.4).sin() * 3.);
        }
        ActionKind::Walk | ActionKind::Run => {
            crate::locomotion::gait(
                c,
                p,
                a.distance * dir * t,
                a.distance * dir,
                v,
                a.kind == ActionKind::Run,
            );
        }
        ActionKind::Jump => {
            p.root.y -= (t * std::f32::consts::PI).sin() * 135. * v;
            offset("left_hand", -10., -40. * (t * std::f32::consts::PI).sin());
            offset("right_hand", 10., -40. * (t * std::f32::consts::PI).sin());
        }
        ActionKind::Turn => {
            for j in &mut p.joints {
                j.x *= (t * std::f32::consts::PI).cos();
            }
        }
        ActionKind::Strike => {
            let hit = (t * std::f32::consts::PI).sin().powf(0.6);
            offset("right_hand", hit * 95., -hit * 60.);
            offset("right_elbow", hit * 40., -hit * 20.);
            offset("chest", hit * 14., 0.);
        }
        ActionKind::Recoil => {
            let hit = (t * std::f32::consts::PI).sin();
            offset("head", -hit * 38., hit * 8.);
            offset("chest", -hit * 22., 0.);
            p.root.x -= hit * 20. * v * dir;
        }
        ActionKind::Fall => {
            let angle = t * std::f32::consts::FRAC_PI_2 * dir;
            for j in &mut p.joints {
                let x = j.x;
                let y = j.y;
                j.x = x * angle.cos() - y * angle.sin();
                j.y = x * angle.sin() + y * angle.cos();
            }
            p.root.y += t * 55. * v;
        }
    }
}

pub fn solve_two_bone(root: Point, target: Point, l1: f32, l2: f32, bend: f32) -> (Point, Point) {
    let d = root
        .distance(target)
        .clamp(0.001, (l1 + l2 - 0.001).max(0.001))
        .max((l1 - l2).abs() + 0.001);
    let angle = (target.y - root.y).atan2(target.x - root.x);
    let theta = ((l1 * l1 + d * d - l2 * l2) / (2. * l1.max(0.001) * d))
        .clamp(-1., 1.)
        .acos()
        * bend.signum();
    (
        Point::new(
            root.x + l1 * (angle + theta).cos(),
            root.y + l1 * (angle + theta).sin(),
        ),
        Point::new(root.x + d * angle.cos(), root.y + d * angle.sin()),
    )
}

// Shape-preserving cubic tangents avoid the zero-speed pause at every eased key.
fn flow_scalar(
    keys: &[PoseKey],
    i: usize,
    t: f32,
    value: impl Fn(&PoseKey) -> f32,
    angular: bool,
) -> f32 {
    let a = &keys[i];
    let b = &keys[i + 1];
    let va = value(a);
    let unwrap = |x: f32, center: f32| {
        if angular {
            center + (x - center + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI
        } else {
            x
        }
    };
    let vb = unwrap(value(b), va);
    let dt = (b.frame - a.frame) as f32;
    let slope = (vb - va) / dt;
    let tangent = |left: f32, right: f32, lh: f32, rh: f32| {
        if left * right <= 0. {
            0.
        } else {
            let w1 = 2. * rh + lh;
            let w2 = rh + 2. * lh;
            (w1 + w2) / (w1 / left + w2 / right)
        }
    };
    let m0 = if i > 0 && matches!(keys[i - 1].easing, Easing::Flow) {
        let prev = &keys[i - 1];
        let h = (a.frame - prev.frame) as f32;
        tangent((va - unwrap(value(prev), va)) / h, slope, h, dt)
    } else {
        0.
    };
    let m1 = if i + 2 < keys.len() && matches!(b.easing, Easing::Flow) {
        let next = &keys[i + 2];
        let h = (next.frame - b.frame) as f32;
        tangent(slope, (unwrap(value(next), vb) - vb) / h, dt, h)
    } else {
        0.
    };
    let t2 = t * t;
    let t3 = t2 * t;
    ((2. * t3 - 3. * t2 + 1.) * va
        + (t3 - 2. * t2 + t) * dt * m0
        + (-2. * t3 + 3. * t2) * vb
        + (t3 - t2) * dt * m1)
        .clamp(va.min(vb), va.max(vb))
}
fn flow_pose(c: &Character, i: usize, t: f32) -> Pose {
    let mut pose = c.keys[i].pose.clone();
    pose.root = Point::new(
        flow_scalar(&c.keys, i, t, |k| k.pose.root.x, false),
        flow_scalar(&c.keys, i, t, |k| k.pose.root.y, false),
    );
    let mut angles = vec![0.; c.joints.len()];
    for j in 0..c.joints.len() {
        if let Some(parent) = c.joints[j].parent {
            let angle = |k: &PoseKey| {
                let v = k.pose.joints[j] - k.pose.joints[parent];
                let mut a = v.y.atan2(v.x);
                if let Some(grand) = c.joints[parent].parent {
                    let p = k.pose.joints[parent] - k.pose.joints[grand];
                    a -= p.y.atan2(p.x);
                }
                a
            };
            let local = flow_scalar(&c.keys, i, t, angle, true);
            angles[j] = angles[parent] + local;
            let length = flow_scalar(
                &c.keys,
                i,
                t,
                |k| k.pose.joints[j].distance(k.pose.joints[parent]),
                false,
            );
            pose.joints[j] = pose.joints[parent]
                + Point::new(angles[j].cos() * length, angles[j].sin() * length);
        } else {
            pose.joints[j] = Point::new(
                flow_scalar(&c.keys, i, t, |k| k.pose.joints[j].x, false),
                flow_scalar(&c.keys, i, t, |k| k.pose.joints[j].y, false),
            );
        }
    }
    pose
}
