use super::*;
use crate::production::ProductionRequest;
use astral_core::studio::*;
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Pose,
    Motion,
    Fight,
    Coach,
    Frames,
    Library,
    Finish,
    Camera,
}
struct Proposed {
    project: Id,
    revision: u64,
    plan: StudioPlan,
    before: Project,
    after: Project,
    frame: f64,
    playing: bool,
    tick: Instant,
}
struct Reply {
    project: Id,
    result: anyhow::Result<Value>,
    clipboard: bool,
}
#[derive(Clone)]
pub(super) struct Wave {
    pub peaks: Vec<f32>,
    pub duration: f64,
}
pub(super) struct MotionState {
    pub open: bool,
    pub tab: Tab,
    pub draw_path: bool,
    pub path: Vec<Point>,
    pub start: u32,
    pub end: u32,
    pub selection: Option<(u32, u32)>,
    pub anchor: Option<u32>,
    pub clipboard: Option<LibraryEntry>,
    pub last_drag_constrained: bool,
    context: Option<Id>,
    joint: String,
    style: MovementStyle,
    easing: Easing,
    keys: u32,
    seconds: f32,
    first: Option<Id>,
    second: Option<Id>,
    beats: Vec<FightBeat>,
    impact: bool,
    overwrite: bool,
    frame_action: FrameEdit,
    target: u32,
    delta: Point,
    pose_scope: bool,
    name: String,
    kind: LibraryKind,
    library_path: String,
    selected_entry: Option<Id>,
    pub dragging_entry: Option<Id>,
    template: RigTemplate,
    rig_height: f32,
    pack: ImpactPack,
    sound: Option<Id>,
    sound_seconds: f64,
    pub selected_audio: Option<Id>,
    audio_edit: Option<AudioClip>,
    pub selected_effect: Option<Id>,
    effect_edit: Option<Effect>,
    pub waves: BTreeMap<Id, Wave>,
    wave_pending: Option<std::sync::mpsc::Receiver<(Id, anyhow::Result<Wave>)>>,
    pending: Option<std::sync::mpsc::Receiver<Reply>>,
    proposal: Option<Proposed>,
    issues: Vec<MotionIssue>,
    issue_revision: u64,
    foot_left: bool,
    foot_right: bool,
    auto_feet: bool,
    tolerance: f32,
    mirror_insert: bool,
    follow: bool,
    from: Point,
    to: Point,
    zoom_start: f32,
    zoom_end: f32,
    marker: String,
    ghost_key: Option<(Id, u64, Id, String, u32, u32)>,
    ghost: Vec<TimedPoint>,
}
impl Default for MotionState {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Pose,
            draw_path: false,
            path: vec![],
            start: 0,
            end: 24,
            selection: None,
            anchor: None,
            clipboard: None,
            last_drag_constrained: false,
            context: None,
            joint: "right_hand".into(),
            style: MovementStyle::Natural,
            easing: Easing::Smooth,
            keys: 25,
            seconds: 1.,
            first: None,
            second: None,
            beats: vec![],
            impact: true,
            overwrite: false,
            frame_action: FrameEdit::Copy,
            target: 25,
            delta: Point::default(),
            pose_scope: false,
            name: "New motion".into(),
            kind: LibraryKind::Motion,
            library_path: "library/motions.astral-library.json".into(),
            selected_entry: None,
            dragging_entry: None,
            template: RigTemplate::Human,
            rig_height: 220.,
            pack: ImpactPack::Light,
            sound: None,
            sound_seconds: 0.3,
            selected_audio: None,
            audio_edit: None,
            selected_effect: None,
            effect_edit: None,
            waves: BTreeMap::new(),
            wave_pending: None,
            pending: None,
            proposal: None,
            issues: vec![],
            issue_revision: 0,
            foot_left: true,
            foot_right: true,
            auto_feet: true,
            tolerance: 0.12,
            mirror_insert: false,
            follow: true,
            from: Point::new(0., -40.),
            to: Point::new(0., -40.),
            zoom_start: 1.,
            zoom_end: 1.2,
            marker: "Contact".into(),
            ghost_key: None,
            ghost: vec![],
        }
    }
}
impl Studio {
    fn motion_request(&mut self, request: ProductionRequest, clipboard: bool) {
        if self.motion.pending.is_some() {
            self.status = "A motion preview is already running".into();
            return;
        }
        let service = self.service.clone();
        let project = self.project;
        let (send, receive) = std::sync::mpsc::channel();
        self.motion.pending = Some(receive);
        std::thread::spawn(move || {
            let result = service.request(Request::Production { request });
            let _ = send.send(Reply {
                project,
                result,
                clipboard,
            });
        });
        self.context.request_repaint();
    }
    pub(super) fn motion_plan(&mut self, p: &Project, tool: StudioOperation, preview: u32) {
        self.motion_request(
            ProductionRequest::Edit {
                project: p.id,
                expected_revision: p.revision,
                tool,
                apply: false,
                preview_frame: Some(preview),
            },
            false,
        );
    }
    pub(super) fn motion_tick(&mut self, ctx: &egui::Context) {
        if self.motion.context != Some(self.project) {
            self.motion.context = Some(self.project);
            self.motion.proposal = None;
            self.motion.path.clear();
            self.motion.draw_path = false;
            self.motion.selection = None;
            self.motion.anchor = None;
            self.motion.issues.clear();
            self.motion.ghost_key = None;
            self.motion.first = self.selected;
            self.motion.second = None;
            self.motion.beats.clear();
            self.motion.selected_audio = None;
            self.motion.selected_effect = None;
        }
        if self.motion.proposal.as_ref().is_some_and(|p| {
            !self
                .service
                .snapshot(p.project)
                .is_ok_and(|s| s.revision == p.revision)
        }) {
            self.motion.proposal = None;
        }
        if self.motion.draw_path
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.motion.draw_path = false;
            self.motion.path.clear();
            self.status = "Motion path cancelled".into();
        }
        if let Some(receiver) = &self.motion.pending
            && let Ok(reply) = receiver.try_recv()
        {
            self.motion.pending = None;
            if reply.project == self.project {
                match reply.result {
                    Err(error) => self.status = format!("{error:#}"),
                    Ok(value) => {
                        if let Ok(plan) =
                            serde_json::from_value::<StudioPlan>(value["plan"].clone())
                        {
                            if reply.clipboard {
                                self.motion.clipboard = plan.commands.iter().find_map(|c| {
                                    if let Command::SetLibrary { entries } = c {
                                        entries.last().cloned()
                                    } else {
                                        None
                                    }
                                });
                                self.status = "Frame range copied".into();
                            } else if plan.commands.is_empty() {
                                self.motion.issues = plan.diagnostics;
                                self.motion.issue_revision =
                                    value["revision"].as_u64().unwrap_or(0);
                                self.status = if self.motion.issues.is_empty() {
                                    "No motion issues found in this range".into()
                                } else {
                                    format!(
                                        "{} possible issues to review",
                                        self.motion.issues.len()
                                    )
                                };
                            } else {
                                let revision = value["revision"].as_u64().unwrap_or(0);
                                if let Ok(before) = self.service.snapshot(self.project)
                                    && before.revision == revision
                                    && let Ok(mut session) = Session::new(before.clone())
                                    && session
                                        .apply(Batch {
                                            project: before.id,
                                            expected_revision: revision,
                                            commands: plan.commands.clone(),
                                        })
                                        .is_ok()
                                {
                                    let frame = value["preview_frame"]
                                        .as_u64()
                                        .unwrap_or(self.frame as u64)
                                        .min((session.project.duration - 1) as u64)
                                        as f64;
                                    self.motion.proposal = Some(Proposed {
                                        project: self.project,
                                        revision,
                                        before,
                                        after: session.project,
                                        plan,
                                        frame,
                                        playing: false,
                                        tick: Instant::now(),
                                    });
                                } else {
                                    self.status="The scene changed while preparing this suggestion. Preview it again.".into();
                                }
                            }
                        } else {
                            self.status = if let Some(n) = value["imported"].as_u64() {
                                format!("Imported {n} library items")
                            } else {
                                "Library exported".into()
                            };
                            if value["receipt"].is_object() {
                                self.dirty = true;
                            }
                        }
                    }
                }
            }
        }
        if self.motion.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
        if let Some(receiver) = &self.motion.wave_pending
            && let Ok((id, value)) = receiver.try_recv()
        {
            self.motion.wave_pending = None;
            match value {
                Ok(wave) => {
                    self.motion.waves.insert(id, wave);
                }
                Err(e) => self.status = format!("Waveform: {e:#}"),
            }
        }
        if self.motion.wave_pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
    }
    pub(super) fn motion_windows(&mut self, ctx: &egui::Context, p: &Project) {
        if self.motion.open && !self.manager.home {
            let mut open = true;
            egui::Window::new(i18n::tr("Motion studio"))
                .id(egui::Id::unique("astral.motion.v1"))
                .open(&mut open)
                .default_size(theme::dialog_size(ctx, Vec2::new(740., 640.)))
                .max_size(theme::dialog_size(ctx, Vec2::new(900., 760.)))
                .vscroll(true)
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (tab, label) in [
                            (Tab::Pose, "Pose"),
                            (Tab::Motion, "Motion"),
                            (Tab::Fight, "Fight"),
                            (Tab::Coach, "Coach"),
                            (Tab::Frames, "Frames"),
                            (Tab::Library, "Library"),
                            (Tab::Finish, "Finish"),
                            (Tab::Camera, "Camera"),
                        ] {
                            ui.selectable_value(&mut self.motion.tab, tab, label);
                        }
                    });
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        for (label, mode) in [
                            ("Auto Create", astral_core::director::AutoMode::Create),
                            ("Auto Improve", astral_core::director::AutoMode::Improve),
                            ("Auto Continue", astral_core::director::AutoMode::Continue),
                        ] {
                            if ui
                                .add_enabled(
                                    self.motion.pending.is_none(),
                                    egui::Button::new(label),
                                )
                                .clicked()
                            {
                                let start = self.frame as u32;
                                let end = if mode == astral_core::director::AutoMode::Continue {
                                    start + (p.fps * 2).min(1200) - 1
                                } else if mode == astral_core::director::AutoMode::Create {
                                    start + (p.fps * 6).min(960) - 1
                                } else {
                                    self.motion
                                        .end
                                        .min(p.duration - 1)
                                        .max(start + 1)
                                        .min(p.duration - 1)
                                };
                                self.motion_request(
                                    ProductionRequest::AutoAnimate {
                                        project: p.id,
                                        expected_revision: p.revision,
                                        spec: astral_core::director::AutoSpec {
                                            mode,
                                            start,
                                            end,
                                            characters: if mode
                                                == astral_core::director::AutoMode::Continue
                                            {
                                                self.selected.into_iter().collect()
                                            } else {
                                                vec![]
                                            },
                                            ..Default::default()
                                        },
                                        apply: false,
                                    },
                                    false,
                                );
                            }
                        }
                    });
                    match self.motion.tab {
                        Tab::Pose => self.motion_pose(ui, p),
                        Tab::Motion => self.motion_path_panel(ui, p),
                        Tab::Fight => self.motion_fight(ui, p),
                        Tab::Coach => self.motion_coach(ui, p),
                        Tab::Frames => self.motion_frames(ui, p),
                        Tab::Library => self.motion_library(ui, p),
                        Tab::Finish => self.motion_finish(ui, p),
                        Tab::Camera => self.motion_camera(ui, p),
                    }
                    if self.motion.pending.is_some() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(i18n::tr("Preparing preview…"));
                        });
                    }
                    ui.add(
                        egui::Label::new(RichText::new(&self.status).small().color(MUTED)).wrap(),
                    );
                });
            self.motion.open &= open;
        }
        if let Some(mut proposed) = self.motion.proposal.take() {
            let now = Instant::now();
            if proposed.playing {
                proposed.frame = (proposed.frame
                    + now.duration_since(proposed.tick).as_secs_f64().min(0.1)
                        * proposed.after.fps as f64)
                    % proposed.after.duration as f64;
                ctx.request_repaint_after(Duration::from_millis(16));
            }
            proposed.tick = now;
            let mut accept = false;
            let mut close = false;
            let mut open = true;
            egui::Window::new(i18n::tr("Review motion"))
                .id(egui::Id::unique("motion.review"))
                .open(&mut open)
                .default_size(theme::dialog_size(ctx, Vec2::new(640., 560.)))
                .max_size(theme::dialog_size(ctx, Vec2::new(850., 700.)))
                .show(ctx, |ui| {
                    ui.heading(&proposed.plan.name);
                    egui::ScrollArea::vertical()
                        .max_height(
                            (theme::dialog_size(ctx, Vec2::new(640., 560.)).y - 110.).max(120.),
                        )
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if theme::icon_button(
                                    ui,
                                    if proposed.playing {
                                        "Pause preview"
                                    } else {
                                        "Play preview"
                                    },
                                    if proposed.playing {
                                        theme::Icon::Pause
                                    } else {
                                        theme::Icon::Play
                                    },
                                    proposed.playing,
                                )
                                .clicked()
                                {
                                    proposed.playing = !proposed.playing;
                                    proposed.tick = Instant::now();
                                }
                                ui.add(
                                    egui::Slider::new(
                                        &mut proposed.frame,
                                        0.0..=(proposed.after.duration - 1) as f64,
                                    )
                                    .show_value(false),
                                );
                                ui.small(format!(
                                    "Visual preview · frame {}",
                                    proposed.frame as u32 + 1
                                ));
                            });
                            let width = (ui.available_width() - 12.) / 2.;
                            ui.horizontal_top(|ui| {
                                for (label, document, frame) in [
                                    (
                                        "Original",
                                        &proposed.before,
                                        proposed.frame / proposed.after.fps as f64
                                            * proposed.before.fps as f64,
                                    ),
                                    ("Suggested", &proposed.after, proposed.frame),
                                ] {
                                    ui.vertical(|ui| {
                                        ui.set_width(width);
                                        ui.label(RichText::new(label).small().color(MUTED));
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(width, width * 0.5625),
                                            Sense::hover(),
                                        );
                                        let scale = (rect.width() / document.width as f32)
                                            .min(rect.height() / document.height as f32);
                                        let art = Rect::from_center_size(
                                            rect.center(),
                                            Vec2::new(
                                                document.width as f32 * scale,
                                                document.height as f32 * scale,
                                            ),
                                        );
                                        let list = display_list(
                                            document,
                                            frame.min((document.duration - 1) as f64),
                                        );
                                        let painter = ui.painter_at(art);
                                        painter.rect_filled(art, 3., rgba(list.background));
                                        paint_list(&painter, &list, art, scale, None);
                                    });
                                }
                            });
                            let poses = proposed
                                .plan
                                .commands
                                .iter()
                                .filter(|c| matches!(c, Command::SetPose { .. }))
                                .count();
                            let effects = proposed
                                .plan
                                .commands
                                .iter()
                                .filter(|c| matches!(c, Command::AddEffect { .. }))
                                .count();
                            if poses + effects > 0 {
                                ui.label(format!("{poses} editable poses · {effects} effects"));
                            }
                            for warning in &proposed.plan.warnings {
                                ui.add(
                                    egui::Label::new(RichText::new(warning).small().color(MUTED))
                                        .wrap(),
                                );
                            }
                            if !proposed.plan.markers.is_empty() {
                                ui.collapsing(i18n::tr("Contact markers"), |ui| {
                                    for (name, frame) in &proposed.plan.markers {
                                        ui.label(format!("{name}: frame {}", frame + 1));
                                    }
                                });
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if theme::button(ui, "Accept", theme::Icon::Check, true).clicked() {
                            accept = true;
                        }
                        if ui.button(i18n::tr("Cancel")).clicked() {
                            close = true;
                        }
                    });
                });
            if accept {
                match self.service.request(Request::ApplyCommands {
                    batch: Batch {
                        project: proposed.project,
                        expected_revision: proposed.revision,
                        commands: proposed.plan.commands,
                    },
                }) {
                    Ok(value) => {
                        self.dirty = true;
                        self.status = "Motion applied · one undo step".into();
                        self.motion.issues.clear();
                        self.motion.draw_path = false;
                        self.motion.path.clear();
                        if let Some(ids) = value["created_ids"].as_array()
                            && let Ok(now) = self.service.snapshot(self.project)
                        {
                            for id in ids {
                                if let Ok(id) = serde_json::from_value::<Id>(id.clone()) {
                                    if now.character(id).is_some() {
                                        self.selected = Some(id);
                                    }
                                    if now.layers.iter().any(|l| l.id == id) {
                                        self.selected_layer = Some(id);
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => self.status = format!("{error:#}"),
                }
            } else if open && !close {
                self.motion.proposal = Some(proposed);
            }
        }
    }
    fn motion_actor(ui: &mut egui::Ui, p: &Project, id: &mut Option<Id>, salt: &str, label: &str) {
        if id.is_none_or(|id| p.character(id).is_none()) {
            *id = p.characters.first().map(|c| c.id);
        }
        ui.label(label);
        egui::ComboBox::from_id_salt(salt)
            .truncate()
            .selected_text(
                id.and_then(|id| p.character(id))
                    .map_or("Select a character", |c| c.name.as_str()),
            )
            .width(ui.available_width().min(320.))
            .show_ui(ui, |ui| {
                for c in &p.characters {
                    ui.selectable_value(id, Some(c.id), &c.name);
                }
            });
    }
    fn motion_range(&mut self, ui: &mut egui::Ui, p: &Project) {
        self.motion.start = self.motion.start.min(p.duration - 1);
        self.motion.end = self.motion.end.clamp(self.motion.start, p.duration - 1);
        ui.horizontal_wrapped(|ui| {
            ui.label(i18n::tr("Frames"));
            frame_field(ui, &mut self.motion.start, p.duration - 1);
            ui.label(i18n::tr("to"));
            frame_field(ui, &mut self.motion.end, p.duration - 1);
            ui.small(format!(
                "{:.2}s",
                (self.motion.end - self.motion.start + 1) as f32 / p.fps as f32
            ));
        });
    }
    fn motion_pose(&mut self, ui: &mut egui::Ui, p: &Project) {
        Self::motion_actor(ui, p, &mut self.selected, "motion.pose.actor", "Character");
        ui.checkbox(&mut self.preferences.smart_pose, i18n::tr("Smart posing"));
        ui.small(i18n::tr(
            "Drag a hand or foot to guide the limb. Hold Alt to move a joint manually.",
        ));
        style_picker(ui, &mut self.preferences.movement_style);
        ui.checkbox(
            &mut self.preferences.smart_onion,
            "Highlight changed joints",
        );
        ui.checkbox(
            &mut self.preferences.ghost_path,
            i18n::tr("Show ghost path"),
        );
        self.motion_range(ui, p);
        if let Some(character) = self.selected
            && ui.button(i18n::tr("Preview flowing motion")).on_hover_text("Continuous joint arcs through existing pose keys; preserves authored key poses").clicked() {
                self.motion_plan(p, StudioOperation::FlowMotion {character,start:self.motion.start,end:self.motion.end}, self.frame as u32);
            }
        if let Some(character)=self.selected && ui.button(i18n::tr("Preview motion polish")).on_hover_text("Proposes planted feet and subtle body follow-through while preserving authored poses").clicked(){self.motion_plan(p,StudioOperation::PolishMotion{character,start:self.motion.start,end:self.motion.end,strength:0.6},self.frame as u32);}
        if let Some(id) = self.selected {
            ui.horizontal_wrapped(|ui| {
                if theme::button(ui, "Save pose", theme::Icon::Save, false).clicked() {
                    self.motion_plan(
                        p,
                        StudioOperation::Commands {
                            name: "Save pose".into(),
                            commands: vec![Command::SavePose {
                                character: id,
                                frame: self.frame as u32,
                                name: self.motion.name.clone(),
                            }],
                        },
                        self.frame as u32,
                    );
                }
                if ui.button(i18n::tr("Mirror range")).clicked() {
                    self.motion_plan(
                        p,
                        StudioOperation::Mirror {
                            character: id,
                            start: self.motion.start,
                            end: self.motion.end,
                        },
                        self.frame as u32,
                    );
                }
            });
            ui.text_edit_singleline(&mut self.motion.name);
            for pose in &p.poses {
                ui.horizontal_wrapped(|ui| {
                    ui.label(&pose.name);
                    if ui.button(i18n::tr("Apply")).clicked() {
                        self.motion_plan(
                            p,
                            StudioOperation::Commands {
                                name: "Apply saved pose".into(),
                                commands: vec![Command::ApplyPose {
                                    character: id,
                                    pose: pose.id,
                                    frame: self.frame as u32,
                                    blend: 1.,
                                    joints: vec![],
                                    mirror: false,
                                }],
                            },
                            self.frame as u32,
                        );
                    }
                });
            }
            theme::section(ui, "PLANTED FEET");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.motion.foot_left, i18n::tr("Left"));
                ui.checkbox(&mut self.motion.foot_right, i18n::tr("Right"));
                ui.checkbox(&mut self.motion.auto_feet, i18n::tr("Detect stance ranges"));
            });
            if ui.button(i18n::tr("Preview foot lock")).clicked() {
                let joints = [
                    (self.motion.foot_left, "left_foot"),
                    (self.motion.foot_right, "right_foot"),
                ]
                .into_iter()
                .filter(|(selected, _)| *selected)
                .map(|(_, name)| name.into())
                .collect();
                self.motion_plan(
                    p,
                    StudioOperation::FootLock {
                        character: id,
                        start: self.motion.start,
                        end: self.motion.end,
                        joints,
                        automatic: self.motion.auto_feet,
                    },
                    self.motion.start,
                );
            }
            if p.character(id).is_some_and(|c| !c.contacts.is_empty())
                && ui.button(i18n::tr("Clear foot contacts")).clicked()
            {
                self.motion_plan(
                    p,
                    StudioOperation::Commands {
                        name: "Clear foot contacts".into(),
                        commands: vec![Command::ClearContacts { character: id }],
                    },
                    self.frame as u32,
                );
            }
        }
        ui.separator();
        if ui.button(i18n::tr("Frame completion tools")).clicked() {
            self.assist.panel_open = true;
        }
    }
    fn motion_path_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        Self::motion_actor(ui, p, &mut self.selected, "motion.path.actor", "Character");
        if let Some(c) = self.selected.and_then(|id| p.character(id)) {
            egui::ComboBox::from_id_salt("motion.path.control")
                .truncate()
                .selected_text(&self.motion.joint)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.motion.joint, "root".into(), "Whole character");
                    for j in &c.joints {
                        ui.selectable_value(
                            &mut self.motion.joint,
                            j.name.clone(),
                            j.name.replace('_', " "),
                        );
                    }
                });
        }
        self.motion_range(ui, p);
        curve_editor(ui, &mut self.motion.easing);
        ui.horizontal_wrapped(|ui| {
            ui.label(i18n::tr("Key poses"));
            ui.add(egui::DragValue::new(&mut self.motion.keys).range(2..=1201));
        });
        style_picker(ui, &mut self.motion.style);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.selected.is_some(),
                    egui::Button::new("Draw motion path"),
                )
                .clicked()
            {
                self.motion.path.clear();
                self.motion.draw_path = true;
                self.preferences.mode = WorkspaceMode::Rig;
                self.motion.open = false;
                self.status = "Drag on the canvas to draw a path · Esc cancels".into();
            }
            if ui.button(i18n::tr("Clear path")).clicked() {
                self.motion.path.clear();
            }
        });
        ui.small(format!("{} path points", self.motion.path.len()));
        if ui
            .add_enabled(
                self.motion.path.len() >= 2 && self.selected.is_some(),
                egui::Button::new("Preview motion"),
            )
            .clicked()
        {
            self.motion_plan(
                p,
                StudioOperation::MotionBrush {
                    character: self.selected.unwrap(),
                    joint: self.motion.joint.clone(),
                    path: self.motion.path.clone(),
                    start: self.motion.start,
                    end: self.motion.end,
                    keys: self.motion.keys,
                    easing: self.motion.easing.clone(),
                    style: self.motion.style,
                },
                self.motion.start,
            );
        }
        theme::section(ui, "POSE CURVE");
        ui.small(i18n::tr(
            "Change the timing between existing pose keys without redrawing them.",
        ));
        if let Some(character) = self.selected
            && ui
                .button(i18n::tr("Preview curve on selected range"))
                .clicked()
        {
            self.motion_plan(
                p,
                StudioOperation::Tween {
                    character,
                    start: self.motion.start,
                    end: self.motion.end,
                    easing: self.motion.easing.clone(),
                    preserve_bones: true,
                },
                self.motion.start,
            );
        }
    }
    fn motion_fight(&mut self, ui: &mut egui::Ui, p: &Project) {
        Self::motion_actor(
            ui,
            p,
            &mut self.motion.first,
            "fight.first",
            "First character",
        );
        if self.motion.second == self.motion.first {
            self.motion.second = p
                .characters
                .iter()
                .find(|c| Some(c.id) != self.motion.first)
                .map(|c| c.id);
        }
        Self::motion_actor(
            ui,
            p,
            &mut self.motion.second,
            "fight.second",
            "Second character",
        );
        style_picker(ui, &mut self.motion.style);
        ui.horizontal_wrapped(|ui| {
            ui.label(i18n::tr("Start"));
            frame_field(ui, &mut self.motion.start, p.duration - 1);
            ui.checkbox(&mut self.motion.impact, i18n::tr("Impact accents"));
            ui.checkbox(
                &mut self.motion.overwrite,
                i18n::tr("Replace poses in range"),
            );
        });
        let mut remove = None;
        for i in 0..self.motion.beats.len() {
            ui.push_id(i, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let beat = &mut self.motion.beats[i];
                    egui::ComboBox::from_id_salt("actor")
                        .truncate()
                        .selected_text(
                            p.character(beat.attacker)
                                .map_or("Actor", |c| c.name.as_str()),
                        )
                        .width(130.)
                        .show_ui(ui, |ui| {
                            for id in [self.motion.first, self.motion.second]
                                .into_iter()
                                .flatten()
                            {
                                if let Some(c) = p.character(id) {
                                    ui.selectable_value(&mut beat.attacker, id, &c.name);
                                }
                            }
                        });
                    egui::ComboBox::from_id_salt("beat")
                        .truncate()
                        .selected_text(format!("{:?}", beat.action))
                        .width(100.)
                        .show_ui(ui, |ui| {
                            for action in [
                                FightAction::Punch,
                                FightAction::Dodge,
                                FightAction::Counter,
                                FightAction::Kick,
                                FightAction::Knockback,
                            ] {
                                ui.selectable_value(
                                    &mut beat.action,
                                    action,
                                    format!("{action:?}"),
                                );
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut beat.frames)
                            .range(6..=240)
                            .suffix(" fr"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut beat.strength)
                            .range(0.1..=3.)
                            .speed(0.05)
                            .prefix("Power "),
                    );
                    if theme::icon_button(ui, "Remove beat", theme::Icon::Close, false).clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            self.motion.beats.remove(i);
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.motion.beats.len() < 32 && self.motion.first.is_some(),
                    egui::Button::new("Add beat"),
                )
                .clicked()
            {
                self.motion.beats.push(FightBeat {
                    attacker: self.motion.first.unwrap(),
                    action: FightAction::Punch,
                    frames: 24,
                    strength: 1.,
                });
            }
            if ui.button(i18n::tr("Starter combo")).clicked()
                && let (Some(a), Some(b)) = (self.motion.first, self.motion.second)
            {
                self.motion.beats = vec![
                    FightBeat {
                        attacker: a,
                        action: FightAction::Punch,
                        frames: 24,
                        strength: 1.,
                    },
                    FightBeat {
                        attacker: b,
                        action: FightAction::Dodge,
                        frames: 18,
                        strength: 1.,
                    },
                    FightBeat {
                        attacker: b,
                        action: FightAction::Counter,
                        frames: 24,
                        strength: 1.2,
                    },
                    FightBeat {
                        attacker: a,
                        action: FightAction::Kick,
                        frames: 24,
                        strength: 1.4,
                    },
                    FightBeat {
                        attacker: b,
                        action: FightAction::Knockback,
                        frames: 24,
                        strength: 1.,
                    },
                ];
            }
        });
        ui.small(i18n::tr(
            "Editable starting poses. Review spacing and contact for your characters.",
        ));
        if ui
            .add_enabled(
                !self.motion.beats.is_empty() && self.motion.first != self.motion.second,
                egui::Button::new("Preview choreography"),
            )
            .clicked()
            && let (Some(first), Some(second)) = (self.motion.first, self.motion.second)
        {
            let preview = self.motion.start + self.motion.beats[0].frames / 2;
            self.motion_plan(
                p,
                StudioOperation::Fight {
                    first,
                    second,
                    start: self.motion.start,
                    beats: self.motion.beats.clone(),
                    style: self.motion.style,
                    impacts: self.motion.impact,
                    overwrite: self.motion.overwrite,
                },
                preview,
            );
        }
    }
    fn motion_coach(&mut self, ui: &mut egui::Ui, p: &Project) {
        self.motion_range(ui, p);
        theme::slider(
            ui,
            "Length tolerance",
            egui::Slider::new(&mut self.motion.tolerance, 0.03..=0.5)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.)),
        );
        ui.small(i18n::tr(
            "Flags are suggestions. Intentional stretch, skids and impact timing can be kept.",
        ));
        if ui.button(i18n::tr("Review motion")).clicked() {
            self.motion_request(
                ProductionRequest::Edit {
                    project: p.id,
                    expected_revision: p.revision,
                    tool: StudioOperation::Analyze {
                        start: self.motion.start,
                        end: self.motion.end,
                        characters: self.selected.into_iter().collect(),
                        tolerance: self.motion.tolerance,
                    },
                    apply: false,
                    preview_frame: None,
                },
                false,
            );
        }
        let issues = self.motion.issues.clone();
        for (i, issue) in issues.iter().enumerate() {
            ui.push_id(i, |ui| {
                ui.separator();
                ui.add(egui::Label::new(&issue.message).wrap());
                ui.horizontal_wrapped(|ui| {
                    if ui.button(format!("Frame {}", issue.frame + 1)).clicked() {
                        self.seek_frame(issue.frame);
                        self.selected = Some(issue.character);
                    }
                    if ui
                        .add_enabled(
                            !issue.commands.is_empty() && self.motion.issue_revision == p.revision,
                            egui::Button::new("Preview correction"),
                        )
                        .clicked()
                    {
                        self.motion_plan(
                            p,
                            StudioOperation::Commands {
                                name: "Suggested correction".into(),
                                commands: issue.commands.clone(),
                            },
                            issue.frame,
                        );
                    }
                });
            });
        }
    }
    fn motion_frames(&mut self, ui: &mut egui::Ui, p: &Project) {
        self.motion_range(ui, p);
        ui.small(i18n::tr("Shift-click frame thumbnails to select a range."));
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.motion.pose_scope, false, "Drawing layer");
            ui.selectable_value(&mut self.motion.pose_scope, true, "Pose keys");
        });
        egui::ComboBox::from_id_salt("frames.operation")
            .truncate()
            .selected_text(format!("{:?}", self.motion.frame_action))
            .show_ui(ui, |ui| {
                for action in [
                    FrameEdit::Copy,
                    FrameEdit::Move,
                    FrameEdit::Stretch,
                    FrameEdit::Translate,
                    FrameEdit::Hold,
                ] {
                    ui.selectable_value(
                        &mut self.motion.frame_action,
                        action,
                        format!("{action:?}"),
                    );
                }
            });
        ui.horizontal_wrapped(|ui| {
            ui.label(i18n::tr("Destination / new end"));
            frame_field(ui, &mut self.motion.target, 999999);
        });
        if self.motion.frame_action == FrameEdit::Translate {
            point_fields(ui, &mut self.motion.delta, "Offset");
        }
        ui.checkbox(
            &mut self.motion.overwrite,
            i18n::tr("Replace destination keys"),
        );
        if ui.button(i18n::tr("Preview batch edit")).clicked() {
            let tool = if self.motion.pose_scope {
                self.selected.map(|character| StudioOperation::PoseBatch {
                    character,
                    start: self.motion.start,
                    end: self.motion.end,
                    action: self.motion.frame_action,
                    target: self.motion.target,
                    new_end: self.motion.target,
                    delta: self.motion.delta,
                    overwrite: self.motion.overwrite,
                })
            } else {
                self.selected_layer
                    .map(|layer| StudioOperation::FrameBatch {
                        layer,
                        start: self.motion.start,
                        end: self.motion.end,
                        action: self.motion.frame_action,
                        target: self.motion.target,
                        new_end: self.motion.target,
                        delta: self.motion.delta,
                        overwrite: self.motion.overwrite,
                    })
            };
            if let Some(tool) = tool {
                self.motion_plan(p, tool, self.motion.target.min(p.duration - 1));
            } else {
                self.status = "Select a character or drawing layer first".into();
            }
        }
        theme::section(ui, "TIMING");
        ui.horizontal_wrapped(|ui| {
            ui.label(i18n::tr("Selected scene duration"));
            ui.add(
                egui::DragValue::new(&mut self.motion.seconds)
                    .range(0.05..=120.)
                    .speed(0.05)
                    .suffix(" s"),
            );
            if ui.button(i18n::tr("Preview ripple retime")).clicked() {
                self.motion_plan(
                    p,
                    StudioOperation::Timing {
                        start: self.motion.start,
                        end: self.motion.end,
                        seconds: self.motion.seconds,
                    },
                    self.motion.start,
                );
            }
        });
        ui.small(i18n::tr(
            "Retime changes movement duration. FPS changes the playback/export rate.",
        ));
        ui.horizontal_wrapped(|ui| {
            for fps in [12, 24, 30, 60] {
                if ui.button(format!("{fps} fps")).clicked() {
                    self.motion_plan(
                        p,
                        StudioOperation::FrameRate {
                            fps,
                            preserve_seconds: true,
                        },
                        0,
                    );
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.text_edit_singleline(&mut self.motion.marker);
            if ui.button(i18n::tr("Mark playhead")).clicked() {
                self.motion_plan(
                    p,
                    StudioOperation::Marker {
                        name: self.motion.marker.clone(),
                        frame: Some(self.frame as u32),
                    },
                    self.frame as u32,
                );
            }
        });
        for (name, frame) in &p.markers {
            if ui.button(format!("{name} · {}", frame + 1)).clicked() {
                self.seek_frame(*frame);
            }
        }
    }
    fn motion_library(&mut self, ui: &mut egui::Ui, p: &Project) {
        theme::section(ui, "ACTION PRESETS");
        ui.horizontal_wrapped(|ui| {
            for action in ActionKind::ALL {
                if ui
                    .add_enabled(
                        self.selected.is_some(),
                        egui::Button::new(format!("{action:?}")),
                    )
                    .clicked()
                {
                    let start = self.frame as u32;
                    let end = (start + p.fps).min(p.duration - 1);
                    self.motion_plan(
                        p,
                        StudioOperation::Preset {
                            character: self.selected.unwrap(),
                            action,
                            start,
                            end,
                            distance: if matches!(action, ActionKind::Walk | ActionKind::Run) {
                                120.
                            } else {
                                0.
                            },
                            intensity: 1.,
                            direction: 1.,
                        },
                        start,
                    );
                }
            }
        });
        theme::section(ui, "RIG TEMPLATES");
        ui.horizontal_wrapped(|ui| {
            for (template, label) in [
                (RigTemplate::Human, "Human"),
                (RigTemplate::Quadruped, "Animal"),
                (RigTemplate::Winged, "Winged"),
            ] {
                ui.selectable_value(&mut self.motion.template, template, label);
            }
            ui.add(
                egui::DragValue::new(&mut self.motion.rig_height)
                    .range(20..=8000)
                    .suffix(" px"),
            );
            if ui.button(i18n::tr("Create rig")).clicked() {
                self.motion_plan(
                    p,
                    StudioOperation::Rig {
                        template: self.motion.template,
                        name: self.motion.name.clone(),
                        position: Point::new(p.width as f32 * 0.5, p.height as f32 * 0.6),
                        height: self.motion.rig_height,
                        color: [35, 38, 44, 255],
                    },
                    self.frame as u32,
                );
            }
        });
        theme::section(ui, "SAVED ITEMS");
        if let Ok(entries) = library_entries(p) {
            for entry in entries {
                ui.push_id(entry.id, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let response = ui.add(
                            egui::Button::new(&entry.name)
                                .truncate()
                                .selected(self.motion.selected_entry == Some(entry.id))
                                .sense(Sense::click_and_drag()),
                        );
                        if response.clicked() {
                            self.motion.selected_entry = Some(entry.id);
                        }
                        if response.drag_started() {
                            self.motion.dragging_entry = Some(entry.id);
                            self.motion.open = false;
                        }
                        ui.small(format!("{:?} · {} fr", entry.kind, entry.duration));
                        if theme::icon_button(ui, "Insert item", theme::Icon::Plus, false).clicked()
                        {
                            let character = if entry.kind == LibraryKind::Motion {
                                self.selected
                            } else {
                                None
                            };
                            let position = character
                                .and_then(|id| pose_at(p, id, self.frame as u32).ok())
                                .map_or(
                                    Point::new(p.width as f32 * 0.5, p.height as f32 * 0.6),
                                    |pose| pose.root,
                                );
                            self.motion_plan(
                                p,
                                StudioOperation::InsertLibrary {
                                    entry: entry.id,
                                    character,
                                    frame: self.frame as u32,
                                    duration: entry.duration,
                                    position,
                                    mirror: self.motion.mirror_insert,
                                },
                                self.frame as u32,
                            );
                        }
                        if theme::icon_button(ui, "Remove item", theme::Icon::Delete, false)
                            .clicked()
                        {
                            self.motion_plan(
                                p,
                                StudioOperation::RemoveLibrary { entry: entry.id },
                                self.frame as u32,
                            );
                        }
                    });
                });
            }
        }
        ui.checkbox(
            &mut self.motion.mirror_insert,
            i18n::tr("Mirror inserted motion"),
        );
        self.motion_range(ui, p);
        ui.text_edit_singleline(&mut self.motion.name);
        egui::ComboBox::from_id_salt("library.kind")
            .truncate()
            .selected_text(format!("{:?}", self.motion.kind))
            .show_ui(ui, |ui| {
                for kind in [
                    LibraryKind::Character,
                    LibraryKind::Motion,
                    LibraryKind::Symbol,
                    LibraryKind::Combo,
                ] {
                    ui.selectable_value(&mut self.motion.kind, kind, format!("{kind:?}"));
                }
            });
        if ui.button(i18n::tr("Save selection to library")).clicked() {
            let characters = if self.motion.kind == LibraryKind::Combo {
                [self.motion.first.or(self.selected), self.motion.second]
                    .into_iter()
                    .flatten()
                    .collect()
            } else {
                self.selected.into_iter().collect()
            };
            self.motion_request(
                ProductionRequest::Edit {
                    project: p.id,
                    expected_revision: p.revision,
                    tool: StudioOperation::SaveLibrary {
                        name: self.motion.name.clone(),
                        kind: self.motion.kind,
                        characters,
                        layer: self.selected_layer,
                        start: self.motion.start,
                        end: self.motion.end,
                    },
                    apply: false,
                    preview_frame: None,
                },
                false,
            );
        }
        ui.separator();
        ui.text_edit_singleline(&mut self.motion.library_path);
        ui.horizontal_wrapped(|ui| {
            if ui.button(i18n::tr("Export library")).clicked() {
                self.motion_request(
                    ProductionRequest::ExportLibrary {
                        project: p.id,
                        path: self.motion.library_path.clone(),
                        entries: vec![],
                    },
                    false,
                );
            }
            if ui.button(i18n::tr("Import library")).clicked() {
                self.motion_request(
                    ProductionRequest::ImportLibrary {
                        project: p.id,
                        expected_revision: p.revision,
                        path: self.motion.library_path.clone(),
                    },
                    false,
                );
            }
        });
    }
    fn motion_finish(&mut self, ui: &mut egui::Ui, p: &Project) {
        theme::section(ui, "IMPACT ACCENTS");
        Self::motion_actor(ui, p, &mut self.selected, "impact.actor", "Attach to");
        if let Some(c) = self.selected.and_then(|id| p.character(id)) {
            egui::ComboBox::from_id_salt("impact.joint")
                .truncate()
                .selected_text(&self.motion.joint)
                .show_ui(ui, |ui| {
                    for j in &c.joints {
                        ui.selectable_value(
                            &mut self.motion.joint,
                            j.name.clone(),
                            j.name.replace('_', " "),
                        );
                    }
                });
        }
        ui.horizontal_wrapped(|ui| {
            for pack in [ImpactPack::Light, ImpactPack::Heavy, ImpactPack::Energy] {
                ui.selectable_value(&mut self.motion.pack, pack, format!("{pack:?}"));
            }
        });
        egui::ComboBox::from_id_salt("impact.sound")
            .truncate()
            .selected_text(
                self.motion
                    .sound
                    .and_then(|id| p.assets.iter().find(|a| a.id == id))
                    .map_or("No sound cue", |a| a.name.as_str()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.motion.sound, None, "No sound cue");
                for a in p.assets.iter().filter(|a| a.kind == "audio") {
                    ui.selectable_value(&mut self.motion.sound, Some(a.id), &a.name);
                }
            });
        ui.add(
            egui::DragValue::new(&mut self.motion.sound_seconds)
                .range(0.01..=10.)
                .speed(0.05)
                .suffix(" seconds"),
        );
        if let Some(character) = self.selected
            && ui.button(i18n::tr("Preview impact at contact")).clicked()
        {
            self.motion_plan(
                p,
                StudioOperation::Impact {
                    character,
                    joint: self.motion.joint.clone(),
                    frame: self.frame as u32,
                    action: self.selected_action,
                    pack: self.motion.pack,
                    sound: self.motion.sound,
                    sound_seconds: self.motion.sound_seconds,
                },
                self.frame as u32,
            );
        }
        ui.separator();
        egui::ComboBox::from_id_salt("effect.edit")
            .truncate()
            .selected_text(
                self.motion
                    .selected_effect
                    .and_then(|id| p.effects.iter().find(|e| e.id == id))
                    .map_or("Select effect", |e| e.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for effect in &p.effects {
                    if ui
                        .selectable_label(
                            self.motion.selected_effect == Some(effect.id),
                            &effect.name,
                        )
                        .clicked()
                    {
                        self.motion.selected_effect = Some(effect.id);
                        self.motion.effect_edit = Some(effect.clone());
                    }
                }
            });
        if let Some(mut effect) = self.motion.effect_edit.take() {
            ui.horizontal_wrapped(|ui| {
                ui.label(i18n::tr("Start"));
                frame_field(ui, &mut effect.start, p.duration - 1);
                ui.label(i18n::tr("End"));
                frame_field(ui, &mut effect.end, p.duration - 1);
            });
            theme::slider(
                ui,
                "Intensity",
                egui::Slider::new(&mut effect.intensity, 0.01..=5.),
            );
            ui.color_edit_button_srgba_unmultiplied(&mut effect.color);
            if ui.button(i18n::tr("Preview effect changes")).clicked() {
                self.motion_plan(
                    p,
                    StudioOperation::EditEffect {
                        effect: effect.clone(),
                    },
                    self.frame as u32,
                );
            }
            self.motion.effect_edit = Some(effect);
        }
        theme::section(ui, "SOUND");
        ui.horizontal_wrapped(|ui| {
            if ui.button(i18n::tr("Import audio")).clicked() {
                self.import_audio();
            }
            if ui.button(i18n::tr("Voice and dialogue")).clicked() {
                self.assist.voice_open = true;
            }
        });
        egui::ComboBox::from_id_salt("audio.edit")
            .truncate()
            .selected_text(
                self.motion
                    .selected_audio
                    .and_then(|id| p.audio.iter().find(|a| a.id == id))
                    .map_or("Select audio clip", |a| a.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for audio in &p.audio {
                    if ui
                        .selectable_label(self.motion.selected_audio == Some(audio.id), &audio.name)
                        .clicked()
                    {
                        self.motion.selected_audio = Some(audio.id);
                        self.motion.audio_edit = Some(audio.clone());
                    }
                }
            });
        if let Some(mut clip) = self.motion.audio_edit.take() {
            self.request_wave(p, clip.asset);
            if let Some(wave) = self.motion.waves.get(&clip.asset) {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 70.), Sense::hover());
                paint_wave(ui.painter(), rect, wave, &clip);
                ui.small(format!("Source {:.2}s", wave.duration));
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(i18n::tr("Start"));
                frame_field(ui, &mut clip.start, p.duration - 1);
                ui.label(i18n::tr("Trim"));
                ui.add(
                    egui::DragValue::new(&mut clip.offset_seconds)
                        .range(0.0..=600.)
                        .speed(0.02)
                        .suffix(" s"),
                );
                ui.label(i18n::tr("Length"));
                ui.add(
                    egui::DragValue::new(&mut clip.duration_seconds)
                        .range(0.01..=600.)
                        .speed(0.02)
                        .suffix(" s"),
                );
            });
            theme::slider(
                ui,
                "Pitch / speed",
                egui::Slider::new(&mut clip.speed, 0.25..=4.0),
            );
            theme::slider(ui, "Gain", egui::Slider::new(&mut clip.gain, 0.0..=4.));
            ui.horizontal_wrapped(|ui| {
                ui.label(i18n::tr("Fade in"));
                ui.add(
                    egui::DragValue::new(&mut clip.fade_in)
                        .range(0.0..=10.)
                        .speed(0.01),
                );
                ui.label(i18n::tr("Fade out"));
                ui.add(
                    egui::DragValue::new(&mut clip.fade_out)
                        .range(0.0..=10.)
                        .speed(0.01),
                );
                ui.checkbox(&mut clip.looping, i18n::tr("Loop"));
            });
            if ui.button(i18n::tr("Preview audio edit")).clicked() {
                self.motion_plan(
                    p,
                    StudioOperation::EditAudio { clip: clip.clone() },
                    self.frame as u32,
                );
            }
            self.motion.audio_edit = Some(clip);
        }
    }
    fn motion_camera(&mut self, ui: &mut egui::Ui, p: &Project) {
        self.motion_range(ui, p);
        ui.checkbox(
            &mut self.motion.follow,
            i18n::tr("Track selected character"),
        );
        if self.motion.follow {
            Self::motion_actor(ui, p, &mut self.selected, "camera.actor", "Track");
        }
        point_fields(
            ui,
            &mut self.motion.from,
            if self.motion.follow {
                "Starting offset"
            } else {
                "From"
            },
        );
        point_fields(
            ui,
            &mut self.motion.to,
            if self.motion.follow {
                "Ending offset"
            } else {
                "To"
            },
        );
        theme::slider(
            ui,
            "Starting zoom",
            egui::Slider::new(&mut self.motion.zoom_start, 0.1..=5.),
        );
        theme::slider(
            ui,
            "Ending zoom",
            egui::Slider::new(&mut self.motion.zoom_end, 0.1..=5.),
        );
        if ui.button(i18n::tr("Preview camera move")).clicked() {
            self.motion_plan(
                p,
                StudioOperation::Camera {
                    start: self.motion.start,
                    end: self.motion.end,
                    character: if self.motion.follow {
                        self.selected
                    } else {
                        None
                    },
                    from: self.motion.from,
                    to: self.motion.to,
                    zoom_start: self.motion.zoom_start,
                    zoom_end: self.motion.zoom_end,
                },
                self.motion.start,
            );
        }
    }
    pub(super) fn quick_motion_inspector(&mut self, ui: &mut egui::Ui, p: &Project) {
        match self.preferences.mode {
            WorkspaceMode::Audio => {
                ui.heading(i18n::tr("Sound"));
                if theme::button(ui, "Import audio", theme::Icon::Plus, false).clicked() {
                    self.import_audio();
                }
                if ui.button(i18n::tr("Clips & waveforms")).clicked() {
                    self.motion.tab = Tab::Finish;
                    self.motion.open = true;
                }
                if ui.button(i18n::tr("Voice & dialogue")).clicked() {
                    self.assist.voice_open = true;
                }
                ui.small(format!("{} editable audio clips", p.audio.len()));
                ui.small(i18n::tr(
                    "Click a waveform in the timeline to trim, fade or mix a clip.",
                ));
            }
            WorkspaceMode::Effects => {
                ui.heading(i18n::tr("Effects"));
                for (tab, label) in [
                    (Tab::Finish, "Impact packs & effects"),
                    (Tab::Camera, "Camera director"),
                ] {
                    if ui.button(label).clicked() {
                        self.motion.tab = tab;
                        self.motion.open = true;
                    }
                }
                ui.small(format!("{} editable effect tracks", p.effects.len()));
                ui.small(i18n::tr(
                    "Click an effect on the detailed timeline to refine it.",
                ));
            }
            _ => {
                ui.heading(i18n::tr("Motion"));
                ui.checkbox(&mut self.preferences.smart_pose, i18n::tr("Smart posing"));
                style_picker(ui, &mut self.preferences.movement_style);
                ui.checkbox(
                    &mut self.preferences.smart_onion,
                    i18n::tr("Changed-joint ghosts"),
                );
                ui.checkbox(&mut self.preferences.ghost_path, i18n::tr("Ghost path"));
                for (tab, label) in [
                    (Tab::Pose, "Pose & foot locks"),
                    (Tab::Motion, "Motion brush & curves"),
                    (Tab::Fight, "Fight choreography"),
                    (Tab::Coach, "Review motion"),
                    (Tab::Frames, "Frames & timing"),
                    (Tab::Library, "Reusable library"),
                    (Tab::Finish, "Effects & sound"),
                    (Tab::Camera, "Camera director"),
                ] {
                    if ui.button(label).clicked() {
                        self.motion.tab = tab;
                        self.motion.open = true;
                    }
                }
                if self.selected.is_none() {
                    ui.small(i18n::tr("Select a character on the canvas to pose it."));
                }
                if p.characters.is_empty() && ui.button(i18n::tr("Create a rig")).clicked() {
                    self.motion.tab = Tab::Library;
                    self.motion.open = true;
                }
            }
        }
        ui.separator();
        ui.checkbox(&mut self.preferences.pro_mode, i18n::tr("Pro controls"));
    }
    pub(super) fn finishing_tracks(
        &mut self,
        ui: &mut egui::Ui,
        p: &Project,
        width: f32,
        name_width: f32,
        pixels: f32,
        row_height: f32,
    ) {
        for clip in &p.audio {
            self.request_wave(p, clip.asset);
            ui.push_id(clip.id, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [name_width, 24.],
                        egui::Label::new(RichText::new(&clip.name).color(MUTED)).truncate(),
                    );
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new((width - name_width - 12.).max(1.), row_height),
                        Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 3., theme::BACKGROUND);
                    let end = (clip.start as f64 + clip.duration_seconds * p.fps as f64)
                        .min(p.duration as f64) as f32;
                    let r = Rect::from_min_max(
                        Pos2::new(rect.left() + clip.start as f32 * pixels, rect.top() + 2.),
                        Pos2::new(rect.left() + end * pixels, rect.bottom() - 2.),
                    );
                    if let Some(wave) = self.motion.waves.get(&clip.asset) {
                        paint_wave(ui.painter(), r, wave, clip);
                    } else {
                        ui.painter()
                            .rect_filled(r, 4., Color32::from_rgb(50, 105, 115));
                    }
                    ui.painter().rect_stroke(
                        r,
                        4.,
                        Stroke::new(
                            1.,
                            if self.motion.selected_audio == Some(clip.id) {
                                theme::ACCENT
                            } else {
                                theme::BORDER
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    if ui
                        .interact(r, ui.scope_id().with("audio"), Sense::click())
                        .on_hover_text("Edit timing, trim, gain and fades")
                        .clicked()
                    {
                        self.motion.selected_audio = Some(clip.id);
                        self.motion.audio_edit = Some(clip.clone());
                        self.motion.tab = Tab::Finish;
                        self.motion.open = true;
                    }
                });
            });
        }
        for effect in &p.effects {
            ui.push_id(effect.id, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [name_width, 24.],
                        egui::Label::new(RichText::new(&effect.name).color(MUTED)).truncate(),
                    );
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new((width - name_width - 12.).max(1.), row_height),
                        Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 3., theme::BACKGROUND);
                    let r = Rect::from_min_max(
                        Pos2::new(rect.left() + effect.start as f32 * pixels, rect.top() + 5.),
                        Pos2::new(
                            rect.left() + (effect.end + 1) as f32 * pixels,
                            rect.bottom() - 5.,
                        ),
                    );
                    ui.painter().rect_filled(
                        r,
                        4.,
                        if self.motion.selected_effect == Some(effect.id) {
                            theme::ACCENT
                        } else {
                            Color32::from_rgb(170, 135, 72)
                        },
                    );
                    if ui
                        .interact(r, ui.scope_id().with("effect"), Sense::click())
                        .on_hover_text("Edit this effect")
                        .clicked()
                    {
                        self.motion.selected_effect = Some(effect.id);
                        self.motion.effect_edit = Some(effect.clone());
                        self.motion.tab = Tab::Finish;
                        self.motion.open = true;
                    }
                });
            });
        }
    }
    pub(super) fn request_wave(&mut self, p: &Project, asset: Id) {
        if self.motion.waves.contains_key(&asset) || self.motion.wave_pending.is_some() {
            return;
        }
        let service = self.service.clone();
        let project = p.id;
        let (send, receive) = std::sync::mpsc::channel();
        self.motion.wave_pending = Some(receive);
        std::thread::spawn(move || {
            let result = service
                .production(ProductionRequest::Waveform {
                    project,
                    asset,
                    columns: 512,
                })
                .and_then(|value| {
                    Ok(Wave {
                        peaks: serde_json::from_value(value["peaks"].clone())?,
                        duration: value["duration"].as_f64().unwrap_or(0.),
                    })
                });
            let _ = send.send((asset, result));
        });
    }
    pub(super) fn motion_copy_range(&mut self, p: &Project) -> bool {
        let Some((start, end)) = self.motion.selection else {
            return false;
        };
        if end <= start {
            return false;
        }
        let kind = if self.preferences.mode == WorkspaceMode::Draw {
            LibraryKind::Symbol
        } else {
            LibraryKind::Motion
        };
        self.motion_request(
            ProductionRequest::Edit {
                project: p.id,
                expected_revision: p.revision,
                tool: StudioOperation::SaveLibrary {
                    name: "Clipboard".into(),
                    kind,
                    characters: if kind == LibraryKind::Motion {
                        self.selected.into_iter().collect()
                    } else {
                        vec![]
                    },
                    layer: self.selected_layer,
                    start,
                    end,
                },
                apply: false,
                preview_frame: None,
            },
            true,
        );
        true
    }
    pub(super) fn motion_paste_range(&mut self, p: &Project) -> bool {
        let Some(entry) = self.motion.clipboard.clone() else {
            return false;
        };
        let character = if entry.kind == LibraryKind::Motion {
            self.selected
        } else {
            None
        };
        let position = character
            .and_then(|id| pose_at(p, id, self.frame as u32).ok())
            .map_or(entry.origin, |pose| pose.root);
        self.motion_plan(
            p,
            StudioOperation::Paste {
                entry,
                character,
                layer: self.selected_layer,
                frame: self.frame as u32,
                position,
                overwrite: true,
            },
            self.frame as u32,
        );
        true
    }
    pub(super) fn motion_canvas_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        to_world: impl Fn(Pos2) -> Point,
    ) {
        let primary = egui::PointerButton::Primary;
        let (press, release, position, down) = ui.input(|i| {
            let event_pos = |pressed| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: p,
                        ..
                    } if *p == pressed => Some(*pos),
                    _ => None,
                })
            };
            (
                event_pos(true),
                event_pos(false),
                i.pointer.interact_pos(),
                i.pointer.primary_down(),
            )
        });
        let press = press.filter(|pos| {
            rect.contains(*pos)
                && ui
                    .ctx()
                    .layer_id_at(*pos)
                    .is_none_or(|layer| layer == ui.layer_id())
        });
        if let Some(origin) = press.or_else(|| {
            if response.drag_started_by(primary) {
                ui.input(|i| i.pointer.press_origin())
                    .filter(|p| rect.contains(*p))
            } else {
                None
            }
        }) {
            self.motion.path.clear();
            self.motion.path.push(to_world(origin));
        }
        let finished = response.drag_stopped_by(primary)
            || (release.is_some() && !self.motion.path.is_empty());
        if !self.motion.path.is_empty()
            && (down || response.dragged_by(primary) || finished)
            && let Some(pos) = release.or(position).filter(|p| rect.contains(*p))
        {
            let point = to_world(pos);
            if self.motion.path.len() < 512
                && self
                    .motion
                    .path
                    .last()
                    .is_none_or(|p| p.distance(point) > 1.)
            {
                self.motion.path.push(point);
            } else if finished && self.motion.path.len() == 512 {
                *self.motion.path.last_mut().unwrap() = point;
            }
        }
        if finished && self.motion.path.len() >= 2 {
            self.motion.draw_path = false;
            self.motion.open = true;
            self.motion.tab = Tab::Motion;
            self.status = "Path captured · preview the key poses".into();
        }
    }
    pub(super) fn handle_library_drop(
        &mut self,
        p: &Project,
        ui: &egui::Ui,
        rect: Rect,
        to_world: impl Fn(Pos2) -> Point,
    ) {
        if let Some(id) = self.motion.dragging_entry
            && ui.input(|i| i.pointer.any_released())
        {
            self.motion.dragging_entry = None;
            if let Some(pos) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|pos| rect.contains(*pos))
                && let Ok(entries) = library_entries(p)
                && let Some(entry) = entries.iter().find(|e| e.id == id)
            {
                self.motion_plan(
                    p,
                    StudioOperation::InsertLibrary {
                        entry: id,
                        character: if entry.kind == LibraryKind::Motion {
                            self.selected
                        } else {
                            None
                        },
                        frame: self.frame as u32,
                        duration: entry.duration,
                        position: to_world(pos),
                        mirror: self.motion.mirror_insert,
                    },
                    self.frame as u32,
                );
            }
        }
    }
    pub(super) fn paint_motion_overlay(
        &mut self,
        p: &Project,
        painter: &egui::Painter,
        to_screen: impl Fn(Point) -> Pos2 + Copy,
        scale: f32,
    ) {
        if self.motion.path.len() > 1 {
            painter.add(egui::Shape::line(
                self.motion.path.iter().copied().map(to_screen).collect(),
                Stroke::new(2., theme::ACCENT),
            ));
        }
        if let Some(id) = self.selected {
            if self.preferences.ghost_path {
                let start = self.motion.start.min(p.duration - 1);
                let end = self.motion.end.clamp(start, p.duration - 1);
                let key = (p.id, p.revision, id, self.motion.joint.clone(), start, end);
                if self.motion.ghost_key.as_ref() != Some(&key) {
                    self.motion.ghost =
                        trajectory(p, id, &self.motion.joint, start, end, 2).unwrap_or_default();
                    self.motion.ghost_key = Some(key);
                }
                for pair in self.motion.ghost.windows(2) {
                    let color = if pair[1].speed > 250. {
                        Color32::from_rgb(226, 165, 100)
                    } else {
                        theme::ACCENT
                    };
                    painter.line_segment(
                        [to_screen(pair[0].position), to_screen(pair[1].position)],
                        Stroke::new(1.5, color),
                    );
                }
            }
            if self.preferences.smart_onion
                && let (Some(c), Ok(now), Ok(previous)) = (
                    p.character(id),
                    pose_at(p, id, self.frame as u32),
                    pose_at(p, id, (self.frame as u32).saturating_sub(2)),
                )
            {
                for (i, joint) in c.joints.iter().enumerate() {
                    let a = previous.root + previous.joints[i];
                    let b = now.root + now.joints[i];
                    let changed = a.distance(b) > 0.5;
                    let color = if changed {
                        Color32::from_rgba_unmultiplied(104, 157, 245, 140)
                    } else {
                        Color32::from_rgba_unmultiplied(150, 154, 166, 55)
                    };
                    if let Some(parent) = joint.parent {
                        painter.line_segment(
                            [
                                to_screen(a),
                                to_screen(previous.root + previous.joints[parent]),
                            ],
                            Stroke::new((c.thickness * scale).max(1.), color),
                        );
                    }
                    if changed {
                        painter.circle_stroke(to_screen(b), 5., Stroke::new(1.5, theme::ACCENT));
                    }
                }
            }
        }
    }
}
fn frame_field(ui: &mut egui::Ui, value: &mut u32, max: u32) {
    ui.add(
        egui::DragValue::new(value)
            .range(0..=max)
            .custom_formatter(|v, _| format!("{}", v as u32 + 1))
            .custom_parser(|s| s.parse::<f64>().ok().map(|v| (v - 1.).max(0.))),
    );
}

