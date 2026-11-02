use crate::*;
use anyhow::{Context, Result, ensure};
use geo::algorithm::bool_ops::{FillRule, OpType};
use geo::orient::Direction;
use geo::{BooleanOps, LineString, MultiPolygon, Orient, Polygon};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOperation {
    Union,
    Subtract,
    Intersect,
    Xor,
}
pub fn contours(g: &Graphic) -> Result<Vec<Vec<Point>>> {
    if !g.contours.is_empty() {
        return Ok(g.contours.clone());
    }
    let w = g.size.x / 2.;
    let h = g.size.y / 2.;
    let ellipse = |scale: f32, reverse: bool| {
        let mut v: Vec<_> = (0..64)
            .map(|i| {
                let a = i as f32 / 64. * std::f32::consts::TAU;
                Point::new(a.cos() * w * scale, a.sin() * h * scale)
            })
            .collect();
        if reverse {
            v.reverse();
        }
        v
    };
    Ok(match g.kind {
        GraphicKind::Rectangle => vec![vec![
            Point::new(-w, -h),
            Point::new(w, -h),
            Point::new(w, h),
            Point::new(-w, h),
        ]],
        GraphicKind::Ellipse => vec![ellipse(1., false)],
        GraphicKind::Ring => vec![ellipse(1., false), ellipse(0.76, true)],
        GraphicKind::Star => vec![
            (0..10)
                .map(|i| {
                    let a = i as f32 / 10. * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                    let f = if i % 2 == 0 { 1. } else { 0.45 };
                    Point::new(a.cos() * w * f, a.sin() * h * f)
                })
                .collect(),
        ],
        _ => anyhow::bail!("Convert to a custom path or use Rectangle/Ellipse/Star/Ring"),
    })
}
fn polygon(g: &Graphic, frame: u32) -> Result<MultiPolygon<f64>> {
    let contours = contours(g)?;
    ensure!(
        !contours.is_empty() && contours.iter().map(Vec::len).sum::<usize>() <= 10000,
        "Boolean shapes require explicit contours and at most 10,000 points"
    );
    let key = g.at(frame as f64);
    let rings = contours
        .iter()
        .map(|ring| {
            let points: Vec<_> = ring
                .iter()
                .map(|p| {
                    let x = p.x * key.scale;
                    let y = p.y * key.scale;
                    (
                        key.position.x as f64
                            + (x * key.rotation.cos() - y * key.rotation.sin()) as f64,
                        key.position.y as f64
                            + (x * key.rotation.sin() + y * key.rotation.cos()) as f64,
                    )
                })
                .collect();
            Polygon::new(LineString::from(points), vec![])
        })
        .collect();
    Ok(MultiPolygon(rings))
}
pub fn boolean_graphics(
    p: &Project,
    left: Id,
    right: Id,
    frame: u32,
    operation: BooleanOperation,
) -> Result<Graphic> {
    ensure!(
        left != right && frame < p.duration,
        "Choose two distinct shapes at a valid frame"
    );
    let a = p
        .graphics
        .iter()
        .find(|g| g.id == left)
        .context("First shape missing")?;
    let b = p
        .graphics
        .iter()
        .find(|g| g.id == right)
        .context("Second shape missing")?;
    ensure!(
        !p.bindings
            .iter()
            .any(|binding| binding.graphic == left || binding.graphic == right),
        "Unbind rig-attached graphics before Boolean editing"
    );
    let result = polygon(a, frame)?
        .boolean_op_with_fill_rule(
            &polygon(b, frame)?,
            match operation {
                BooleanOperation::Union => OpType::Union,
                BooleanOperation::Subtract => OpType::Difference,
                BooleanOperation::Intersect => OpType::Intersection,
                BooleanOperation::Xor => OpType::Xor,
            },
            FillRule::NonZero,
        )
        .orient(Direction::Default);
    let mut contours = vec![];
    for polygon in result.0 {
        for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
            let mut points: Vec<_> = ring
                .0
                .iter()
                .map(|p| Point::new(p.x as f32, p.y as f32))
                .collect();
            if points.first() == points.last() {
                points.pop();
            }
            if points.len() >= 3 {
                contours.push(points);
            }
        }
    }
    ensure!(
        !contours.is_empty() && contours.iter().map(Vec::len).sum::<usize>() <= 100000,
        "Boolean result is empty or exceeds geometry budget"
    );
    let mut out = a.clone();
    out.id = Id::new_v4();
    out.name = format!("{:?} · {}", operation, a.name);
    out.name.truncate(
        out.name
            .char_indices()
            .nth(500)
            .map_or(out.name.len(), |(i, _)| i),
    );
    out.contours = contours;
    out.start = frame;
    out.keys = vec![GraphicKey {
        frame,
        position: Point::default(),
        scale: 1.,
        rotation: 0.,
        opacity: a.at(frame as f64).opacity,
        easing: Easing::Hold,
    }];
    out.draw_on = false;
    Ok(out)
}
