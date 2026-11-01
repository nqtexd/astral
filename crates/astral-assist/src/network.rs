use anyhow::{Result, ensure};
use astral_core::*;
use serde::{Deserialize, Serialize};

pub const INPUT: usize = 33;
pub const HIDDEN: usize = 64;
pub const OUTPUT: usize = 22;
#[derive(Clone, Serialize, Deserialize)]
pub struct PoseNetwork {
    pub version: u32,
    pub training_samples: usize,
    pub seed: u64,
    pub w1: Vec<f32>,
    pub b1: Vec<f32>,
    pub w2: Vec<f32>,
    pub b2: Vec<f32>,
}
fn rest() -> Vec<f32> {
    stick_character("Template".into(), Point::default(), [0, 0, 0, 255])
        .joints
        .iter()
        .flat_map(|j| [j.position.x / 100., j.position.y / 100.])
        .collect()
}
impl PoseNetwork {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && self.training_samples > 0
                && self.w1.len() == INPUT * HIDDEN
                && self.b1.len() == HIDDEN
                && self.w2.len() == HIDDEN * OUTPUT
                && self.b2.len() == OUTPUT,
            "Invalid pose model dimensions"
        );
        ensure!(
            self.w1
                .iter()
                .chain(&self.b1)
                .chain(&self.w2)
                .chain(&self.b2)
                .all(|v| v.is_finite() && v.abs() < 100.),
            "Invalid pose model coefficients"
        );
        Ok(())
    }
    pub fn bundled() -> Result<Self> {
        let model: Self =
            serde_json::from_slice(include_bytes!("../../../models/pose-prior-v1.json"))?;
        model.validate()?;
        Ok(model)
    }
    fn forward(&self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let h: Vec<_> = (0..HIDDEN)
            .map(|j| {
                (self.b1[j]
                    + (0..INPUT)
                        .map(|i| self.w1[j * INPUT + i] * x[i])
                        .sum::<f32>())
                .tanh()
            })
            .collect();
        let y = (0..OUTPUT)
            .map(|j| {
                self.b2[j]
                    + (0..HIDDEN)
                        .map(|i| self.w2[j * HIDDEN + i] * h[i])
                        .sum::<f32>()
            })
            .collect();
        (h, y)
    }
    pub fn predict(&self, observed: &[Point], known: &[bool]) -> Result<Vec<Point>> {
        self.validate()?;
        ensure!(
            observed.len() == 11 && known.len() == 11 && observed.iter().all(|p| p.finite()),
            "Pose model needs eleven finite canonical joint positions and masks"
        );
        let reference = rest();
        let mut x = vec![0.; INPUT];
        for j in 0..11 {
            if known[j] {
                x[j * 2] = observed[j].x - reference[j * 2];
                x[j * 2 + 1] = observed[j].y - reference[j * 2 + 1];
            }
            x[22 + j] = if known[j] { 1. } else { 0. };
        }
        let (_, y) = self.forward(&x);
        Ok((0..11)
            .map(|j| {
                if known[j] {
                    observed[j]
                } else {
                    Point::new(
                        (reference[j * 2] + y[j * 2]).clamp(-3., 3.),
                        (reference[j * 2 + 1] + y[j * 2 + 1]).clamp(-3., 3.),
                    )
                }
            })
            .collect())
    }
}
pub fn complete_pose(
    model: &PoseNetwork,
    c: &Character,
    pose: &Pose,
    known_names: &[String],
) -> Result<Pose> {
    ensure!(
        c.joints.len() == 11 && pose.joints.len() == 11,
        "Pose model supports canonical eleven-joint stick rigs"
    );
    let reference = stick_character("Template".into(), Point::default(), [0, 0, 0, 255]);
    ensure!(
        c.joints
            .iter()
            .zip(&reference.joints)
            .all(|(a, b)| a.name == b.name && a.parent == b.parent),
        "Map the rig to canonical named controls before completion"
    );
    ensure!(
        known_names
            .iter()
            .all(|name| c.joints.iter().any(|j| &j.name == name)),
        "Unknown observed joint name"
    );
    let known: Vec<_> = c
        .joints
        .iter()
        .map(|j| j.parent.is_none() || known_names.contains(&j.name))
        .collect();
    ensure!(
        known.iter().filter(|k| **k).count() >= 2,
        "Mark at least two joints as observed"
    );
    let origin = pose.joints[0];
    let scale = c.joints[1].position.distance(c.joints[0].position).max(1.) / 0.65;
    let normalized: Vec<_> = pose
        .joints
        .iter()
        .map(|p| Point::new((p.x - origin.x) / scale, (p.y - origin.y) / scale))
        .collect();
    let predicted = model.predict(&normalized, &known)?;
    let mut points: Vec<_> = predicted
        .iter()
        .map(|p| Point::new(p.x * scale + origin.x, p.y * scale + origin.y))
        .collect();
    // Positional bone constraints move only inferred joints, never authored landmarks.
    for _ in 0..32 {
        for (i, j) in c.joints.iter().enumerate() {
            if let Some(parent) = j.parent {
                let length = j.position.distance(c.joints[parent].position);
                let delta = points[i] - points[parent];
                let d = delta.distance(Point::default());
                if d > 0.001 {
                    let error = (d - length) / d;
                    let correction = Point::new(delta.x * error, delta.y * error);
                    match (known[parent], known[i]) {
                        (true, false) => points[i] = points[i] - correction,
                        (false, true) => points[parent] = points[parent] + correction,
                        (false, false) => {
                            let half = Point::new(correction.x * 0.5, correction.y * 0.5);
                            points[i] = points[i] - half;
                            points[parent] = points[parent] + half;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    for i in 0..11 {
        if known[i] {
            points[i] = pose.joints[i];
        }
    }
    Ok(Pose {
        root: pose.root,
        joints: points,
    })
}

struct Random(u64);
impl Random {
    fn value(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / 16_777_216.
    }
}
fn sample(seed: u64) -> (Vec<f32>, Vec<f32>, Vec<bool>) {
    let mut rng = Random(seed.max(1));
    let mut p = Project::new("Training", 100, 100, 30, 120);
    let mut c = stick_character("Actor".into(), Point::default(), [0, 0, 0, 255]);
    let proportions = 0.8 + rng.value() * 0.4;
    for j in &mut c.joints {
        j.position.x *= proportions;
    }
    for j in &mut c.keys[0].pose.joints {
        j.x *= proportions;
    }
    let id = c.id;
    p.characters.push(c);
    let kind = ActionKind::ALL[(rng.value() * 8.) as usize % 8];
    p.actions.push(ActionClip {
        id: Id::nil(),
        character: id,
        kind,
        start: 0,
        end: 119,
        intensity: 0.4 + rng.value() * 1.4,
        distance: 0.,
        direction: if rng.value() > 0.5 { 1. } else { -1. },
        markers: Default::default(),
    });
    let frame = rng.value() * 119.;
    let scene = evaluate(&p, frame as f64);
    let pose = &scene.characters[0].pose;
    let angle = (rng.value() - 0.5) * 0.6;
    let (sin, cos) = angle.sin_cos();
    let target: Vec<_> = pose
        .joints
        .iter()
        .flat_map(|q| {
            [
                (q.x * cos - q.y * sin) / 100.,
                (q.y * cos + q.x * sin) / 100.,
            ]
        })
        .collect();
    let reference = rest();
    let mut x = vec![0.; INPUT];
    let mut known = vec![false; 11];
    for j in 0..11 {
        known[j] = j <= 1 || rng.value() > 0.5;
        if known[j] {
            x[j * 2] = target[j * 2] - reference[j * 2];
            x[j * 2 + 1] = target[j * 2 + 1] - reference[j * 2 + 1];
        }
        x[22 + j] = if known[j] { 1. } else { 0. };
    }
    (
        x,
        target.iter().zip(reference).map(|(a, b)| a - b).collect(),
        known,
    )
}
/// Deterministic Adam training on independent synthetic character/trajectory seeds.
pub fn train(samples: usize, epochs: usize, seed: u64) -> Result<(PoseNetwork, serde_json::Value)> {
    train_with_progress(samples, epochs, seed, |_| Ok(()))
}
pub fn evaluate_model(model: &PoseNetwork, samples: usize, seed: u64) -> Result<serde_json::Value> {
    model.validate()?;
    ensure!(
        (1..=8192).contains(&samples),
        "Evaluation sample budget is invalid"
    );
    let mut error = 0.;
    let mut baseline = 0.;
    let mut count = 0;
    for i in 0..samples {
        let (x, target, known) = sample(seed.wrapping_add(i as u64 * 104729 + 1));
        let (_, predicted) = model.forward(&x);
        for j in 0..OUTPUT {
            if !known[j / 2] {
                error += (predicted[j] - target[j]).powi(2) as f64;
                baseline += target[j].powi(2) as f64;
                count += 1;
            }
        }
    }
    Ok(
        serde_json::json!({"missing_joint_mse":error/count as f64,"rest_pose_baseline_mse":baseline/count as f64,"samples":samples}),
    )
}
pub fn train_with_progress(
    samples: usize,
    epochs: usize,
    seed: u64,
    mut progress: impl FnMut(f32) -> Result<()>,
) -> Result<(PoseNetwork, serde_json::Value)> {
    ensure!(
        (512..=32768).contains(&samples) && (1..=300).contains(&epochs),
        "Training budget is outside supported bounds"
    );
    let data: Vec<_> = (0..samples)
        .map(|i| sample(seed.wrapping_add(i as u64 * 7919 + 1)))
        .collect();
    let held: Vec<_> = (0..1024)
        .map(|i| sample(seed.wrapping_add(9_000_000_000 + i as u64 * 104729)))
        .collect();
    let mut rng = Random(seed.max(1));
    let mut model = PoseNetwork {
        version: 1,
        training_samples: samples,
        seed,
        w1: (0..INPUT * HIDDEN)
            .map(|_| (rng.value() - 0.5) * 0.3)
            .collect(),
        b1: vec![0.; HIDDEN],
        w2: (0..HIDDEN * OUTPUT)
            .map(|_| (rng.value() - 0.5) * 0.1)
            .collect(),
        b2: vec![0.; OUTPUT],
    };
    let count = INPUT * HIDDEN + HIDDEN + HIDDEN * OUTPUT + OUTPUT;
    let mut first = vec![0f32; count];
    let mut second = vec![0f32; count];
    let mut step = 0i32;
    for epoch in 0..epochs {
        progress(epoch as f32 / epochs as f32)?;
        for batch in data.chunks(64) {
            let mut grad = vec![0.; count];
            for (x, target, known) in batch {
                let (h, y) = model.forward(x);
                let mut dy = [0.; OUTPUT];
                let missing = known.iter().filter(|k| !**k).count().max(1) as f32 * 2.;
                for j in 0..OUTPUT {
                    let weight = if known[j / 2] { 0.05 } else { 1. };
                    dy[j] = 2. * (y[j] - target[j]) * weight / missing;
                    let offset = INPUT * HIDDEN + HIDDEN;
                    for i in 0..HIDDEN {
                        grad[offset + j * HIDDEN + i] += dy[j] * h[i];
                    }
                    grad[count - OUTPUT + j] += dy[j];
                }
                for j in 0..HIDDEN {
                    let dh = (0..OUTPUT)
                        .map(|i| model.w2[i * HIDDEN + j] * dy[i])
                        .sum::<f32>()
                        * (1. - h[j] * h[j]);
                    for i in 0..INPUT {
                        grad[j * INPUT + i] += dh * x[i];
                    }
                    grad[INPUT * HIDDEN + j] += dh;
                }
            }
            step += 1;
            let rate = 0.001;
            let denominator = batch.len() as f32;
            for (i, weight) in model
                .w1
                .iter_mut()
                .chain(&mut model.b1)
                .chain(&mut model.w2)
                .chain(&mut model.b2)
                .enumerate()
            {
                let g = (grad[i] / denominator).clamp(-1., 1.);
                first[i] = 0.9 * first[i] + 0.1 * g;
                second[i] = 0.999 * second[i] + 0.001 * g * g;
                let m = first[i] / (1. - 0.9f32.powi(step));
                let v = second[i] / (1. - 0.999f32.powi(step));
                *weight -= rate * m / (v.sqrt() + 1e-8);
            }
        }
    }
    let mut error = 0.;
    let mut baseline = 0.;
    let mut n = 0;
    for (x, target, known) in &held {
        let (_, y) = model.forward(x);
        for j in 0..OUTPUT {
            if !known[j / 2] {
                error += (y[j] - target[j]).powi(2) as f64;
                baseline += target[j].powi(2) as f64;
                n += 1;
            }
        }
    }
    model.validate()?;
    let mse = error / n as f64;
    let baseline_mse = baseline / n as f64;
    Ok((
        model,
        serde_json::json!({"model":"masked_pose_mlp_v1","architecture":[INPUT,HIDDEN,OUTPUT],"parameters":count,"training_samples":samples,"epochs":epochs,"seed":seed,"held_out_samples":held.len(),"split":"disjoint_whole_trajectory_seeds","missing_joint_mse":mse,"rest_pose_baseline_mse":baseline_mse,"relative_mse_reduction":1.-mse/baseline_mse,"observed_joint_drift":0,"training_data":"procedurally generated Astral action poses; no external images","scope":"canonical 2D stick-rig missing-joint completion; not raster image generation"}),
    ))
}
