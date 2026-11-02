use super::*;
impl Studio {
    pub(super) fn curve_tracks_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        ui.label("Independent root and joint channels");
        ui.small("Draft edits are applied as one undoable transaction. Curves override the chosen channel within their key range; rig constraints and planted contacts still apply.");
        egui::ComboBox::from_id_salt("curve.select")
            .selected_text(
                self.motion
                    .curve_edit
                    .as_ref()
                    .map_or("Select a curve", |c| c.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for c in &p.curves {
                    if ui.selectable_label(false, &c.name).clicked() {
                        self.motion.curve_edit = Some(c.clone());
                    }
                }
            });
        if ui.button("New channel from selected character").clicked() {
            if let Some(c) = self.selected.and_then(|id| p.character(id)) {
                let frame = (self.frame as u32).min(p.duration - 1);
                let value = evaluate(p, frame as f64)
                    .characters
                    .iter()
                    .find(|v| v.id == c.id)
                    .map_or(0., |v| v.pose.root.x);
                let mut keys = vec![ChannelKey {
                    frame,
                    value,
                    incoming: 0.,
                    outgoing: 0.,
                    interpolation: CurveInterpolation::Cubic,
                }];
                if frame + 1 < p.duration {
                    keys.push(ChannelKey {
                        frame: (frame + 24).min(p.duration - 1),
                        value,
                        incoming: 0.,
                        outgoing: 0.,
                        interpolation: CurveInterpolation::Cubic,
                    });
                }
                self.motion.curve_edit = Some(CurveTrack {
                    id: Id::new_v4(),
                    name: format!("{} root X", c.name),
                    character: c.id,
                    joint: None,
                    axis: CurveAxis::X,
                    keys,
                });
            } else {
                self.status = "Select a character first".into();
            }
        }
        let Some(mut c) = self.motion.curve_edit.take() else {
            return;
        };
        let Some(character) = p.character(c.character) else {
            return;
        };
        ui.text_edit_singleline(&mut c.name);
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("curve.joint")
                .selected_text(c.joint.map_or("Root", |j| {
                    character
                        .joints
                        .get(j)
                        .map_or("Missing", |j| j.name.as_str())
                }))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut c.joint, None, "Root");
                    for (j, joint) in character.joints.iter().enumerate() {
                        ui.selectable_value(&mut c.joint, Some(j), &joint.name);
                    }
                });
            ui.selectable_value(&mut c.axis, CurveAxis::X, "X");
            ui.selectable_value(&mut c.axis, CurveAxis::Y, "Y");
            if ui.button("Key at playhead").clicked() {
                let frame = self.frame as u32;
                if !c.keys.iter().any(|k| k.frame == frame) {
                    let scene = evaluate(p, self.frame);
                    let value = scene
                        .characters
                        .iter()
                        .find(|v| v.id == c.character)
                        .map_or(0., |v| {
                            let point = c.joint.map_or(v.pose.root, |j| v.pose.joints[j]);
                            match c.axis {
                                CurveAxis::X => point.x,
                                CurveAxis::Y => point.y,
                            }
                        });
                    c.keys.push(ChannelKey {
                        frame,
                        value,
                        incoming: 0.,
                        outgoing: 0.,
                        interpolation: CurveInterpolation::Cubic,
                    });
                    c.keys.sort_by_key(|k| k.frame);
                }
            }
            if ui.button("Auto tangents").clicked() {
                let values = c.keys.clone();
                for (i, k) in c.keys.iter_mut().enumerate() {
                    let a = &values[i.saturating_sub(1)];
                    let b = &values[(i + 1).min(values.len() - 1)];
                    let slope = if b.frame > a.frame {
                        (b.value - a.value) / (b.frame - a.frame) as f32
                    } else {
                        0.
                    };
                    k.incoming = slope;
                    k.outgoing = slope;
                }
            }
        });
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 240.), Sense::hover());
        let inner = rect.shrink2(Vec2::new(28., 24.));
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8., theme::BACKGROUND);
        let first = c.keys.first().map_or(0, |k| k.frame);
        let last = c.keys.last().map_or(1, |k| k.frame).max(first + 1);
        let span = (last - first) as f32;
        let min = c.keys.iter().map(|k| k.value).fold(f32::INFINITY, f32::min);
        let max = c
            .keys
            .iter()
            .map(|k| k.value)
            .fold(f32::NEG_INFINITY, f32::max);
        let range = (max - min).max(20.);
        let low = min - range * 0.25;
        let high = max + range * 0.25;
        let at = |f: f32, v: f32| {
            Pos2::new(
                inner.left() + (f - first as f32) / span * inner.width(),
                inner.bottom() - (v - low) / (high - low) * inner.height(),
            )
        };
        for i in 0..=4 {
            let x = inner.left() + inner.width() * i as f32 / 4.;
            painter.line_segment(
                [Pos2::new(x, inner.top()), Pos2::new(x, inner.bottom())],
                Stroke::new(1., theme::BORDER),
            );
        }
        let pts = (0..=256)
            .filter_map(|i| {
                let f = first as f64 + (last - first) as f64 * i as f64 / 256.;
                c.sample(f).map(|v| at(f as f32, v))
            })
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new(2., theme::ACCENT)));
        painter.text(
            rect.left_bottom(),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "Frames {}Ã¢â‚¬â€œ{}  Ã‚Â·  {:.1}Ã¢â‚¬â€œ{:.1}",
                first + 1,
                last + 1,
                low,
                high
            ),
            egui::FontId::proportional(11.),
            theme::MUTED,
        );
        let snapshot = c.keys.clone();
        for (i, k) in c.keys.iter_mut().enumerate() {
            let center = at(k.frame as f32, k.value);
            for (side, slope) in [(false, &mut k.incoming), (true, &mut k.outgoing)] {
                let dt = span.min(24.) / 6. * if side { 1. } else { -1. };
                let handle = at(k.frame as f32 + dt, k.value + *slope * dt);
                painter.line_segment([center, handle], Stroke::new(1., theme::MUTED));
                painter.circle_filled(handle, 3., theme::MUTED);
                let r = ui.interact(
                    Rect::from_center_size(handle, Vec2::splat(12.)),
                    ui.scope_id().with(("channel_tangent", i, side)),
                    Sense::drag(),
                );
                if r.dragged()
                    && let Some(pos) = r.interact_pointer_pos()
                {
                    let value = low + (inner.bottom() - pos.y) / inner.height() * (high - low);
                    *slope = ((value - k.value) / dt).clamp(-100_000., 100_000.);
                }
                r.on_hover_text(if side {
                    "Outgoing tangent"
                } else {
                    "Incoming tangent"
                });
            }
            painter.circle_filled(center, 5., theme::ACCENT);
            let r = ui.interact(
                Rect::from_center_size(center, Vec2::splat(14.)),
                ui.scope_id().with(("channel_key", i)),
                Sense::drag(),
            );
            if r.dragged()
                && let Some(pos) = r.interact_pointer_pos()
            {
                let frame = (first as f32 + (pos.x - inner.left()) / inner.width() * span)
                    .round()
                    .max(0.) as u32;
                let lower = if i == 0 { 0 } else { snapshot[i - 1].frame + 1 };
                let upper = snapshot.get(i + 1).map_or(p.duration - 1, |n| n.frame - 1);
                k.frame = frame.clamp(lower, upper);
                k.value = (low + (inner.bottom() - pos.y) / inner.height() * (high - low))
                    .clamp(-1_000_000., 1_000_000.);
            }
        }
        let mut remove = None;
        egui::ScrollArea::vertical()
            .max_height(180.)
            .show(ui, |ui| {
                for (i, k) in c.keys.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("Frame {}", k.frame + 1));
                            ui.add(
                                egui::DragValue::new(&mut k.value)
                                    .speed(0.5)
                                    .prefix("Value "),
                            );
                            ui.add(
                                egui::DragValue::new(&mut k.incoming)
                                    .speed(0.01)
                                    .prefix("In "),
                            );
                            ui.add(
                                egui::DragValue::new(&mut k.outgoing)
                                    .speed(0.01)
                                    .prefix("Out "),
                            );
                            egui::ComboBox::from_id_salt("interpolation")
                                .selected_text(format!("{:?}", k.interpolation))
                                .show_ui(ui, |ui| {
                                    for mode in [
                                        CurveInterpolation::Cubic,
                                        CurveInterpolation::Linear,
                                        CurveInterpolation::Hold,
                                    ] {
                                        ui.selectable_value(
                                            &mut k.interpolation,
                                            mode,
                                            format!("{mode:?}"),
                                        );
                                    }
                                });
                            if ui.small_button("Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                }
            });
        if let Some(i) = remove
            && c.keys.len() > 1
        {
            c.keys.remove(i);
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Apply curve").clicked() {
                let mut tracks = p.curves.clone();
                tracks.retain(|t| t.id != c.id);
                tracks.push(c.clone());
                self.apply(vec![Command::SetCurveTracks { tracks }]);
            }
            if ui.button("Delete curve").clicked() {
                self.apply(vec![Command::SetCurveTracks {
                    tracks: p.curves.iter().filter(|t| t.id != c.id).cloned().collect(),
                }]);
            }
        });
        self.motion.curve_edit = Some(c);
    }
}
