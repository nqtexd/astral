use super::*;
use astral_core::storyboard::*;

#[derive(Default)]
pub(super) struct BoardState {
    context: Option<(Id, u64)>,
    draft: Storyboard,
    selected: usize,
    cast_tab: bool,
    pub(super) dirty: bool,
    review: String,
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, multiline: bool) {
    ui.label(RichText::new(label).size(11.).color(MUTED));
    if multiline {
        ui.add(
            egui::TextEdit::multiline(value)
                .desired_width(f32::INFINITY)
                .desired_rows(2),
        );
    } else {
        ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
    }
    ui.add_space(4.);
}
impl Studio {
    pub(super) fn flush_storyboard(&mut self) -> bool {
        if !self.board.dirty {
            return true;
        }
        let Some((project, revision)) = self.board.context else {
            return true;
        };
        match self.service.request(Request::ApplyCommands {
            batch: Batch {
                project,
                expected_revision: revision,
                commands: vec![Command::SetStoryboard {
                    board: self.board.draft.clone(),
                }],
            },
        }) {
            Ok(v) => {
                self.board.context =
                    Some((project, v["revision"].as_u64().unwrap_or(revision + 1)));
                self.board.dirty = false;
                true
            }
            Err(e) => {
                self.status =
                    format!("Storyboard not saved: {e}. Resolve the draft or reload it first.");
                false
            }
        }
    }
    pub(super) fn storyboard_workspace(&mut self, ui: &mut egui::Ui, p: &Project) {
        self.stop();
        if self.board.context.is_none_or(|c| c.0 != p.id)
            || (!self.board.dirty && self.board.context != Some((p.id, p.revision)))
        {
            self.board.draft = read(p).unwrap_or_default();
            self.board.context = Some((p.id, p.revision));
            self.board.dirty = false;
        }
        let before = serde_json::to_value(&self.board.draft).unwrap_or_default();
        egui::Panel::top("storyboard.toolbar")
            .exact_size(58.)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(14.);
                    ui.heading("Storyboard");
                    ui.add_space(12.);
                    if ui.selectable_label(!self.board.cast_tab, "Shots").clicked() {
                        self.board.cast_tab = false;
                        self.board.selected = 0;
                    }
                    if ui
                        .selectable_label(self.board.cast_tab, "Characters")
                        .clicked()
                    {
                        self.board.cast_tab = true;
                        self.board.selected = 0;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::button(ui, "Apply plan", theme::Icon::Check, true)
                            .on_hover_text(
                                "Save this plan to the document. Ctrl+S saves the project to disk.",
                            )
                            .clicked()
                        {
                            let expected = self.board.context.unwrap().1;
                            match self.service.request(Request::ApplyCommands {
                                batch: Batch {
                                    project: p.id,
                                    expected_revision: expected,
                                    commands: vec![Command::SetStoryboard {
                                        board: self.board.draft.clone(),
                                    }],
                                },
                            }) {
                                Ok(v) => {
                                    self.board.context = Some((
                                        p.id,
                                        v["revision"].as_u64().unwrap_or(p.revision + 1),
                                    ));
                                    self.board.dirty = false;
                                    self.status =
                                        "Storyboard applied · save the project to keep it on disk"
                                            .into();
                                }
                                Err(e) => self.status = format!("Plan not applied: {e}"),
                            }
                        }
                        if theme::icon_button(ui, "Reload saved plan", theme::Icon::Undo, false)
                            .clicked()
                        {
                            self.board.context = None;
                            self.board.dirty = false;
                        }
                        ui.label(
                            RichText::new(if self.board.dirty {
                                "Draft"
                            } else {
                                "In project"
                            })
                            .small()
                            .color(MUTED),
                        );
                    });
                });
            });
        egui::Panel::left("storyboard.list")
            .exact_size(228.)
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        let label = if self.board.cast_tab {
                            "Add character"
                        } else {
                            "Add shot"
                        };
                        if theme::button(ui, label, theme::Icon::Plus, false).clicked() {
                            if self.board.cast_tab {
                                self.board.draft.characters.push(StoryCharacter {
                                    id: Id::new_v4(),
                                    name: format!(
                                        "Character {}",
                                        self.board.draft.characters.len() + 1
                                    ),
                                    role: String::new(),
                                    appearance: String::new(),
                                    motivation: String::new(),
                                    rig: None,
                                });
                                self.board.selected = self.board.draft.characters.len() - 1;
                            } else {
                                let start = self
                                    .board
                                    .draft
                                    .shots
                                    .last()
                                    .map_or(0, |s| s.end.saturating_add(1));
                                if start < p.duration {
                                    self.board.draft.shots.push(StoryShot {
                                        id: Id::new_v4(),
                                        title: format!("Shot {}", self.board.draft.shots.len() + 1),
                                        start,
                                        end: (start + p.fps * 4 - 1).min(p.duration - 1),
                                        location: String::new(),
                                        action: String::new(),
                                        camera: String::new(),
                                        audio: String::new(),
                                        transition: String::new(),
                                        cast: vec![],
                                    });
                                    self.board.selected = self.board.draft.shots.len() - 1;
                                } else {
                                    self.status =
                                        "Extend the project duration before adding another shot"
                                            .into();
                                }
                            }
                        }
                        ui.add_space(12.);
                        egui::ScrollArea::vertical()
                            .id_salt("storyboard.list.scroll")
                            .show(ui, |ui| {
                                if self.board.cast_tab {
                                    for (i, c) in self.board.draft.characters.iter().enumerate() {
                                        ui.push_id(c.id, |ui| {
                                            if ui
                                                .add_sized(
                                                    [ui.available_width(), 42.],
                                                    egui::Button::selectable(
                                                        self.board.selected == i,
                                                        &c.name,
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.board.selected = i;
                                            }
                                        });
                                    }
                                } else {
                                    for (i, s) in self.board.draft.shots.iter().enumerate() {
                                        ui.push_id(s.id, |ui| {
                                            let text = format!(
                                                "{:02}  {}\n{:.1}–{:.1}s",
                                                i + 1,
                                                s.title,
                                                s.start as f32 / p.fps as f32,
                                                (s.end + 1) as f32 / p.fps as f32
                                            );
                                            if ui
                                                .add_sized(
                                                    [ui.available_width(), 60.],
                                                    egui::Button::selectable(
                                                        self.board.selected == i,
                                                        text,
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.board.selected = i;
                                            }
                                        });
                                    }
                                }
                            });
                    });
            });
        egui::CentralPanel::default().show(ui,|ui| {
            egui::ScrollArea::vertical().id_salt("storyboard.detail.scroll").show(ui,|ui| {
                let width=ui.available_width().min(760.);
                ui.set_max_width(width);
                egui::Frame::new().inner_margin(egui::Margin::same(20)).show(ui,|ui| {
                    field(ui,"STORY",&mut self.board.draft.logline,true);
                    ui.add_space(12.);
                    if self.board.cast_tab {
                        let index=self.board.selected;
                        if let Some(c)=self.board.draft.characters.get_mut(index) {
                            field(ui,"Name",&mut c.name,false);field(ui,"Role",&mut c.role,false);
                            field(ui,"Appearance · keep this consistent",&mut c.appearance,true);
                            field(ui,"Motivation",&mut c.motivation,true);
                            egui::ComboBox::from_id_salt("storyboard.rig").selected_text(c.rig.and_then(|id|p.character(id)).map_or("No rig assigned",|c|c.name.as_str())).show_ui(ui,|ui|{
                                ui.selectable_value(&mut c.rig,None,"No rig assigned");
                                for rig in &p.characters{ui.selectable_value(&mut c.rig,Some(rig.id),&rig.name);}
                            });
                            ui.add_space(18.);
                            if theme::button(ui,"Remove character",theme::Icon::Delete,false).clicked(){let id=c.id;self.board.draft.characters.remove(index);for s in &mut self.board.draft.shots{s.cast.retain(|c|c.character!=id);}}
                        } else {ui.heading("Define your cast");ui.label(RichText::new("Add a character, describe their appearance and goal, then assign a rig when you are ready.").color(MUTED));}
                    } else {
                        let cast=self.board.draft.characters.clone();
                        let index=self.board.selected;
                        let mut delete_shot=false;
                        if let Some(shot)=self.board.draft.shots.get_mut(index) {
                            field(ui,"Shot",&mut shot.title,false);
                            ui.horizontal_wrapped(|ui|{
                                if theme::button(ui,"Animate shot",theme::Icon::Play,false).clicked(){let frame=shot.start;self.frame=frame as f64;self.preferences.mode=WorkspaceMode::Animate;}
                                if theme::button(ui,"Review shot",theme::Icon::Check,false).clicked(){self.board.review=match astral_core::staging::review(p,shot.start,shot.end,(p.fps/10).max(1)){Ok(r)=>format!("{} missed contacts · {} torso overlaps · {} layout warnings · {} samples",r.contacts.iter().filter(|c|!c.passed).count(),r.overlaps.len(),r.layout.len(),r.sampled_frames),Err(e)=>e.to_string()};}
                                if theme::icon_button(ui,"Delete shot",theme::Icon::Delete,false).clicked(){delete_shot=true;}
                            });
                            ui.add_space(16.);
                            let preview_height=(ui.clip_rect().height()*0.32).clamp(100.,220.);
                            let preview_width=ui.available_width().min(preview_height*16./9.);
                            let (rect,_)=ui.allocate_exact_size(Vec2::new(preview_width,preview_width*9./16.),Sense::hover());
                            let list=astral_render::display_list(p,shot.start.min(p.duration-1) as f64);
                            // Fit the rendered project, including non-16:9 documents, into the card.
                            let scale=(rect.width()/p.width as f32).min(rect.height()/p.height as f32);
                            let framed=Rect::from_center_size(rect.center(),Vec2::new(p.width as f32*scale,p.height as f32*scale));
                            ui.painter().rect_filled(rect,10.,theme::BACKGROUND);
                            paint_list(&ui.painter().with_clip_rect(rect),&list,framed,scale,None);


                            ui.horizontal(|ui|{
                                ui.label("Frames");ui.add(egui::DragValue::new(&mut shot.start).range(0..=p.duration-1));ui.label("to");ui.add(egui::DragValue::new(&mut shot.end).range(0..=p.duration-1));
                                ui.label(RichText::new(format!("{:.1}s",shot.end.saturating_sub(shot.start).saturating_add(1)as f32/p.fps as f32)).color(MUTED));
                            });
                            field(ui,"Location",&mut shot.location,false);field(ui,"Action · what changes in this shot?",&mut shot.action,true);
                            field(ui,"Camera",&mut shot.camera,false);field(ui,"Sound / dialogue",&mut shot.audio,true);
                            field(ui,"Transition · explain a new location, prop, injury or reversal",&mut shot.transition,true);
                            ui.add_space(8.);ui.heading("Blocking & continuity");
                            for c in &cast {
                                ui.push_id(c.id,|ui|{
                                    let mut included=shot.cast.iter().any(|v|v.character==c.id);
                                    if ui.checkbox(&mut included,&c.name).changed(){if included{let state=CharacterState{facing:1,..Default::default()};shot.cast.push(ShotCharacter{character:c.id,entry:state.clone(),exit:state});}else{shot.cast.retain(|v|v.character!=c.id);}}
                                    if let Some(actor)=shot.cast.iter_mut().find(|v|v.character==c.id){
                                        egui::CollapsingHeader::new("Entry / exit state").show(ui,|ui|{
                                            for (label,state) in [("Entry",&mut actor.entry),("Exit",&mut actor.exit)]{
                                                ui.label(RichText::new(label).strong());
                                                ui.horizontal_wrapped(|ui|{ui.label("X");ui.add(egui::DragValue::new(&mut state.position.x));ui.label("Y");ui.add(egui::DragValue::new(&mut state.position.y));ui.selectable_value(&mut state.facing,-1,"Left");ui.selectable_value(&mut state.facing,1,"Right");});
                                                field(ui,"Condition",&mut state.condition,false);
                                                let mut props=state.props.join(", ");
                                                ui.label(RichText::new("Props · comma separated").small().color(MUTED));
                                                if ui.add(egui::TextEdit::singleline(&mut props).desired_width(f32::INFINITY)).changed(){state.props=props.split(',').map(str::trim).filter(|s|!s.is_empty()).map(str::to_string).collect();}
                                            }
                                        });
                                    }
                                });
                            }
                        } else {ui.heading("Plan the film before animating");ui.label(RichText::new("Define your characters, then add shots with a clear action, location and continuity. Each shot opens at its exact timeline frame.").color(MUTED));}
                        if delete_shot {self.board.draft.shots.remove(index);}
                        if !self.board.review.is_empty(){ui.label(&self.board.review);}
                    }
                    ui.add_space(20.);
                    let issues=analyze(&self.board.draft);
                    ui.label(RichText::new(format!("Continuity · {} notes",issues.len())).strong());
                    for issue in issues.iter().take(16){ui.label(RichText::new(&issue.message).small().color(MUTED));}
                    if let Err(e)=validate(p,&self.board.draft){ui.colored_label(Color32::from_rgb(242,168,118),e.to_string());}
                });
            });
        });
        if before != serde_json::to_value(&self.board.draft).unwrap_or_default() {
            self.board.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storyboard_layout_and_draft_save_fit_compact_windows() {
        for size in [[800., 620.], [960., 620.], [1280., 800.]] {
            let (_dir, mut studio, _) = super::super::tests::studio();
            let p = studio.service.snapshot(studio.project).unwrap();
            studio.board.context = Some((p.id, p.revision));
            studio.board.draft.logline = "A survivor reaches the east gate.".into();
            studio.board.draft.shots.push(StoryShot {
                id: Id::new_v4(),
                title: "East gate".into(),
                start: 0,
                end: 23,
                location: "Yard".into(),
                action: "Open the gate".into(),
                camera: "Wide".into(),
                audio: "Bell".into(),
                transition: String::new(),
                cast: vec![],
            });
            studio.board.dirty = true;
            assert!(studio.flush_storyboard());
            assert_eq!(
                read(&studio.service.snapshot(p.id).unwrap())
                    .unwrap()
                    .shots
                    .len(),
                1
            );
            let ctx = studio.context.clone();
            theme::install(&ctx);
            ctx.enable_accesskit();
            for cast_tab in [false, true] {
                studio.board.cast_tab = cast_tab;
                for _ in 0..3 {
                    let mut output = ctx.run_pass(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(size[0], size[1]),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            let mut ui = egui::Ui::new(
                                ctx.clone(),
                                egui::Id::unique("board-test"),
                                egui::UiBuilder::new().max_rect(ctx.content_rect()),
                            );
                            let p = studio.service.snapshot(p.id).unwrap();
                            studio.storyboard_workspace(&mut ui, &p);
                        },
                    );
                    output.textures_delta.clear();
                    for (_, node) in output.platform_output.accesskit_update.unwrap().nodes {
                        if let Some(r) = node.bounds()
                            && matches!(
                                format!("{:?}", node.role()).as_str(),
                                "Button" | "TextInput" | "SpinButton" | "ComboBox"
                            )
                        {
                            assert!(
                                r.x0 >= -0.5 && r.x1 <= size[0] as f64 + 0.5,
                                "Control overflows {:?}: {:?} {:?}",
                                size,
                                node.label(),
                                r
                            );
                        }
                    }
                }
            }
        }
    }
}
