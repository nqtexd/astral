use crate::*;
fn transform(primitive: &mut Primitive, key: &GraphicKey, origin: Point) {
    let at = |p: Point| {
        let p = p - origin;
        let (s, c) = key.rotation.sin_cos();
        key.position
            + Point::new(
                (p.x * c - p.y * s) * key.scale,
                (p.x * s + p.y * c) * key.scale,
            )
    };
    let alpha = |c: &mut Color| {
        c[3] = (c[3] as f32 * key.opacity).round() as u8;
    };
    match primitive {
        Primitive::Line { a, b, width, color } => {
            *a = at(*a);
            *b = at(*b);
            *width *= key.scale;
            alpha(color);
        }
        Primitive::Circle {
            center,
            radius,
            color,
        } => {
            *center = at(*center);
            *radius *= key.scale;
            alpha(color);
        }
        Primitive::Polyline {
            points,
            width,
            color,
        } => {
            for p in points {
                *p = at(*p);
            }
            *width *= key.scale;
            alpha(color);
        }
        Primitive::Path { contours, color } => {
            for p in contours.iter_mut().flatten() {
                *p = at(*p);
            }
            alpha(color);
        }
        Primitive::Image {
            vertices, opacity, ..
        } => {
            for v in vertices {
                v.position = at(v.position);
            }
            *opacity *= key.opacity;
        }
    }
}
fn instance(
    p: &Project,
    entries: &[studio::LibraryEntry],
    s: &SymbolInstance,
    frame: f64,
    depth: u32,
) -> Vec<Primitive> {
    if depth > 8 {
        return vec![];
    }
    let Some(entry) = entries.iter().find(|e| e.id == s.entry) else {
        return vec![];
    };
    let Some(local) = s.local_frame(frame, entry.duration) else {
        return vec![];
    };
    let mut child = Project::new(&entry.name, p.width, p.height, p.fps, entry.duration);
    child.schema = 11;
    child.background = [0, 0, 0, 0];
    child.bitmaps = p.bitmaps.clone();
    child.characters = entry.figures.clone();
    child.layers = entry.layers.clone();
    child.cels = entry.cels.clone();
    child.effects = entry.effects.clone();
    child.markers = entry.markers.clone();
    let mut list = display_list(&child, local).primitives;
    for nested in p.symbols.iter().filter(|v| v.parent == Some(s.id)) {
        list.extend(instance(p, entries, nested, local, depth + 1));
    }
    let key = s.at(frame);
    for primitive in &mut list {
        transform(primitive, &key, entry.origin);
    }
    list
}
pub fn append(out: &mut DisplayList, p: &Project, frame: f64) {
    if p.symbols.is_empty() {
        return;
    }
    let Ok(entries) = studio::library_entries(p) else {
        return;
    };
    let scene = evaluate(p, frame);
    let camera = GraphicKey {
        frame: 0,
        position: Point::new(p.width as f32 / 2., p.height as f32 / 2.),
        scale: scene.zoom,
        rotation: 0.,
        opacity: 1.,
        easing: Easing::Linear,
    };
    let mut background = vec![];
    for s in p.symbols.iter().filter(|v| v.parent.is_none()) {
        let mut primitives = instance(p, &entries, s, frame, 0);
        for primitive in &mut primitives {
            transform(primitive, &camera, scene.center);
        }
        if s.layer == GraphicLayer::Background {
            background.extend(primitives);
        } else {
            out.primitives.extend(primitives);
        }
    }
    background.append(&mut out.primitives);
    out.primitives = background;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn symbol_rotation_matches_graphic_key_radians() {
        let mut p = Primitive::Line {
            a: Point::new(1., 0.),
            b: Point::new(2., 0.),
            width: 2.,
            color: [255; 4],
        };
        let key = GraphicKey {
            frame: 0,
            position: Point::new(10., 20.),
            scale: 2.,
            rotation: std::f32::consts::FRAC_PI_2,
            opacity: 0.5,
            easing: Easing::Linear,
        };
        transform(&mut p, &key, Point::default());
        let Primitive::Line { a, b, width, color } = p else {
            unreachable!()
        };
        assert!(a.distance(Point::new(10., 22.)) < 0.001);
        assert!(b.distance(Point::new(10., 24.)) < 0.001);
        assert_eq!(width, 4.);
        assert_eq!(color[3], 128);
    }
    #[test]
    fn live_instances_update_together_and_nested_cycles_are_rejected() {
        let mut p = Project::new("Symbols", 64, 64, 24, 48);
        p.schema = 11;
        let layer = DrawingLayer {
            id: Id::new_v4(),
            name: "Dot".into(),
            visible: true,
            locked: false,
            opacity: 1.,
        };
        let entry = studio::LibraryEntry {
            id: Id::new_v4(),
            name: "Dot source".into(),
            kind: studio::LibraryKind::Symbol,
            duration: 24,
            origin: Point::default(),
            figures: vec![],
            layers: vec![layer.clone()],
            cels: vec![DrawingCel {
                id: Id::new_v4(),
                layer: layer.id,
                frame: 0,
                strokes: vec![InkStroke {
                    points: vec![Point::new(0., 0.), Point::new(3., 0.)],
                    color: [255, 0, 0, 255],
                    width: 3.,
                    brush: None,
                    bitmap: None,
                    skin: None,
                    pressures: vec![],
                    style: BrushStyle::Solid,
                    filled: false,
                    curved: false,
                }],
            }],
            effects: vec![],
            markers: Default::default(),
        };
        let key = GraphicKey {
            frame: 0,
            position: Point::new(15., 15.),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        };
        let parent = SymbolInstance {
            id: Id::new_v4(),
            name: "Parent".into(),
            entry: entry.id,
            parent: None,
            start: 0,
            end: 47,
            offset: 0.,
            speed: 1.,
            looping: true,
            layer: GraphicLayer::Foreground,
            keys: vec![key.clone()],
        };
        let child = SymbolInstance {
            id: Id::new_v4(),
            name: "Child".into(),
            parent: Some(parent.id),
            end: 23,
            keys: vec![key],
            ..parent.clone()
        };
        let mut session = Session::new(p).unwrap();
        session
            .apply(Batch {
                project: session.project.id,
                expected_revision: 0,
                commands: vec![
                    Command::SetLibrary {
                        entries: vec![entry.clone()],
                    },
                    Command::SetSymbolInstances {
                        instances: vec![parent.clone(), child.clone()],
                    },
                ],
            })
            .unwrap();
        let before = png(&session.project, 0., 64, 64).unwrap();
        let mut edited = entry;
        edited.cels[0].strokes[0].color = [0, 255, 0, 255];
        session
            .apply(Batch {
                project: session.project.id,
                expected_revision: session.project.revision,
                commands: vec![Command::SetLibrary {
                    entries: vec![edited],
                }],
            })
            .unwrap();
        assert_ne!(before, png(&session.project, 0., 64, 64).unwrap());
        let revision = session.project.revision;
        let mut cycle = parent;
        cycle.parent = Some(child.id);
        cycle.end = 23;
        assert!(
            session
                .apply(Batch {
                    project: session.project.id,
                    expected_revision: revision,
                    commands: vec![Command::SetSymbolInstances {
                        instances: vec![cycle, child]
                    }]
                })
                .is_err()
        );
        assert_eq!(session.project.revision, revision);
    }
}
