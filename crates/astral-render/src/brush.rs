use crate::*;
pub fn append(
    out: &mut DisplayList,
    points: &[Point],
    stroke: &InkStroke,
    color: Color,
    transform: impl Fn(Point) -> Point,
    zoom: f32,
) {
    let Some(brush) = &stroke.brush else {
        return;
    };
    let mut distance = 0.;
    let mut next = 0.;
    let mut index = 0u64;
    let step = (stroke.width * brush.spacing).max(0.5);
    for (i, pair) in points.windows(2).enumerate() {
        let length = pair[0].distance(pair[1]);
        while next <= distance + length && index < 50001 {
            let t = if length > 0. {
                ((next - distance) / length).clamp(0., 1.)
            } else {
                0.
            };
            let pressure =
                stroke.pressure_at(i) + (stroke.pressure_at(i + 1) - stroke.pressure_at(i)) * t;
            let radius = (stroke.width * pressure * 0.5).max(0.05);
            let seed = brush.seed as u64 + index * 11;
            let q = pair[0].lerp(pair[1], t)
                + Point::new(
                    (random(seed) - 0.5) * stroke.width * brush.scatter,
                    (random(seed + 1) - 0.5) * stroke.width * brush.scatter,
                );
            let alpha = brush.flow * (1. - brush.grain * random(seed + 2));
            let rings = if brush.hardness < 0.99 { 4 } else { 1 };
            for ring in 0..rings {
                let factor = 1. - ring as f32 * (1. - brush.hardness) / 4.;
                let mut c = color;
                c[3] = (color[3] as f32 * alpha / rings as f32).round() as u8;
                out.primitives.push(Primitive::Circle {
                    center: transform(q),
                    radius: radius * factor * zoom,
                    color: c,
                });
            }
            next += step;
            index += 1;
        }
        distance += length;
    }
}
