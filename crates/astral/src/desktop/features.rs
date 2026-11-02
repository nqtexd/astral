use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GraphicPreset {
    Hold,
    Drift,
    Slide,
    Pop,
    Spin,
    DrawOn,
}

impl Studio {
    pub(super) fn feature_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        if matches!(
            self.preferences.mode,
            WorkspaceMode::Animate | WorkspaceMode::Rig | WorkspaceMode::Draw
        ) {
            ui.heading(i18n::tr("Learn inbetweens"));
            ui.label(
                RichText::new("Offline learning from your example frames")
                    .size(11.)
                    .color(MUTED),
            );
            ui.add(
                egui::DragValue::new(&mut self.learning_smoothing)
                    .range(0.00000001..=0.1)
                    .speed(0.0001)
                    .prefix("Smoothing "),
            );
            if self.preferences.mode != WorkspaceMode::Draw {
                if let Some(c) = self.selected.and_then(|id| p.character(id)) {
                    ui.label(format!("{} authored poses - need at least 3", c.keys.len()));
                    if ui.button(i18n::tr("Key current pose")).clicked() {
                        let pose = evaluate(p, self.frame)
                            .characters
                            .into_iter()
                            .find(|v| v.id == c.id)
                            .map(|c| c.pose);
                        if let Some(pose) = pose {
                            self.apply(vec![Command::SetPose {
                                character: c.id,
                                frame: self.frame as u32,
                                pose,
                                easing: Easing::Smooth,
                            }]);
                        }
                    }
                    if ui
                        .add_enabled(
                            c.keys.len() >= 3,
                            egui::Button::new("Learn pose inbetweens"),
                        )
                        .clicked()
                    {
                        self.apply(vec![Command::TrainMotion {
                            character: c.id,
                            name: format!("{} learned motion", c.name),
                            smoothing: self.learning_smoothing,
                        }]);
                    }
                    if let Some(m) = p.learned_motions.iter().find(|m| m.character == c.id) {
                        ui.colored_label(
                            VIOLET,
                            format!("Learning active - frames {}–{}", m.start, m.end),
                        );
                        if ui.button(i18n::tr("Bake to editable pose keys")).clicked() {
                            self.apply(vec![Command::BakeMotion { motion: m.id }]);
                        }
                    }
                } else {
                    ui.label(i18n::tr("Select a character. Pose it at three or more frames in Rig, then learn the motion."));
                }
            } else {
                ui.checkbox(&mut self.edit_stroke, i18n::tr("Edit stroke points"));
                egui::ComboBox::from_id_salt("stroke.edit")
                    .selected_text(
                        self.selected_stroke
                            .and_then(|id| p.drawings.iter().find(|d| d.id == id))
                            .map_or("Select a drawing", |d| d.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for d in &p.drawings {
                            if ui
                                .selectable_label(
                                    self.selected_stroke == Some(d.id),
                                    format!("{} - frame {}", d.name, d.start),
                                )
                                .clicked()
                            {
                                self.selected_stroke = Some(d.id);
                                self.edit_stroke = true;
                                self.stop();
                                self.frame = d.start as f64;
                            }
                        }
                    });
                ui.label(i18n::tr("Draw the same outline at 3+ different frames. Select the corresponding strokes:"));
                for d in &p.drawings {
                    if p.drawing_motions.iter().any(|m| m.sources.contains(&d.id)) {
                        continue;
                    }
                    let mut checked = self.learning_drawings.contains(&d.id);
                    if ui
                        .checkbox(&mut checked, format!("{} - frame {}", d.name, d.start))
                        .changed()
                    {
                        if checked {
                            self.learning_drawings.push(d.id)
                        } else {
                            self.learning_drawings.retain(|id| *id != d.id)
                        }
                    }
                }
                self.learning_drawings
                    .retain(|id| p.drawings.iter().any(|d| d.id == *id));
                if ui
                    .add_enabled(
                        self.learning_drawings.len() >= 3,
                        egui::Button::new("Learn stroke inbetweens"),
                    )
                    .clicked()
                {
                    self.apply(vec![Command::TrainDrawing {
                        sources: self.learning_drawings.clone(),
                        name: "Learned drawing".into(),
                        smoothing: self.learning_smoothing,
                    }]);
                    self.learning_drawings.clear();
                }
                for m in &p.drawing_motions {
                    ui.label(format!("{} - {}–{}", m.name, m.start, m.end));
                    if ui.button(format!("Bake {} into frames", m.name)).clicked() {
                        self.apply(vec![Command::BakeDrawing { motion: m.id }]);
                    }
                }
            }
            ui.separator();
        }
        if self.preferences.mode == WorkspaceMode::Dialogue {
            ui.heading(i18n::tr("Dialogue & captions"));
            ui.label(i18n::tr("Speaker"));
            ui.text_edit_singleline(&mut self.dialogue_speaker);
            ui.label(i18n::tr("Text"));
            ui.add(
                egui::TextEdit::multiline(&mut self.dialogue_text)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            );
            theme::slider(
                ui,
                "Font size",
                egui::Slider::new(&mut self.text_font_size, 8.0..=120.),
            );
            text_format_controls(ui, &mut self.dialogue_formatting);
            egui::ComboBox::from_id_salt("dialogue.style")
                .selected_text(format!("{:?}", self.dialogue_style))
                .show_ui(ui, |ui| {
                    for style in [
                        DialogueStyle::Speech,
                        DialogueStyle::Thought,
                        DialogueStyle::Caption,
                    ] {
                        ui.selectable_value(&mut self.dialogue_style, style, format!("{style:?}"));
                    }
                });
            ui.add(
                egui::DragValue::new(&mut self.feature_duration)
                    .range(1..=100000)
                    .prefix("Duration (frames) "),
            );
            theme::slider(
                ui,
                "Typewriter / sec",
                egui::Slider::new(&mut self.dialogue_speed, 0.0..=100.),
            );
            if theme::icon_button(ui, "Add dialogue at playhead", theme::Icon::Plus, false)
                .clicked()
            {
                let start = self.frame as u32;
                let character = if self.dialogue_style == DialogueStyle::Caption {
                    None
                } else {
                    self.selected
                };
                let d = Dialogue {
                    id: Id::new_v4(),
                    name: if self.dialogue_speaker.is_empty() {
                        "Dialogue".into()
                    } else {
                        self.dialogue_speaker.clone()
                    },
                    text: self.dialogue_text.clone(),
                    speaker: self.dialogue_speaker.clone(),
                    start,
                    end: start
                        .saturating_add(self.feature_duration - 1)
                        .min(p.duration - 1),
                    position: if character.is_some() {
                        Point::new(-230., -340.)
                    } else {
                        Point::new(p.width as f32 * 0.2, p.height as f32 * 0.12)
                    },
                    size: Point::new(540., 190.),
                    font_size: self.text_font_size,
                    color: [250, 248, 255, 255],
                    text_color: [32, 28, 48, 255],
                    appearance: DialogueAppearance::default(),
                    style: self.dialogue_style,
                    character,
                    typewriter: self.dialogue_speed,
                    formatting: self.dialogue_formatting.clone(),
                };
                self.selected_dialogue = Some(d.id);
                self.apply(vec![Command::AddDialogue { dialogue: d }]);
            }
            ui.separator();
            for d in &p.dialogues {
                if ui
                    .selectable_label(
                        self.selected_dialogue == Some(d.id),
                        format!("{} - {}–{}", d.name, d.start, d.end),
                    )
                    .clicked()
                {
                    self.selected_dialogue = Some(d.id);
                    self.dialogue_text = d.text.clone();
                    self.dialogue_speaker = d.speaker.clone();
                    self.dialogue_style = d.style;
                    self.dialogue_speed = d.typewriter;
                    self.dialogue_formatting = d.formatting.clone();
                    self.text_font_size = d.font_size;
                }
            }
            if let Some(d) = self
                .selected_dialogue
                .and_then(|id| p.dialogues.iter().find(|d| d.id == id))
            {
                let mut next = d.clone();
                let mut changed = false;
                if ui
                    .button(i18n::tr("Fit text & suggest placement"))
                    .clicked()
                {
                    self.motion_plan(
                        p,
                        astral_core::studio::StudioOperation::LayoutDialogue { dialogue: d.id },
                        self.frame as u32,
                    );
                }
                if let Ok(issues) =
                    astral_core::composition::review(p, self.frame as u32, self.frame as u32, 1)
                {
                    for issue in issues.iter().filter(|i| i.subject == d.id).take(4) {
                        ui.colored_label(Color32::from_rgb(242, 199, 116), &issue.message);
                    }
                }
                ui.label(i18n::tr("Position / size"));
                changed |= ui
                    .add(egui::DragValue::new(&mut next.position.x).prefix("X "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut next.position.y).prefix("Y "))
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.size.x)
                            .range(100.0..=8192.)
                            .prefix("Width "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.size.y)
                            .range(60.0..=8192.)
                            .prefix("Height "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.start)
                            .range(0..=next.end)
                            .prefix("Start "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.end)
                            .range(next.start..=p.duration - 1)
                            .prefix("End "),
                    )
                    .changed();
                ui.label(i18n::tr("Bubble fill"));
                changed |= ui
                    .color_edit_button_srgba_unmultiplied(&mut next.color)
                    .changed();
                ui.label(i18n::tr("Border"));
                changed |= ui
                    .color_edit_button_srgba_unmultiplied(&mut next.appearance.border_color)
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.appearance.border_width)
                            .range(0.0..=20.)
                            .prefix("Width "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.appearance.corner_radius)
                            .range(0.0..=200.)
                            .prefix("Corners "),
                    )
                    .changed();
                ui.label(i18n::tr("Speaker color"));
                changed |= ui
                    .color_edit_button_srgba_unmultiplied(&mut next.appearance.speaker_color)
                    .changed();
                if ui.button(i18n::tr("Dark caption")).clicked() {
                    next.appearance = DialogueAppearance::dark();
                    next.color = [24, 27, 33, 245];
                    next.text_color = [239, 242, 248, 255];
                    next.style = DialogueStyle::Caption;
                    self.dialogue_style = DialogueStyle::Caption;
                    changed = true;
                }
                ui.label(i18n::tr("Text color"));
                changed |= ui
                    .color_edit_button_srgba_unmultiplied(&mut next.text_color)
                    .changed();
                if ui.button(i18n::tr("Update text & style")).clicked() {
                    next.text = self.dialogue_text.clone();
                    next.speaker = self.dialogue_speaker.clone();
                    next.style = self.dialogue_style;
                    next.typewriter = self.dialogue_speed;
                    next.formatting = self.dialogue_formatting.clone();
                    next.font_size = self.text_font_size;
                    changed = true;
                }
                if changed {
                    self.apply(vec![Command::ReplaceDialogue { dialogue: next }]);
                }
                if theme::icon_button(ui, "Delete dialogue", theme::Icon::Delete, false).clicked() {
                    self.apply(vec![Command::Remove { id: d.id }]);
                    self.selected_dialogue = None;
                }
            }
            ui.separator();
        }
        if self.preferences.mode == WorkspaceMode::Graphics {
            ui.heading(i18n::tr("Motion graphics"));
            self.vector_path_tools(ui, p);
            egui::ComboBox::from_id_salt("graphic.kind")
                .selected_text(format!("{:?}", self.graphic_kind))
                .show_ui(ui, |ui| {
                    for kind in GraphicKind::ALL {
                        ui.selectable_value(&mut self.graphic_kind, kind, format!("{kind:?}"));
                    }
                });
            egui::ComboBox::from_id_salt("graphic.preset")
                .selected_text(preset_name(self.graphic_preset))
                .show_ui(ui, |ui| {
                    for preset in [
                        GraphicPreset::Hold,
                        GraphicPreset::Drift,
                        GraphicPreset::Slide,
                        GraphicPreset::Pop,
                        GraphicPreset::Spin,
                        GraphicPreset::DrawOn,
                    ] {
                        ui.selectable_value(&mut self.graphic_preset, preset, preset_name(preset));
                    }
                });
            if self.graphic_kind == GraphicKind::LowerThird {
                ui.text_edit_singleline(&mut self.graphic_text);
            }
            ui.add(
                egui::DragValue::new(&mut self.feature_duration)
                    .range(2..=100000)
                    .prefix("Duration (frames) "),
            );
            if theme::icon_button(ui, "Add graphic at playhead", theme::Icon::Plus, false).clicked()
            {
                let start = self.frame as u32;
                let end = start
                    .saturating_add(self.feature_duration.max(2) - 1)
                    .min(p.duration - 1);
                let center = if self.graphic_kind == GraphicKind::Cloud {
                    Point::new(p.width as f32 * 0.35, p.height as f32 * 0.2)
                } else {
                    Point::new(p.width as f32 * 0.5, p.height as f32 * 0.72)
                };
                let mut a = GraphicKey {
                    frame: start,
                    position: center,
                    scale: 1.,
                    rotation: 0.,
                    opacity: 1.,
                    easing: Easing::Smooth,
                };
                let mut b = a.clone();
                b.frame = end;
                let mut keys = vec![];
                match self.graphic_preset {
                    GraphicPreset::Hold => {}
                    GraphicPreset::Drift => b.position.x += 350.,
                    GraphicPreset::Slide => {
                        a.position.x -= 500.;
                        a.opacity = 0.;
                    }
                    GraphicPreset::Pop => {
                        a.scale = 0.1;
                        a.opacity = 0.;
                        if end > start + 1 {
                            let mut overshoot = b.clone();
                            overshoot.frame = start + (end - start) / 2;
                            overshoot.scale = 1.15;
                            keys.push(overshoot);
                        }
                    }
                    GraphicPreset::Spin => b.rotation = std::f32::consts::TAU,
                    GraphicPreset::DrawOn => {}
                }
                keys.insert(0, a);
                if end > start {
                    keys.push(b)
                } else {
                    keys.clear();
                    keys.push(b);
                }
                let graphic = Graphic {
                    contours: vec![],
                    id: Id::new_v4(),
                    name: format!("{:?}", self.graphic_kind),
                    kind: self.graphic_kind,
                    start,
                    end,
                    size: if self.graphic_kind == GraphicKind::Cloud {
                        Point::new(420., 200.)
                    } else if self.graphic_kind == GraphicKind::LowerThird {
                        Point::new(650., 130.)
                    } else {
                        Point::new(180., 180.)
                    },
                    color: if self.graphic_kind == GraphicKind::Cloud {
                        [227, 235, 255, 220]
                    } else {
                        [140, 100, 220, 255]
                    },
                    layer: if self.graphic_kind == GraphicKind::Cloud {
                        GraphicLayer::Background
                    } else {
                        GraphicLayer::Foreground
                    },
                    text: self.graphic_text.clone(),
                    keys,
                    draw_on: self.graphic_preset == GraphicPreset::DrawOn,
                };
                self.selected_graphic = Some(graphic.id);
                self.apply(vec![Command::AddGraphic { graphic }]);
            }
            ui.separator();
            for g in &p.graphics {
                if ui
                    .selectable_label(
                        self.selected_graphic == Some(g.id),
                        format!("{} - {}–{}", g.name, g.start, g.end),
                    )
                    .clicked()
                {
                    self.selected_graphic = Some(g.id);
                }
            }
            if let Some(g) = self
                .selected_graphic
                .and_then(|id| p.graphics.iter().find(|g| g.id == id))
            {
                ui.label(format!("{} transform keys", g.keys.len()));
                if self.frame >= g.start as f64 && self.frame <= g.end as f64 {
                    let mut key = g.at(self.frame);
                    key.frame = self.frame as u32;
                    let mut changed = false;
                    changed |= ui
                        .add(egui::DragValue::new(&mut key.position.x).prefix("X "))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut key.position.y).prefix("Y "))
                        .changed();
                    changed |=
                        theme::slider(ui, "Scale", egui::Slider::new(&mut key.scale, 0.01..=5.))
                            .changed();
                    let mut degrees = key.rotation.to_degrees();
                    if ui
                        .add(egui::DragValue::new(&mut degrees).prefix("Rotation ° "))
                        .changed()
                    {
                        key.rotation = degrees.to_radians();
                        changed = true;
                    }
                    changed |=
                        theme::slider(ui, "Opacity", egui::Slider::new(&mut key.opacity, 0.0..=1.))
                            .changed();
                    let mut easing = match key.easing {
                        Easing::Flow => 4,
                        Easing::Linear => 0,
                        Easing::Smooth => 1,
                        Easing::Hold => 2,
                        Easing::Bezier { .. } => 3,
                    };
                    let previous = easing;
                    egui::ComboBox::from_id_salt("graphic.easing")
                        .selected_text(
                            ["Linear", "Smooth", "Hold", "Custom Bezier", "Flow"][easing],
                        )
                        .show_ui(ui, |ui| {
                            for (i, name) in ["Linear", "Smooth", "Hold", "Custom Bezier", "Flow"]
                                .into_iter()
                                .enumerate()
                            {
                                ui.selectable_value(&mut easing, i, name);
                            }
                        });
                    if easing != previous {
                        key.easing = match easing {
                            0 => Easing::Linear,
                            1 => Easing::Smooth,
                            2 => Easing::Hold,
                            4 => Easing::Flow,
                            _ => Easing::Bezier {
                                x1: 0.25,
                                y1: 0.,
                                x2: 0.75,
                                y2: 1.,
                            },
                        };
                        changed = true;
                    }
                    if let Easing::Bezier { x1, y1, x2, y2 } = &mut key.easing {
                        changed |=
                            theme::slider(ui, "Ease X1", egui::Slider::new(x1, 0.0..=1.)).changed();
                        changed |= theme::slider(ui, "Ease Y1", egui::Slider::new(y1, -2.0..=3.))
                            .changed();
                        changed |=
                            theme::slider(ui, "Ease X2", egui::Slider::new(x2, 0.0..=1.)).changed();
                        changed |= theme::slider(ui, "Ease Y2", egui::Slider::new(y2, -2.0..=3.))
                            .changed();
                    }
                    if changed || ui.button(i18n::tr("Key this transform")).clicked() {
                        self.apply(vec![Command::SetGraphicKey { graphic: g.id, key }]);
                    }
                } else {
                    ui.label(i18n::tr(
                        "Move the playhead into this graphic to add transform keys.",
                    ));
                }
                let mut next = g.clone();
                let mut changed = false;
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.size.x)
                            .range(1.0..=8192.)
                            .prefix("Width "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut next.size.y)
                            .range(1.0..=8192.)
                            .prefix("Height "),
                    )
                    .changed();
                let mut foreground = next.layer == GraphicLayer::Foreground;
                if ui
                    .checkbox(&mut foreground, i18n::tr("Foreground layer"))
                    .changed()
                {
                    next.layer = if foreground {
                        GraphicLayer::Foreground
                    } else {
                        GraphicLayer::Background
                    };
                    changed = true;
                }
                changed |= ui
                    .color_edit_button_srgba_unmultiplied(&mut next.color)
                    .changed();
                changed |= ui
                    .checkbox(&mut next.draw_on, i18n::tr("Draw-on outline"))
                    .changed();
                if g.kind == GraphicKind::LowerThird {
                    changed |= ui.text_edit_singleline(&mut next.text).changed();
                }
                if changed {
                    self.apply(vec![Command::ReplaceGraphic { graphic: next }]);
                }
                if theme::icon_button(ui, "Delete graphic", theme::Icon::Delete, false).clicked() {
                    self.apply(vec![Command::Remove { id: g.id }]);
                    self.selected_graphic = None;
                }
            }
            ui.separator();
        }
    }
    pub(super) fn edit_stroke_on_stage(
        &mut self,
        p: &Project,
        painter: &egui::Painter,
        response: &egui::Response,
        to_screen: &dyn Fn(Point) -> Pos2,
        to_world: &dyn Fn(Pos2) -> Point,
    ) {
        let Some(d) = self
            .selected_stroke
            .and_then(|id| p.drawings.iter().find(|d| d.id == id))
        else {
            return;
        };
        if self.frame < d.start as f64 || self.frame > d.end as f64 {
            return;
        }
        let step = d.points.len().div_ceil(200).max(1);
        for point in d.points.iter().step_by(step) {
            painter.circle_filled(to_screen(*point), 3., VIOLET);
        }
        if response.drag_started()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let nearest = d
                .points
                .iter()
                .enumerate()
                .step_by(step)
                .map(|(i, q)| (i, pos.distance(to_screen(*q))))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((index, distance)) = nearest
                && distance < 20.
            {
                self.stop();
                self.stroke = d.points.clone();
                self.drag_joint = Some(index);
            }
        }
        if response.dragged()
            && let (Some(index), Some(pos)) = (self.drag_joint, response.interact_pointer_pos())
            && let Some(point) = self.stroke.get_mut(index)
        {
            *point = to_world(pos);
        }
        if !self.stroke.is_empty() {
            painter.add(egui::Shape::line(
                self.stroke.iter().map(|q| to_screen(*q)).collect(),
                Stroke::new(2., Color32::WHITE),
            ));
        }
        if response.drag_stopped() {
            if self.drag_joint.take().is_some() && !self.stroke.is_empty() {
                let mut next = d.clone();
                next.points = std::mem::take(&mut self.stroke);
                self.apply(vec![Command::ReplaceDrawing { drawing: next }]);
            }
            self.stroke.clear();
        }
    }
}
fn preset_name(preset: GraphicPreset) -> &'static str {
    match preset {
        GraphicPreset::Hold => "Hold",
        GraphicPreset::Drift => "Cloud drift",
        GraphicPreset::Slide => "Slide & fade",
        GraphicPreset::Pop => "Pop / overshoot",
        GraphicPreset::Spin => "Full rotation",
        GraphicPreset::DrawOn => "Draw-on outline",
    }
}

