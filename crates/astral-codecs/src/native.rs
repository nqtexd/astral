//! Explicit authored-pose conversion. Unsupported source features remain archived.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
fn number(v: &Value, key: &str, default: f32) -> f32 {
    v[key].as_f64().map_or(default, |n| n as f32)
}
pub fn convert(p: &mut Project, report: &Value) -> Result<Vec<String>> {
    let Some(frames) = report["keyframes"].as_array() else {
        return Ok(vec![
            "No canonical frames were available for native conversion".into(),
        ]);
    };
    ensure!(
        frames.len() <= 12000,
        "Legacy conversion exceeds 12000 authored frames"
    );
    let figures: BTreeMap<_, _> = report["figure_types"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f["figure_type_id"].as_u64()?, f)))
        .collect();
    let mut notices = BTreeSet::new();
    let layer = DrawingLayer::new("Imported authored poses");
    let mut tick = 0u32;
    let mut cels = vec![];
    let mut total_points = 0;
    for (index, frame) in frames.iter().enumerate() {
        let duration = number(frame, "duration_raw", 1.).max(0.) as u32;
        if duration == 0 {
            continue;
        }
        let mut strokes = vec![];
        let objects = frame["objects"].as_array().cloned().unwrap_or_default();
        let order: Vec<usize> = frame["object_draw_order"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_else(|| (0..objects.len()).collect());
        for slot in order {
            let Some(object) = objects.get(slot) else {
                continue;
            };
            if !object["join"].is_null() {
                notices.insert("Joined objects were skipped; their original relationship is retained in the archive");
                continue;
            }
            let Some(figure) = object["figure_type_id"]
                .as_u64()
                .and_then(|id| figures.get(&id))
            else {
                continue;
            };
            if figure["_styk"]["movieclip"].is_object() {
                notices.insert("Nested movieclips require the legacy editor");
                continue;
            }
            let Some(segments) = figure["segments"].as_array() else {
                continue;
            };
            let scale = number(object, "scale", 1.);
            let root = Point::new(number(object, "root_x", 0.), number(object, "root_y", 0.));
            let mut endpoints = BTreeMap::from([(0, root)]);
            let mut pending: Vec<usize> = (0..segments.len()).collect();
            while !pending.is_empty() {
                let before = pending.len();
                let mut next = vec![];
                for i in pending {
                    let segment = &segments[i];
                    let parent = segment["parent_id"].as_u64().unwrap_or(0);
                    let Some(start) = endpoints.get(&parent).copied() else {
                        next.push(i);
                        continue;
                    };
                    let angle = object["angles"][i]
                        .as_f64()
                        .map_or(number(segment, "angle_rad", 0.), |v| v as f32);
                    let length = if number(object, "len_flag", 0.) != 0. {
                        object["lengths"][i]
                            .as_f64()
                            .map_or(number(segment, "length", 0.), |v| v as f32)
                    } else {
                        number(segment, "length", 0.)
                    } * scale;
                    let bend = if number(object, "bend_flag", 0.) != 0. {
                        object["bends"][i].as_f64().unwrap_or(0.) as f32
                    } else {
                        number(segment, "bend_rad", 0.)
                    };
                    let steps = if bend.abs() < 0.00001 { 1 } else { 32 };
                    let points: Vec<_> = (0..=steps)
                        .map(|n| {
                            let t = n as f32 / steps as f32;
                            if bend.abs() < 0.00001 {
                                start
                                    + Point::new(length * t * angle.cos(), length * t * angle.sin())
                            } else {
                                let radius = length / bend;
                                start
                                    + Point::new(
                                        radius * ((angle + bend * t).sin() - angle.sin()),
                                        radius * (angle.cos() - (angle + bend * t).cos()),
                                    )
                            }
                        })
                        .collect();
                    let end = *points.last().unwrap();
                    endpoints.insert(segment["segment_id"].as_u64().unwrap_or(i as u64 + 1), end);
                    let primitive = number(segment, "primitive_code", 0.) as u32;
                    if ![0, 1, 3, 4].contains(&primitive)
                        || !segment["paint"].is_null()
                        || !segment["_sticknodes"].is_null()
                    {
                        notices.insert("Image/text segments, square caps, gradients and custom Stick Nodes shapes were skipped");
                        continue;
                    }
                    let width = number(segment, "thickness", 1.) * scale.abs();
                    if width < 0.1 {
                        continue;
                    }
                    let raw = object["color_override_raw"].as_u64().unwrap_or(0) as u32;
                    let mix = (raw >> 24) as f32 / 256.;
                    let mut color = [0u8; 4];
                    for c in 0..3 {
                        let base = segment["color_rgb"][c].as_u64().unwrap_or(0) as f32;
                        let other = ((raw >> (8 * c)) & 255) as f32;
                        color[c] = (base * (1. - mix) + other * mix).round() as u8;
                    }
                    color[3] = ((1. - number(segment, "transparency", 0.) / 255.)
                        * (1. - number(object, "field28", 0.) / 255.)
                        * 255.)
                        .round()
                        .clamp(0., 255.) as u8;
                    if primitive == 0 {
                        if width <= 500. {
                            strokes.push(InkStroke {
                                points,
                                color,
                                width,
                            });
                        } else {
                            notices.insert(
                                "Strokes wider than the native 500-pixel limit were skipped",
                            );
                        }
                    } else {
                        let center = start.lerp(end, 0.5);
                        let filled = primitive == 3 || width >= length.abs();
                        let radius = (length.abs() + if filled { width } else { 0. }) / 2.;
                        if radius * 2. > 500. {
                            notices
                                .insert("Circles exceeding the native stroke limit were skipped");
                            continue;
                        }
                        if filled {
                            strokes.push(InkStroke {
                                points: vec![center, center + Point::new(0.001, 0.)],
                                color,
                                width: (radius * 2.).max(0.1),
                            });
                        } else {
                            if primitive == 1 {
                                strokes.push(InkStroke {
                                    points: vec![center, center + Point::new(0.001, 0.)],
                                    color: [255, 255, 255, color[3]],
                                    width: (radius * 2.).max(0.1),
                                });
                            }
                            strokes.push(InkStroke {
                                points: (0..=64)
                                    .map(|n| {
                                        let a = n as f32 / 64. * std::f32::consts::TAU;
                                        center + Point::new(a.cos() * radius, a.sin() * radius)
                                    })
                                    .collect(),
                                color,
                                width,
                            });
                        }
                    }
                }
                ensure!(
                    next.len() < before,
                    "Legacy segment topology is cyclic or incomplete"
                );
                pending = next;
            }
            if figure["polyfills"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
            {
                notices.insert("Polygon fills remain in the archived source");
            }
        }
        total_points += strokes.iter().map(|s| s.points.len()).sum::<usize>();
        ensure!(
            total_points <= 2_000_000,
            "Native conversion exceeds two million points"
        );
        cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: layer.id,
            frame: tick,
            strokes,
        });
        tick = tick
            .checked_add(
                if report["version"].as_u64().unwrap_or(5) >= 5 && index + 1 == frames.len() {
                    1
                } else {
                    duration
                },
            )
            .context("Legacy timing overflow")?;
        ensure!(tick <= 864000, "Legacy duration exceeds native limit");
    }
    if cels.iter().any(|c| !c.strokes.is_empty()) {
        p.schema = p.schema.max(4);
        p.duration = tick.max(1);
        p.layers.push(layer);
        p.cels = cels;
    }
    let mut warnings=vec!["Supported lines, bent strokes and circles are editable native drawing cels at authored source poses. Source tween solvers, rig semantics, camera transforms, backgrounds, effects and audio are not reproduced. Inspect the converted frames before continuing.".into()];
    warnings.extend(notices.into_iter().map(String::from));
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_lines_keep_parent_geometry_timing_and_independent_editability() {
        let report = json!({"version":5,"figure_types":[{"figure_type_id":1,"segments":[{"segment_id":1,"parent_id":0,"length":20,"angle_rad":0,"thickness":3,"color_rgb":[100,180,220]},{"segment_id":2,"parent_id":1,"length":10,"angle_rad":0,"thickness":3,"color_rgb":[100,180,220]}]}],"keyframes":[{"duration_raw":3,"objects":[{"figure_type_id":1,"root_x":12,"root_y":15,"scale":1,"angles":[0,1.5707963267948966]}]},{"duration_raw":5,"objects":[{"figure_type_id":1,"root_x":32,"root_y":15,"scale":1,"angles":[0,0]}]}]});
        let mut p = Project::new("Source", 64, 64, 24, 300);
        convert(&mut p, &report).unwrap();
        p.validate().unwrap();
        assert_eq!(p.duration, 4);
        assert_eq!(p.cels.len(), 2);
        assert_eq!(p.cels[1].frame, 3);
        assert_eq!(p.cels[0].strokes[0].points[0], Point::new(12., 15.));
        assert_eq!(p.cels[0].strokes[1].points[0], Point::new(32., 15.));
        assert!((p.cels[0].strokes[1].points[1].y - 25.).abs() < 0.001);
        let before = p.cels[1].strokes.clone();
        let layer = p.layers[0].id;
        let mut session = Session::new(p).unwrap();
        session
            .apply(Batch {
                project: session.project.id,
                expected_revision: 0,
                commands: vec![Command::SetFrameDrawing {
                    layer,
                    frame: 0,
                    strokes: vec![],
                }],
            })
            .unwrap();
        assert_eq!(session.project.cel_at(layer, 3).unwrap().strokes, before);
        session
            .undo(session.project.id, session.project.revision)
            .unwrap();
        assert!(!session.project.cel_at(layer, 0).unwrap().strokes.is_empty());
    }
}