fn point_fields(ui: &mut egui::Ui, value: &mut Point, label: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(&mut value.x).speed(1.).prefix("X "));
        ui.add(egui::DragValue::new(&mut value.y).speed(1.).prefix("Y "));
    });
}
fn style_picker(ui: &mut egui::Ui, style: &mut MovementStyle) {
    ui.horizontal_wrapped(|ui| {
        for (value, label) in [
            (MovementStyle::Natural, "Natural"),
            (MovementStyle::Exaggerated, "Exaggerated"),
            (MovementStyle::AnimeFight, "Anime fight"),
        ] {
            ui.selectable_value(style, value, label);
        }
    });
}
fn curve_editor(ui: &mut egui::Ui, easing: &mut Easing) {
    ui.horizontal_wrapped(|ui| {
        for (label, value) in [
            ("Even", Easing::Linear),
            ("Ease", Easing::Smooth),
            ("Flow", Easing::Flow),
            (
                "Snappy",
                Easing::Bezier {
                    x1: 0.5,
                    y1: 0.,
                    x2: 0.65,
                    y2: 1.,
                },
            ),
            ("Hold", Easing::Hold),
        ] {
            if ui.button(label).clicked() {
                *easing = value;
            }
        }
    });
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(440.), 120.),
        Sense::hover(),
    );
    let inner = rect.shrink(10.);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8., theme::BACKGROUND);
    let at = |x: f32, y: f32| {
        Pos2::new(
            inner.left() + x * inner.width(),
            inner.bottom() - y * inner.height(),
        )
    };
    painter.line_segment([at(0., 0.), at(1., 1.)], Stroke::new(1., theme::BORDER));
    painter.add(egui::Shape::line(
        (0..=64)
            .map(|i| {
                let t = i as f32 / 64.;
                at(t, ease(easing, t))
            })
            .collect(),
        Stroke::new(2., theme::ACCENT),
    ));
    if let Easing::Bezier { x1, y1, x2, y2 } = easing {
        ui.horizontal_wrapped(|ui| {
            for (label, value, min, max) in [
                ("X1", x1, 0., 1.),
                ("Y1", y1, -2., 3.),
                ("X2", x2, 0., 1.),
                ("Y2", y2, -2., 3.),
            ] {
                ui.add(
                    egui::DragValue::new(value)
                        .range(min..=max)
                        .speed(0.01)
                        .prefix(format!("{label} ")),
                );
            }
        });
    }
}
pub(super) fn paint_wave(painter: &egui::Painter, rect: Rect, wave: &Wave, clip: &AudioClip) {
    painter.rect_filled(rect, 5., theme::BACKGROUND);
    if wave.peaks.is_empty() || wave.duration <= 0. {
        return;
    }
    let n = rect.width().ceil().max(1.) as usize;
    for i in 0..n {
        let second =
            clip.offset_seconds + i as f64 / n as f64 * clip.duration_seconds * clip.speed as f64;
        let second = if clip.looping {
            second.rem_euclid(wave.duration)
        } else {
            second
        };
        let peak = if second < wave.duration {
            wave.peaks[((second / wave.duration * wave.peaks.len() as f64) as usize)
                .min(wave.peaks.len() - 1)]
        } else {
            0.
        };
        let t = i as f32 / n as f32;
        let fade_in = if clip.fade_in > 0. {
            (t * clip.duration_seconds as f32 / clip.fade_in).min(1.)
        } else {
            1.
        };
        let fade_out = if clip.fade_out > 0. {
            ((1. - t) * clip.duration_seconds as f32 / clip.fade_out).min(1.)
        } else {
            1.
        };
        let height = (peak * clip.gain * fade_in * fade_out).clamp(0., 1.) * rect.height() * 0.43;
        let x = rect.left() + i as f32;
        painter.line_segment(
            [
                Pos2::new(x, rect.center().y - height),
                Pos2::new(x, rect.center().y + height),
            ],
            Stroke::new(1., theme::ACCENT),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_fast_motion_path_keeps_press_and_release_points_without_editing_the_scene() {
        let (_dir, mut studio, _) = super::super::tests::studio();
        studio.motion.draw_path = true;
        let before =
            serde_json::to_value(studio.service.snapshot(studio.project).unwrap()).unwrap();
        let ctx = studio.context.clone();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(600., 400.));
        let start = Pos2::new(80., 90.);
        let end = Pos2::new(220., 160.);
        let mut output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(rect),
                events: vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                    egui::Event::PointerMoved(end),
                    egui::Event::PointerButton {
                        pos: end,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                let mut ui = egui::Ui::new(
                    ctx.clone(),
                    egui::Id::unique("motion-fast-path"),
                    egui::UiBuilder::new().max_rect(rect),
                );
                let (area, response) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
                studio.motion_canvas_input(&ui, &response, area, |p| Point::new(p.x, p.y));
            },
        );
        output.textures_delta.clear();
        assert_eq!(
            studio.motion.path.first(),
            Some(&Point::new(start.x, start.y))
        );
        assert_eq!(studio.motion.path.last(), Some(&Point::new(end.x, end.y)));
        assert!(studio.motion.open);
        assert!(!studio.motion.draw_path);
        assert_eq!(
            serde_json::to_value(studio.service.snapshot(studio.project).unwrap()).unwrap(),
            before
        );
    }
    #[test]
    fn comparison_transport_and_accept_remain_visible_in_short_windows() {
        let (_dir, mut studio, _) = super::super::tests::studio();
        let p = studio.service.snapshot(studio.project).unwrap();
        studio.motion.proposal = Some(Proposed {
            project: p.id,
            revision: p.revision,
            before: p.clone(),
            after: p,
            plan: StudioPlan {
                name: "Review editable motion".into(),
                ..Default::default()
            },
            frame: 0.,
            playing: false,
            tick: Instant::now(),
        });
        let ctx = studio.context.clone();
        theme::install(&ctx);
        ctx.enable_accesskit();
        let mut nodes = vec![];
        for _ in 0..4 {
            let p = studio.service.snapshot(studio.project).unwrap();
            let mut output = ctx.run_pass(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 520.))),
                    ..Default::default()
                },
                |ctx| studio.motion_windows(ctx, &p),
            );
            output.textures_delta.clear();
            nodes = output.platform_output.accesskit_update.unwrap().nodes;
        }
        for label in ["Play preview", "Accept", "Cancel"] {
            let n = nodes
                .iter()
                .find(|(_, n)| n.role() == egui::Role::Button && n.label() == Some(label))
                .unwrap_or_else(|| panic!("Missing {label}"));
            let r = n.1.bounds().unwrap();
            assert!(
                r.x0 >= 0. && r.y0 >= 0. && r.x1 <= 800. && r.y1 <= 520.,
                "{label} escapes window: {r:?}"
            );
        }
    }
}