fn text_format_controls(ui: &mut egui::Ui, format: &mut TextFormatting) {
    egui::ComboBox::from_id_salt("text.font")
        .selected_text(format!("{:?}", format.font))
        .show_ui(ui, |ui| {
            for (font, label) in [
                (TextFont::Regular, "Regular"),
                (TextFont::Bold, "Bold"),
                (TextFont::Italic, "Italic"),
                (TextFont::BoldItalic, "Bold italic"),
            ] {
                ui.selectable_value(&mut format.font, font, label);
            }
        });
    ui.horizontal_wrapped(|ui| {
        ui.label(i18n::tr("Align"));
        for (align, label) in [
            (TextAlign::Left, "Left"),
            (TextAlign::Center, "Center"),
            (TextAlign::Right, "Right"),
        ] {
            ui.selectable_value(&mut format.align, align, label);
        }
    });
    theme::slider(
        ui,
        "Line spacing",
        egui::Slider::new(&mut format.line_height, 1.0..=2.5),
    );
    theme::slider(
        ui,
        "Padding",
        egui::Slider::new(&mut format.padding, 0.0..=50.),
    );
}

impl Studio {
    fn vector_path_tools(&mut self, ui: &mut egui::Ui, p: &Project) {
        ui.collapsing(i18n::tr("Vector paths"),|ui|{
   let id=egui::Id::unique("vector.boolean.selection");let mut state=ui.ctx().data_mut(|d|d.get_temp::<(Option<Id>,Option<Id>,usize)>(id).unwrap_or_default());
   for(label,value)in[("First",&mut state.0),("Second",&mut state.1)]{egui::ComboBox::from_id_salt(label).selected_text(format!("{label}: {}",value.and_then(|id|p.graphics.iter().find(|g|g.id==id)).map_or("Choose shape",|g|g.name.as_str()))).show_ui(ui,|ui|{for g in &p.graphics{ui.selectable_value(value,Some(g.id),&g.name);}});}
   let labels=["Union","Subtract","Intersect","XOR"];state.2=state.2.min(3);egui::ComboBox::from_id_salt("vector.boolean.operation").selected_text(labels[state.2]).show_ui(ui,|ui|{for(i,label)in labels.into_iter().enumerate(){ui.selectable_value(&mut state.2,i,label);}});
   if ui.add_enabled(state.0.is_some()&&state.1.is_some()&&state.0!=state.1,egui::Button::new(i18n::tr("Create combined shape"))).clicked(){let operation=[astral_core::vector::BooleanOperation::Union,astral_core::vector::BooleanOperation::Subtract,astral_core::vector::BooleanOperation::Intersect,astral_core::vector::BooleanOperation::Xor][state.2];self.apply(vec![Command::BooleanGraphics{left:state.0.unwrap(),right:state.1.unwrap(),frame:self.frame as u32,operation}]);}
   ui.ctx().data_mut(|d|d.insert_temp(id,state));
   if let Some(source)=self.selected_graphic.and_then(|id|p.graphics.iter().find(|g|g.id==id)){
    if source.contours.is_empty(){if ui.button(i18n::tr("Make path editable")).clicked(){match astral_core::vector::contours(source){Ok(contours)=>{let mut graphic=source.clone();graphic.contours=contours;self.apply(vec![Command::ReplaceGraphic{graphic}]);},Err(e)=>self.status=e.to_string()}}}
    else{let mut graphic=source.clone();let selection=egui::Id::unique((graphic.id,"point.selection"));let(mut ring,mut point)=ui.ctx().data_mut(|d|d.get_temp::<(usize,usize)>(selection).unwrap_or_default());ring=ring.min(graphic.contours.len()-1);point=point.min(graphic.contours[ring].len()-1);ui.add(egui::DragValue::new(&mut ring).range(0..=graphic.contours.len()-1).prefix("Contour "));point=point.min(graphic.contours[ring].len()-1);ui.add(egui::DragValue::new(&mut point).range(0..=graphic.contours[ring].len()-1).prefix("Point "));ui.ctx().data_mut(|d|d.insert_temp(selection,(ring,point)));let q=&mut graphic.contours[ring][point];let mut changed=ui.add(egui::DragValue::new(&mut q.x).speed(0.25).prefix("X ")).changed();changed|=ui.add(egui::DragValue::new(&mut q.y).speed(0.25).prefix("Y ")).changed();if changed{self.apply(vec![Command::ReplaceGraphic{graphic}]);}ui.small("Path edits change the shared shape across its animation. Originals remain after Boolean operations.");}
   }
  });
    }
}
