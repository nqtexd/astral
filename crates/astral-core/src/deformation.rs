//! Moving least-squares similarity deformation, cubic curvers and bone correctives.
use crate::*;
pub fn pin_offset(pin: &MeshPin, frame: f64) -> Point {
    let i = pin
        .keys
        .partition_point(|k| k.frame as f64 <= frame)
        .saturating_sub(1);
    let a = &pin.keys[i];
    pin.keys.get(i + 1).map_or(a.offset, |b| {
        a.offset.lerp(
            b.offset,
            ease(
                &Easing::Smooth,
                ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32,
            ),
        )
    })
}
pub(crate) fn similarity(uv: Point, pins: &[MeshPin], frame: f64) -> Point {
    similarity_metric(uv, pins, frame, 1.)
}
fn similarity_metric(uv: Point, pins: &[MeshPin], frame: f64, aspect: f32) -> Point {
    let project = |p: Point| Point::new(p.x * aspect, p.y);
    let restore = |p: Point| Point::new(p.x / aspect, p.y);
    let uv = project(uv);
    let mut weights = Vec::with_capacity(pins.len());
    let mut sum = 0.;
    let mut pc = Point::default();
    let mut qc = Point::default();
    for pin in pins {
        let offset = project(pin_offset(pin, frame));
        let rest = project(pin.rest);
        let d = uv.distance(rest);
        if d < 0.00001 {
            return restore(uv + offset);
        }
        let w = 1. / (d * d);
        sum += w;
        pc = pc + Point::new(rest.x * w, rest.y * w);
        let q = rest + offset;
        qc = qc + Point::new(q.x * w, q.y * w);
        weights.push(w);
    }
    if sum == 0. {
        return restore(uv);
    }
    pc = Point::new(pc.x / sum, pc.y / sum);
    qc = Point::new(qc.x / sum, qc.y / sum);
    let (mut norm, mut dot, mut cross) = (0., 0., 0.);
    for (pin, w) in pins.iter().zip(weights) {
        let a = project(pin.rest) - pc;
        let b = project(pin.rest) + project(pin_offset(pin, frame)) - qc;
        norm += w * (a.x * a.x + a.y * a.y);
        dot += w * (a.x * b.x + a.y * b.y);
        cross += w * (a.x * b.y - a.y * b.x);
    }
    let v = uv - pc;
    if norm < 1e-10 {
        return restore(uv + qc - pc);
    }
    restore(
        qc + Point::new(
            (dot * v.x - cross * v.y) / norm,
            (cross * v.x + dot * v.y) / norm,
        ),
    )
}
pub fn deform_uv(mesh: &DeformMesh, uv: Point, frame: f64, aspect: f32) -> Point {
    let mut q = if mesh.mode == DeformMode::Similarity {
        similarity_metric(uv, &mesh.pins, frame, aspect)
    } else {
        let (mut sum, mut delta) = (0., Point::default());
        let mut exact = None;
        for pin in &mesh.pins {
            let d = uv.distance(pin.rest);
            let offset = pin_offset(pin, frame);
            if d < 0.00001 {
                exact = Some(uv + offset);
                break;
            }
            let w = 1. / (d * d);
            sum += w;
            delta = delta + Point::new(offset.x * w, offset.y * w);
        }
        exact.unwrap_or_else(|| {
            if sum > 0. {
                uv + Point::new(delta.x / sum, delta.y / sum)
            } else {
                uv
            }
        })
    };
    if !mesh.curve.is_empty() {
        let i = mesh
            .curve
            .partition_point(|k| k.frame as f64 <= frame)
            .saturating_sub(1);
        let a = &mesh.curve[i];
        let mut c = a.controls;
        if let Some(b) = mesh.curve.get(i + 1) {
            let t = ease(
                &Easing::Smooth,
                ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32,
            );
            for (control, target) in c.iter_mut().zip(b.controls) {
                *control = control.lerp(target, t);
            }
        }
        let t = q.x.clamp(0., 1.);
        let u = 1. - t;
        let v = Point::new(
            u * u * u * c[0].x
                + 3. * u * u * t * c[1].x
                + 3. * u * t * t * c[2].x
                + t * t * t * c[3].x,
            u * u * u * c[0].y
                + 3. * u * u * t * c[1].y
                + 3. * u * t * t * c[2].y
                + t * t * t * c[3].y,
        );
        let tangent = Point::new(
            3. * u * u * (c[1].x - c[0].x)
                + 6. * u * t * (c[2].x - c[1].x)
                + 3. * t * t * (c[3].x - c[2].x),
            3. * u * u * (c[1].y - c[0].y)
                + 6. * u * t * (c[2].y - c[1].y)
                + 3. * t * t * (c[3].y - c[2].y),
        );
        let length = (tangent.x * aspect).hypot(tangent.y).max(0.00001);
        let normal = Point::new(-tangent.y / length / aspect, tangent.x * aspect / length);
        q = v + Point::new(normal.x * (q.y - 0.5), normal.y * (q.y - 0.5));
    }
    q
}
impl RasterLayer {
    /// Resolve once per layer, shared by viewport, exports and composition review.
    pub fn resolve_correctives(&self, scene: &EvaluatedScene) -> Self {
        let mut layer = self.clone();
        if let Some(mesh) = &mut layer.mesh {
            for pin in &mut mesh.pins {
                if let Some(driver) = &pin.driver
                    && let Some(c) = scene.characters.iter().find(|c| c.id == driver.character)
                    && let Some(parent) = c.parents.get(driver.joint).copied().flatten()
                {
                    let d = c.pose.joints[driver.joint] - c.pose.joints[parent];
                    let mut angle = d.y.atan2(d.x);
                    if let Some(grand) = c.parents[parent] {
                        let p = c.pose.joints[parent] - c.pose.joints[grand];
                        angle -= p.y.atan2(p.x);
                    }
                    let delta = (angle - driver.rest_angle + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    let t = (delta / driver.angle_range).clamp(0., 1.);
                    for key in &mut pin.keys {
                        key.offset =
                            key.offset + Point::new(driver.offset.x * t, driver.offset.y * t);
                    }
                }
            }
        }
        layer
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pin(x: f32, y: f32, qx: f32, qy: f32) -> MeshPin {
        MeshPin {
            id: Id::new_v4(),
            name: "Pin".into(),
            rest: Point::new(x, y),
            keys: vec![PinKey {
                frame: 0,
                offset: Point::new(qx - x, qy - y),
            }],
            driver: None,
        }
    }
    #[test]
    fn similarity_preserves_rigid_rotation_and_scale() {
        let mesh = DeformMesh {
            columns: 8,
            rows: 8,
            pins: vec![
                pin(0., 0., 1., 0.),
                pin(1., 0., 1., 2.),
                pin(0., 1., -1., 0.),
            ],
            mode: DeformMode::Similarity,
            curve: vec![],
        };
        let q = deform_uv(&mesh, Point::new(0.25, 0.4), 0., 1.);
        assert!(q.distance(Point::new(0.2, 0.5)) < 0.0001);
    }
    #[test]
    fn non_square_similarity_preserves_pixel_space_rotation() {
        let mesh = DeformMesh {
            columns: 8,
            rows: 8,
            pins: vec![
                pin(0., 0., 0., 0.),
                pin(1., 0., 0., 2.),
                pin(0., 1., -0.5, 0.),
            ],
            mode: DeformMode::Similarity,
            curve: vec![],
        };
        let q = deform_uv(&mesh, Point::new(0.25, 0.4), 0., 2.);
        assert!(q.distance(Point::new(-0.2, 0.5)) < 0.0001);
    }
    #[test]
    fn identity_curver_and_key_interpolation() {
        let mut mesh = DeformMesh {
            columns: 8,
            rows: 8,
            pins: vec![],
            mode: DeformMode::Similarity,
            curve: vec![CurveKey {
                frame: 0,
                controls: [
                    Point::new(0., 0.5),
                    Point::new(1. / 3., 0.5),
                    Point::new(2. / 3., 0.5),
                    Point::new(1., 0.5),
                ],
            }],
        };
        let q = Point::new(0.4, 0.8);
        assert!(deform_uv(&mesh, q, 0., 2.).distance(q) < 0.0001);
        let mut next = mesh.curve[0].clone();
        next.frame = 10;
        for p in &mut next.controls {
            p.y += 0.2;
        }
        mesh.curve.push(next);
        assert!(deform_uv(&mesh, q, 5., 2.).distance(Point::new(0.4, 0.9)) < 0.0001);
    }
}
