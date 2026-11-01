//! Small, offline supervised motion learner. Multi-output RBF kernel ridge
//! regression fits authored trajectories, not a pretrained image generator.
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MotionModel {
    pub centers: Vec<f64>,
    pub weights: Vec<Vec<f64>>,
    pub first: Vec<f64>,
    pub last: Vec<f64>,
    pub gamma: f64,
    pub regularization: f64,
}
impl MotionModel {
    pub fn train(times: &[f64], samples: &[Vec<f64>], regularization: f64) -> Result<Self> {
        ensure!(
            (3..=64).contains(&times.len()) && times.len() == samples.len(),
            "Learning needs 3–64 authored example frames; two frames only provide tweening"
        );
        let width = samples[0].len();
        ensure!(
            (1..=8194).contains(&width)
                && samples
                    .iter()
                    .all(|v| v.len() == width && v.iter().all(|v| v.is_finite())),
            "Training examples must have matching finite dimensions"
        );
        ensure!(
            times.iter().all(|t| t.is_finite()) && times.windows(2).all(|w| w[0] < w[1]),
            "Training timestamps must be distinct and increasing"
        );
        ensure!(
            regularization.is_finite() && (1e-8..=0.1).contains(&regularization),
            "Smoothing must be 0.00000001–0.1"
        );
        let start = times[0];
        let length = times[times.len() - 1] - start;
        ensure!(length > 1e-9, "Training range is too short");
        let centers: Vec<_> = times.iter().map(|t| (t - start) / length).collect();
        let first = samples[0].clone();
        let last = samples[samples.len() - 1].clone();
        // A moderate bandwidth learns smooth trajectories across neighboring examples.
        let gamma = ((times.len() - 1) as f64 * 0.7).powi(2).clamp(4., 256.);
        let n = times.len();
        let mut matrix = vec![vec![0.; n]; n];
        let mut rhs = vec![vec![0.; width]; n];
        for i in 0..n {
            for j in 0..n {
                matrix[i][j] = (-gamma * (centers[i] - centers[j]).powi(2)).exp();
            }
            matrix[i][i] += regularization;
            for d in 0..width {
                rhs[i][d] = samples[i][d] - (first[d] + (last[d] - first[d]) * centers[i]);
            }
        }
        // Cholesky solve of (K + alpha I) W = residuals; no external runtime.
        let mut l = vec![vec![0.; n]; n];
        for i in 0..n {
            for j in 0..=i {
                let sum: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
                if i == j {
                    let diagonal = matrix[i][i] - sum;
                    ensure!(
                        diagonal > 1e-14,
                        "Ill-conditioned training examples; increase smoothing"
                    );
                    l[i][j] = diagonal.sqrt();
                } else {
                    l[i][j] = (matrix[i][j] - sum) / l[j][j];
                }
            }
        }
        let mut weights = vec![vec![0.; width]; n];
        for d in 0..width {
            let mut y = vec![0.; n];
            for i in 0..n {
                y[i] = (rhs[i][d] - (0..i).map(|k| l[i][k] * y[k]).sum::<f64>()) / l[i][i];
            }
            for i in (0..n).rev() {
                weights[i][d] =
                    (y[i] - (i + 1..n).map(|k| l[k][i] * weights[k][d]).sum::<f64>()) / l[i][i];
            }
        }
        let model = Self {
            centers,
            weights,
            first,
            last,
            gamma,
            regularization,
        };
        model.validate(width)?;
        Ok(model)
    }
    pub fn validate(&self, width: usize) -> Result<()> {
        ensure!(
            (3..=64).contains(&self.centers.len())
                && self.weights.len() == self.centers.len()
                && self.first.len() == width
                && self.last.len() == width,
            "Invalid learned model shape"
        );
        ensure!(
            self.centers.first() == Some(&0.)
                && self.centers.last() == Some(&1.)
                && self.centers.windows(2).all(|w| w[0] < w[1]),
            "Invalid model timestamps"
        );
        ensure!(
            self.gamma.is_finite()
                && (4.0..=256.).contains(&self.gamma)
                && self.regularization.is_finite()
                && (1e-8..=0.1).contains(&self.regularization),
            "Invalid model settings"
        );
        ensure!(
            self.centers
                .iter()
                .chain(&self.first)
                .chain(&self.last)
                .all(|v| v.is_finite() && v.abs() < 1e9)
                && self
                    .weights
                    .iter()
                    .all(|w| w.len() == width && w.iter().all(|v| v.is_finite() && v.abs() < 1e14)),
            "Invalid model coefficients"
        );
        Ok(())
    }
    fn residual(&self, t: f64) -> Vec<f64> {
        let mut out = vec![0.; self.first.len()];
        for (x, w) in self.centers.iter().zip(&self.weights) {
            let k = (-self.gamma * (t - x).powi(2)).exp();
            for (v, w) in out.iter_mut().zip(w) {
                *v += k * w;
            }
        }
        out
    }
    pub fn predict(&self, phase: f64) -> Vec<f64> {
        let t = phase.clamp(0., 1.);
        if t == 0. {
            return self.first.clone();
        }
        if t == 1. {
            return self.last.clone();
        }
        let r = self.residual(t);
        let a = self.residual(0.);
        let b = self.residual(1.);
        self.first
            .iter()
            .zip(&self.last)
            .enumerate()
            .map(|(i, (a0, b0))| a0 + (b0 - a0) * t + r[i] - a[i] * (1. - t) - b[i] * t)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn learns_a_curve_at_unseen_frames_better_than_a_straight_tween() {
        let t = vec![0., 0.25, 0.5, 0.75, 1.];
        let samples = t
            .iter()
            .map(|t| vec![(t * std::f64::consts::PI).sin()])
            .collect::<Vec<_>>();
        let m = MotionModel::train(&t, &samples, 1e-5).unwrap();
        let mut learned = 0.;
        let mut tween = 0.;
        for t in [0.125, 0.375, 0.625, 0.875] {
            let expected = (t * std::f64::consts::PI).sin();
            let actual = m.predict(t)[0];
            learned += (actual - expected).powi(2);
            let i = (t * 4.).floor() as usize;
            let linear = samples[i][0] + (samples[i + 1][0] - samples[i][0]) * (t * 4. - i as f64);
            tween += (linear - expected).powi(2);
        }
        assert!(
            learned < tween,
            "held-out learned error {learned} vs tween {tween}"
        );
        assert_eq!(m.predict(0.), samples[0]);
        assert_eq!(m.predict(1.), samples[4]);
    }
    #[test]
    fn rejects_duplicates_mismatches_and_two_frame_training() {
        assert!(MotionModel::train(&[0., 1.], &[vec![0.], vec![1.]], 1e-4).is_err());
        assert!(MotionModel::train(&[0., 0., 1.], &[vec![0.], vec![1.], vec![2.]], 1e-4).is_err());
        assert!(
            MotionModel::train(&[0., 0.5, 1.], &[vec![0.], vec![f64::NAN], vec![2.]], 1e-4)
                .is_err()
        );
    }
}
