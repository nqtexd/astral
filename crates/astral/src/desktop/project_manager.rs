use super::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentProject {
    id: Id,
    file: PathBuf,
    name: String,
    width: u32,
    height: u32,
    fps: u32,
    updated: u64,
    #[serde(default)]
    last_opened: u64,
    #[serde(default)]
    trashed: Option<PathBuf>,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    favorite: bool,
}
pub struct ProjectManager {
    pub home: bool,
    pub create: bool,
    pub placeholder: Option<Id>,
    pub close: Option<Id>,
    pub settings: PathBuf,
    pub(super) properties: bool,
    property_frames: u32,
    recent: Vec<RecentProject>,
    search: String,
    favorites: bool,
    trash_view: bool,
    sort_name: bool,
    manage: Option<(RecentProject, u8)>,
    manage_name: String,
    activity: Vec<crate::projects::ActivityEntry>,
    pub(super) name: String,
    width: u32,
    height: u32,
    fps: u32,
    seconds: u32,
    background: Color32,
    folder: PathBuf,
    thumbnails: BTreeMap<Id, egui::TextureHandle>,
    scan: Option<std::sync::mpsc::Receiver<Vec<RecentProject>>>,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn display_path(path: &std::path::Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}

impl ProjectManager {
    pub(super) fn set_defaults(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
    }
    pub fn new(root: &std::path::Path, placeholder: Id) -> Self {
        let settings = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join(".settings"))
            .join("Astral");
        let recent = std::fs::read(settings.join("projects.json"))
            .ok()
            .filter(|b| b.len() <= 2 * 1024 * 1024)
            .and_then(|b| serde_json::from_slice::<Vec<RecentProject>>(&b).ok())
            .unwrap_or_default();
        let (send, scan) = std::sync::mpsc::channel();
        let base = root.to_path_buf();
        std::thread::spawn(move || {
            let mut found = vec![];
            let candidates = std::fs::read_dir(&base)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|f| {
                    f.path()
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("astral"))
                })
                .take(64);
            for file in candidates {
                if file.metadata().is_ok_and(|m| m.len() < 32 * 1024 * 1024)
                    && let Ok(bytes) = std::fs::read(file.path())
                    && let Ok(p) = astral_storage::decode(&bytes)
                {
                    found.push(RecentProject {
                        id: p.id,
                        file: file.path(),
                        name: p.name,
                        width: p.width,
                        height: p.height,
                        fps: p.fps,
                        revision: p.revision,
                        updated: file
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map_or(0, |t| t.as_secs()),
                        favorite: false,
                        last_opened: 0,
                        trashed: None,
                    });
                }
            }
            let _ = send.send(found);
        });
        Self {
            home: true,
            create: false,
            placeholder: Some(placeholder),
            close: None,
            settings,
            properties: false,
            property_frames: 120,
            recent,
            search: String::new(),
            favorites: false,
            trash_view: false,
            sort_name: false,
            manage: None,
            manage_name: String::new(),
            activity: vec![],
            name: "Untitled".into(),
            width: 1920,
            height: 1080,
            fps: 24,
            seconds: 5,
            background: Color32::from_rgb(250, 250, 250),
            folder: root.join("Projects"),
            thumbnails: BTreeMap::new(),
            scan: Some(scan),
        }
    }
    fn persist(&self) -> Result<()> {
        astral_storage::Workspace::new(&self.settings)?
            .atomic_write("projects.json", &serde_json::to_vec_pretty(&self.recent)?)
    }
    pub fn record(&mut self, p: &Project, file: PathBuf) {
        let favorite = self
            .recent
            .iter()
            .find(|r| r.id == p.id)
            .is_some_and(|r| r.favorite);
        self.recent.retain(|r| r.id != p.id);
        self.recent.insert(
            0,
            RecentProject {
                id: p.id,
                updated: std::fs::metadata(&file)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs()),
                file,
                name: p.name.clone(),
                width: p.width,
                height: p.height,
                fps: p.fps,
                revision: p.revision,
                last_opened: now(),
                trashed: None,
                favorite,
            },
        );
        self.recent.truncate(100);
        let _ = self.persist();
        self.thumbnails.remove(&p.id);
        let root = self.settings.clone();
        let p = p.clone();
        std::thread::spawn(move || {
            if let Ok(pix) = astral_render::rasterize_framed(&display_list(&p, 0.), 320, 180, false)
                && let Ok(bytes) = pix.encode_png()
                && let Ok(w) = astral_storage::Workspace::new(root)
            {
                let _ = w.atomic_write(format!("thumbnails/{}-{}.png", p.id, p.revision), &bytes);
            }
        });
    }
}
impl Studio {
    pub(super) fn open_project_settings(&mut self, p: &Project) {
        self.manager.properties = true;
        self.manager.name = p.name.clone();
        self.manager.property_frames = p.duration;
        self.manager.background = rgba(p.background);
    }
    pub(super) fn show_projects(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(scan) = &self.manager.scan
            && let Ok(found) = scan.try_recv()
        {
            for entry in found {
                if !self.manager.recent.iter().any(|r| r.id == entry.id) {
                    self.manager.recent.push(entry);
                }
            }
            self.manager
                .recent
                .sort_by_key(|r| std::cmp::Reverse(r.updated));
            self.manager.scan = None;
            let _ = self.manager.persist();
        }
        egui::Panel::top("projects.header")
            .exact_size(68.)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(15.);
                    ui.label(RichText::new("Astral").size(22.).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::button(ui, "New project", theme::Icon::Plus, true).clicked() {
                            self.new_project();
                        }
                        if theme::icon_button(ui, "Settings", theme::Icon::Settings, false)
                            .clicked()
                        {
                            self.assist.settings_open = true;
                        }
                        if theme::icon_button(
                            ui,
                            "Import stickman image",
                            theme::Icon::Image,
                            false,
                        )
                        .clicked()
                        {
                            self.choose_stickman();
                        }
                        if theme::icon_button(
                            ui,
                            "Open project · Ctrl+O",
                            theme::Icon::Folder,
                            false,
                        )
                        .clicked()
                        {
                            self.open_project();
                        }
                    });
                });
            });
        egui::Panel::bottom("projects.status")
            .exact_size(28.)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(&self.status).small().color(MUTED)).truncate(),
                );
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(28.)).show(ui,|ui|{
            ui.horizontal(|ui|{
                ui.heading(i18n::tr("Projects"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if theme::icon_button(ui,"Refresh projects",theme::Icon::Undo,false).clicked(){self.manager.refresh_recent();}
                    ui.add(egui::TextEdit::singleline(&mut self.manager.search).hint_text("Search projects").desired_width((ui.available_width()-20.).clamp(80.,210.)));
                });
            });
            ui.horizontal_wrapped(|ui|{
                if ui.selectable_label(!self.manager.favorites&&!self.manager.trash_view,"All projects").clicked(){self.manager.favorites=false;self.manager.trash_view=false;}
                if ui.selectable_label(self.manager.favorites&&!self.manager.trash_view,"Favorites").clicked(){self.manager.favorites=true;self.manager.trash_view=false;}
                if ui.selectable_label(self.manager.trash_view,"Trash").clicked(){self.manager.trash_view=true;self.manager.favorites=false;self.manager.refresh_recent();}
                egui::ComboBox::from_id_salt("project.sort").selected_text(if self.manager.sort_name{"Name"}else{"Last edited"}).width(100.).show_ui(ui,|ui|{ui.selectable_value(&mut self.manager.sort_name,false,"Last edited");ui.selectable_value(&mut self.manager.sort_name,true,"Name");});
            });
            ui.add_space(12.);
            let open:Vec<_>=self.service.store.lock().unwrap().sessions.values().filter(|s|Some(s.project.id)!=self.manager.placeholder || s.project.revision>0).map(|s|(s.project.id,s.project.name.clone())).collect();
            if !open.is_empty(){ui.horizontal_wrapped(|ui|{ui.label(RichText::new("Open").color(MUTED));for (id,name) in open{if ui.button(name).clicked(){self.switch_project(id);self.manager.home=false;}}});ui.add_space(12.);}
            let mut entries:Vec<_>=self.manager.recent.iter().filter(|r|r.trashed.is_some()==self.manager.trash_view&&(!self.manager.favorites||r.favorite)&&(r.name.to_lowercase().contains(&self.manager.search.to_lowercase())||r.file.to_string_lossy().to_lowercase().contains(&self.manager.search.to_lowercase()))).cloned().collect();
            if self.manager.sort_name {entries.sort_by_key(|r|r.name.to_lowercase());} else {entries.sort_by_key(|r|std::cmp::Reverse(r.updated));}
            if entries.is_empty(){ui.add_space(60.);ui.vertical_centered(|ui|{ui.heading(i18n::tr("No recent projects"));ui.label(i18n::tr("Create a project or open an .astral file."));});}
            egui::ScrollArea::vertical().id_salt("project.library").auto_shrink([false,true]).show(ui,|ui|{
                let columns=((ui.available_width()+16.)/296.).floor().clamp(1.,4.) as usize;
                let card_width=(ui.available_width()-(columns-1) as f32*16.)/columns as f32;

                for row in entries.chunks(columns){ui.horizontal_top(|ui|{ui.spacing_mut().item_spacing.x=16.;for entry in row{
                    ui.push_id(entry.id,|ui|{
                        ui.allocate_ui_with_layout(Vec2::new(card_width,0.),egui::Layout::top_down(egui::Align::Min),|ui|{
                        egui::Frame::new().fill(theme::SURFACE).stroke(Stroke::new(1.,theme::BORDER)).corner_radius(14).inner_margin(14.).show(ui,|ui|{
                            ui.set_width(card_width-30.);
                            if !self.manager.thumbnails.contains_key(&entry.id) && let Ok(bytes)=std::fs::read(self.manager.settings.join(format!("thumbnails/{}-{}.png",entry.id,entry.revision))) && let Ok(icon)=eframe::icon_data::from_png_bytes(&bytes){self.manager.thumbnails.insert(entry.id,ctx.load_texture(format!("project-{}",entry.id),egui::ColorImage::from_rgba_unmultiplied([icon.width as usize,icon.height as usize],&icon.rgba),egui::TextureOptions::LINEAR));}
                            let (rect,response)=ui.allocate_exact_size(Vec2::new(card_width-30.,(card_width-30.)*0.5625),Sense::click());
                            response.widget_info(||egui::WidgetInfo::labeled(egui::Role::Image,ui.is_enabled(),format!("Preview {}",entry.name)));
                            ui.painter().rect_filled(rect,4.,Color32::from_rgb(18,20,24));
                            if let Some(texture)=self.manager.thumbnails.get(&entry.id){ui.painter().image(texture.id(),rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);}else{let paper=Rect::from_center_size(rect.center(),Vec2::new(85.,55.));ui.painter().rect_stroke(paper,2.,Stroke::new(1.5,MUTED),egui::StrokeKind::Inside);}
                            if response.clicked()&&entry.trashed.is_none(){self.open_project_path(entry.file.clone());}
                            ui.add_space(6.);let title=ui.add(egui::Label::new(RichText::new(&entry.name).strong()).truncate());title.widget_info(||egui::WidgetInfo::labeled(egui::Role::Label,ui.is_enabled(),&entry.name));
                            ui.label(RichText::new(format!("{} x {}     {} fps",entry.width,entry.height,entry.fps)).small().color(MUTED));
                            ui.add(egui::Label::new(RichText::new(display_path(entry.file.parent().unwrap_or(&entry.file))).small().color(MUTED)).truncate());
                            ui.label(RichText::new(format!("Edited {} · revision {}",relative_time(entry.updated),entry.revision)).small().color(MUTED));
                            ui.label(RichText::new(format!("Opened {}",relative_time(entry.last_opened))).small().color(MUTED));
                            ui.horizontal(|ui|{
                                if let Some(trash)=&entry.trashed {if theme::button(ui,"Restore",theme::Icon::Undo,false).clicked(){let result=self.service.restore_local_project(trash);match result{Ok(id)=>{if let(Some(file),Ok(p))=(self.service.project_file(id),self.service.snapshot(id)){self.manager.record(&p,file);}self.status="Project restored".into();},Err(e)=>self.status=e.to_string()}}return;}
                                if theme::icon_button(ui,"Open project",theme::Icon::Folder,false).clicked(){self.open_project_path(entry.file.clone());}
                                if theme::icon_button(ui,if entry.favorite{"Remove favorite"}else{"Favorite"},theme::Icon::Favorite,entry.favorite).clicked(){if let Some(value)=self.manager.recent.iter_mut().find(|r|r.id==entry.id){value.favorite = !value.favorite;}let _=self.manager.persist();}
                                theme::icon_menu(ui,"Project options",theme::Icon::More,|ui|{
                                    if ui.button(i18n::tr("Rename")).clicked(){self.prepare_management(entry.clone(),1);ui.close();}
                                    if ui.button(i18n::tr("Recent changes")).clicked(){self.prepare_management(entry.clone(),3);ui.close();}
                                    if ui.button(i18n::tr("Move to Trash")).clicked(){self.prepare_management(entry.clone(),2);ui.close();}
                                    if ui.button(i18n::tr("Recover autosave")).clicked(){self.recover_project_file(&entry.file,entry.id);ui.close();}
                                    if ui.button(i18n::tr("Remove from recent")).clicked(){self.manager.recent.retain(|r|r.id!=entry.id);let _=self.manager.persist();ui.close();}
                                });
                            });
                        });
                        });
                    });
                }});ui.add_space(16.);}
            });
        });
        self.project_dialogs(&ctx);
        if self.manager.scan.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    pub(super) fn project_dialogs(&mut self, ctx: &egui::Context) {
        self.project_management_dialog(ctx);
        if self.manager.properties
            && let Ok(p) = self.service.snapshot(self.project)
        {
            let mut open = true;
            let mut apply = false;
            let modal = egui::Modal::new(egui::Id::unique("project.properties.v2"))
                .area(
                    egui::Modal::default_area(egui::Id::unique("project.properties.v2"))
                        .default_size(theme::dialog_size(ctx, Vec2::new(432., 500.))),
                )
                .show(ctx, |ui| {
                    ui.set_width(theme::dialog_size(ctx, Vec2::new(400., 400.)).x);
                    egui::ScrollArea::vertical()
                        .max_height(
                            (theme::dialog_size(ctx, Vec2::new(450., 620.)).y - 100.).max(80.),
                        )
                        .show(ui, |ui| {
                            ui.heading(i18n::tr("Project settings"));
                            ui.label(i18n::tr("Name"));
                            ui.text_edit_singleline(&mut self.manager.name);
                            ui.label(format!("{} x {}    {} fps", p.width, p.height, p.fps));
                            ui.horizontal(|ui| {
                                ui.label(i18n::tr("Timeline length"));
                                ui.add(
                                    egui::DragValue::new(&mut self.manager.property_frames)
                                        .range(1..=1_000_000)
                                        .suffix(" frames"),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.label(i18n::tr("Background"));
                                ui.color_edit_button_srgba(&mut self.manager.background);
                            });
                            ui.small(i18n::tr(
                                "Shortening is rejected if it would remove existing animation.",
                            ));
                            if ui
                                .add_enabled(
                                    !self.manager.name.trim().is_empty(),
                                    egui::Button::new("Apply"),
                                )
                                .clicked()
                            {
                                apply = true;
                            }
                            ui.label(RichText::new(&self.status).small().color(MUTED));
                        });
                });
            if modal.should_close() {
                open = false;
            }
            self.manager.properties = open;
            if apply {
                self.apply(vec![Command::SetProject {
                    name: Some(self.manager.name.trim().into()),
                    duration: Some(self.manager.property_frames),
                    background: Some(self.manager.background.to_srgba_unmultiplied()),
                }]);
                if self
                    .service
                    .snapshot(p.id)
                    .is_ok_and(|after| after.revision > p.revision)
                {
                    self.manager.properties = false;
                }
            }
        }
        if self.manager.create {
            let mut open = true;
            let mut create = false;
            let mut cancel = false;
            let modal = egui::Modal::new(egui::Id::unique("project.create.v2"))
                .area(
                    egui::Modal::default_area(egui::Id::unique("project.create.v2"))
                        .default_size(theme::dialog_size(ctx, Vec2::new(482., 640.))),
                )
                .show(ctx, |ui| {
                    ui.set_width(theme::dialog_size(ctx, Vec2::new(450., 500.)).x);
                    egui::ScrollArea::vertical()
                        .max_height(
                            (theme::dialog_size(ctx, Vec2::new(450., 620.)).y - 100.).max(80.),
                        )
                        .show(ui, |ui| {
                            ui.heading(i18n::tr("New project"));
                            ui.add_space(6.);
                            ui.label(i18n::tr("Name"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.manager.name)
                                    .desired_width(f32::INFINITY),
                            );
                            ui.add_space(6.);
                            ui.horizontal(|ui| {
                                ui.label(i18n::tr("Canvas"));
                                for (label, w, h) in [
                                    ("Landscape", 1920, 1080),
                                    ("Portrait", 1080, 1920),
                                    ("Square", 1080, 1080),
                                ] {
                                    if ui.button(label).clicked() {
                                        self.manager.width = w;
                                        self.manager.height = h;
                                    }
                                }
                            });
                            egui::Grid::new("project.setup")
                                .spacing([18., 12.])
                                .show(ui, |ui| {
                                    ui.label(i18n::tr("Width"));
                                    ui.add(
                                        egui::DragValue::new(&mut self.manager.width)
                                            .range(1..=8192)
                                            .suffix(" px"),
                                    );
                                    ui.end_row();
                                    ui.label(i18n::tr("Height"));
                                    ui.add(
                                        egui::DragValue::new(&mut self.manager.height)
                                            .range(1..=8192)
                                            .suffix(" px"),
                                    );
                                    ui.end_row();
                                    ui.label(i18n::tr("Frame rate"));
                                    ui.horizontal(|ui| {
                                        for fps in [12, 24, 30, 60] {
                                            ui.selectable_value(
                                                &mut self.manager.fps,
                                                fps,
                                                fps.to_string(),
                                            );
                                        }
                                        ui.add(
                                            egui::DragValue::new(&mut self.manager.fps)
                                                .range(1..=240),
                                        );
                                    });
                                    ui.end_row();
                                    ui.label(i18n::tr("Duration"));
                                    ui.add(
                                        egui::DragValue::new(&mut self.manager.seconds)
                                            .range(1..=600)
                                            .suffix(" seconds"),
                                    );
                                    ui.end_row();
                                    ui.label(i18n::tr("Background"));
                                    ui.color_edit_button_srgba(&mut self.manager.background);
                                    ui.end_row();
                                });
                            ui.add_space(8.);
                            ui.label(i18n::tr("Project folder"));
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(display_path(&self.manager.folder)).truncate(),
                                );
                                if theme::icon_button(
                                    ui,
                                    "Choose project folder",
                                    theme::Icon::Folder,
                                    false,
                                )
                                .clicked()
                                    && let Some(folder) = rfd::FileDialog::new()
                                        .set_directory(&self.manager.folder)
                                        .pick_folder()
                                {
                                    self.manager.folder = folder;
                                }
                            });
                            ui.small(i18n::tr("Each project gets its own folder. Linked media travels with Save As."),
                        );
                        });
                    ui.add_space(10.);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.manager.name.trim().is_empty(),
                                egui::Button::new(
                                    RichText::new("Create project").color(theme::ON_ACCENT),
                                )
                                .fill(theme::ACCENT),
                            )
                            .clicked()
                        {
                            create = true;
                        }
                        if ui.button(i18n::tr("Cancel")).clicked() {
                            cancel = true;
                        }
                    });
                });
            if modal.should_close() {
                open = false;
            }
            self.manager.create = open && !cancel;
            if create {
                match self.create_managed_project() {
                    Ok(()) => self.manager.create = false,
                    Err(e) => self.status = format!("{e:#}"),
                }
            }
        }
        if let Some(id) = self.manager.close {
            let name = self
                .service
                .snapshot(id)
                .map(|p| p.name)
                .unwrap_or_default();
            let modal = egui::Modal::new(egui::Id::unique("project.close")).show(ctx, |ui| {
                ui.set_width(theme::dialog_size(ctx, Vec2::new(420., 400.)).x);
                ui.heading(i18n::tr("Save project changes?"));
                ui.label(format!("Save changes to {name} before closing?"));
                ui.horizontal(|ui| {
                    if ui.button(i18n::tr("Save and close")).clicked() {
                        self.switch_project(id);
                        self.save_project();
                        if self
                            .service
                            .snapshot(id)
                            .is_ok_and(|p| self.saved_revision == Some(p.revision))
                        {
                            self.finish_close_project(id);
                        }
                    }
                    if ui.button(i18n::tr("Discard changes")).clicked() {
                        self.finish_close_project(id);
                    }
                    if ui.button(i18n::tr("Cancel")).clicked() {
                        self.manager.close = None;
                    }
                });
            });
            if modal.should_close() {
                self.manager.close = None;
            }
        }
    }
    fn create_managed_project(&mut self) -> Result<()> {
        let mut p = Project::new(
            self.manager.name.trim(),
            self.manager.width,
            self.manager.height,
            self.manager.fps,
            self.manager.seconds * self.manager.fps,
        );
        p.background = self.manager.background.to_srgba_unmultiplied();
        anyhow::ensure!(
            p.width as u64 * p.height as u64 <= 32 * 1024 * 1024,
            "Canvas exceeds the 32-megapixel budget"
        );
        let layer = DrawingLayer::new("Layer 1");
        p.cels.push(DrawingCel {
            id: Id::new_v4(),
            layer: layer.id,
            frame: 0,
            strokes: vec![],
        });
        p.layers.push(layer);
        p.schema = 3;
        p.validate()?;
        let slug: String = p
            .name
            .chars()
            .take(64)
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let folder = self
            .manager
            .folder
            .join(format!("{}-{}", slug, &p.id.to_string()[..8]));
        let file = folder.join(format!("{slug}.astral"));
        let id = self.service.insert(p)?;
        if let Err(e) = self.service.save_local_project_as(id, &file) {
            let _ = self.service.close_local_project(id);
            return Err(e);
        }
        self.switch_project(id);
        self.accept_project_file(file)?;
        self.manager.home = false;
        self.preferences.mode = WorkspaceMode::Draw;
        self.preferences.advanced_timeline = false;
        self.status = "Project created".into();
        if let Some(file) = self.assist.deferred_image.take() {
            self.begin_stickman_import(file);
        }
        Ok(())
    }
    pub(super) fn accept_project_file(&mut self, file: PathBuf) -> Result<()> {
        let file = file.canonicalize()?;
        let p = self.service.snapshot(self.project)?;
        self.save_path = file
            .strip_prefix(self.service.project_workspace(self.project).root())
            .context("Save path is outside the project folder")?
            .to_string_lossy()
            .into();
        self.saved_revision = Some(p.revision);
        self.dirty = false;
        self.remember_document();
        self.manager.record(&p, file);
        self.selected_layer = p.layers.last().map(|l| l.id);
        self.layer_rename = p.layers.last().map(|l| l.name.clone()).unwrap_or_default();
        self.view_zoom = 1.;
        self.view_pan = Vec2::ZERO;
        self.frame_cache.clear();
        Ok(())
    }
    pub(super) fn open_project_path(&mut self, file: PathBuf) {
        let result = (|| -> Result<()> {
            let canonical = file.canonicalize()?;
            let already_open = self
                .service
                .store
                .lock()
                .unwrap()
                .files
                .values()
                .any(|p| *p == canonical);
            let id = self.service.open_local_project(&file)?;
            self.switch_project(id);
            if !already_open {
                let file = self
                    .service
                    .project_file(id)
                    .context("Project location missing")?;
                self.accept_project_file(file)?;
            }
            self.manager.home = false;
            self.status = "Project opened".into();
            Ok(())
        })();
        if let Err(e) = result {
            self.status = format!("{e:#}");
        }
    }
    pub(super) fn save_project_as(&mut self) {
        if !self.flush_storyboard() {
            return;
        }
        let p = match self.service.snapshot(self.project) {
            Ok(p) => p,
            Err(_) => return,
        };
        if let Some(file) = rfd::FileDialog::new()
            .set_title("Save project as")
            .set_directory(self.service.project_workspace(self.project).root())
            .set_file_name(format!(
                "{}.astral",
                p.name
                    .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
            ))
            .add_filter("Astral project", &["astral"])
            .save_file()
        {
            let file = file.with_extension("astral");
            match self
                .service
                .save_local_project_as(self.project, &file)
                .and_then(|path| self.accept_project_file(path))
            {
                Ok(()) => self.status = "Project saved".into(),
                Err(e) => self.status = format!("{e:#}"),
            }
        }
    }
    fn recover_project_file(&mut self, file: &std::path::Path, id: Id) {
        let result = (|| -> Result<()> {
            anyhow::ensure!(
                !self
                    .service
                    .store
                    .lock()
                    .unwrap()
                    .sessions
                    .contains_key(&id),
                "Close the open project before recovering its autosave"
            );
            let workspace =
                astral_storage::Workspace::new(file.parent().context("Missing project folder")?)?;
            let p = workspace.recover(id)?;
            let id = self.service.insert(p)?;
            {
                let mut store = self.service.store.lock().unwrap();
                store.locations.insert(id, workspace);
                store.files.insert(id, file.to_path_buf());
            }
            self.switch_project(id);
            self.save_path = file.file_name().unwrap().to_string_lossy().into();
            self.saved_revision = None;
            self.dirty = true;
            self.manager.home = false;
            self.selected_layer = self.service.snapshot(id)?.layers.last().map(|l| l.id);
            self.status = "Autosave recovered. Save to keep these changes.".into();
            Ok(())
        })();
        if let Err(e) = result {
            self.status = format!("{e:#}");
        }
    }
    pub(super) fn request_close_project(&mut self, id: Id) {
        if !self.flush_storyboard() {
            return;
        }
        let saved = if id == self.project {
            self.saved_revision
        } else {
            self.documents.get(&id).and_then(|v| v.1)
        };
        if self
            .service
            .snapshot(id)
            .is_ok_and(|p| saved != Some(p.revision))
        {
            self.manager.close = Some(id);
        } else {
            self.finish_close_project(id);
        }
    }
    pub(super) fn finish_close_project(&mut self, id: Id) {
        let _ = self.service.close_local_project(id);
        self.documents.remove(&id);
        self.manager.close = None;
        if self.project == id {
            let next = self
                .service
                .store
                .lock()
                .unwrap()
                .sessions
                .keys()
                .copied()
                .find(|id| Some(*id) != self.manager.placeholder);
            if let Some(next) = next {
                self.switch_project(next);
            } else {
                let p = Project::new("Untitled", 1920, 1080, 24, 120);
                if let Ok(id) = self.service.insert(p) {
                    self.project = id;
                    self.manager.placeholder = Some(id);
                    self.saved_revision = None;
                    self.manager.home = true;
                    self.save_path.clear();
                }
            }
        }
        self.documents.remove(&id);
    }
}

fn relative_time(timestamp: u64) -> String {
    if timestamp == 0 {
        return "unknown".into();
    }
    let elapsed = now().saturating_sub(timestamp);
    if elapsed < 60 {
        "just now".into()
    } else if elapsed < 3600 {
        format!("{} min ago", elapsed / 60)
    } else if elapsed < 86400 {
        format!("{} h ago", elapsed / 3600)
    } else {
        format!("{} days ago", elapsed / 86400)
    }
}
impl Studio {
    fn prepare_management(&mut self, entry: RecentProject, action: u8) {
        match self.service.open_local_project(&entry.file) {
            Ok(id) => {
                self.manager.manage_name = self
                    .service
                    .snapshot(id)
                    .map(|p| p.name)
                    .unwrap_or(entry.name.clone());
                self.manager.activity = self.service.project_activity(id).unwrap_or_default();
                self.manager.manage = Some((entry, action));
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn project_management_dialog(&mut self, ctx: &egui::Context) {
        let Some((entry, action)) = self.manager.manage.clone() else {
            return;
        };
        let mut open = true;
        let mut commit = false;
        egui::Window::new(match action{1=>"Rename project",2=>"Move project to Trash",_=>"Recent changes"}).open(&mut open).collapsible(false).default_width(460.).max_size(theme::dialog_size(ctx,Vec2::new(560.,620.))).vscroll(true).show(ctx,|ui|{
            ui.label(RichText::new(&entry.name).strong());
            match action{
                1=>{ui.add(egui::TextEdit::singleline(&mut self.manager.manage_name).desired_width(ui.available_width()));ui.small(i18n::tr("Changes the project title and saves current edits. Filename and linked media stay unchanged."));if ui.add_enabled(!self.manager.manage_name.trim().is_empty(),egui::Button::new("Rename and save")).clicked(){commit=true;}},
                2=>{ui.label(i18n::tr("Save current edits, close this project and move its .astral file to Astral Trash. Linked media stays in place. Restore it from the library's Trash view."));if ui.button(i18n::tr("Save and move to Trash")).clicked(){commit=true;}},
                _=>{ui.small(i18n::tr("Last 200 local edits. This records changed sections, not complete snapshots or a line-by-line diff."));if self.manager.activity.is_empty(){ui.label(i18n::tr("No recorded edits yet. New changes will appear here."));}for event in self.manager.activity.iter().rev(){egui::Frame::new().fill(theme::CONTROL).corner_radius(8).inner_margin(10.).show(ui,|ui|{ui.label(format!("{} · revision {} · {}",event.action,event.revision,relative_time(event.timestamp)));ui.label(event.changes.join(", "));});}},
            }
        });
        if commit {
            let result = (|| -> Result<()> {
                let p = self.service.snapshot(entry.id)?;
                if action == 1 {
                    self.service.rename_local_project(
                        entry.id,
                        p.revision,
                        self.manager.manage_name.clone(),
                    )?;
                    let file = self.service.save_local_project_as(entry.id, &entry.file)?;
                    let p = self.service.snapshot(entry.id)?;
                    self.manager.record(&p, file);
                    self.status = "Project renamed and saved".into();
                } else {
                    let trash = self.service.trash_local_project(entry.id, p.revision)?;
                    if let Some(recent) = self.manager.recent.iter_mut().find(|r| r.id == entry.id)
                    {
                        recent.trashed = Some(trash);
                    }
                    self.manager.persist()?;
                    self.finish_close_project(entry.id);
                    self.manager.home = true;
                    self.status = "Project moved to Trash; restore it from the library".into();
                }
                Ok(())
            })();
            match result {
                Ok(()) => open = false,
                Err(e) => self.status = e.to_string(),
            }
        }
        if !open {
            self.manager.manage = None;
        }
    }
}

impl ProjectManager {
    fn refresh_recent(&mut self) {
        for recent in &mut self.recent {
            if let Ok(meta) = std::fs::metadata(&recent.file) {
                recent.updated = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                if meta.len() <= 32 * 1024 * 1024
                    && let Ok(p) = std::fs::read(&recent.file)
                        .map_err(anyhow::Error::from)
                        .and_then(|bytes| astral_storage::decode(&bytes))
                {
                    recent.name = p.name;
                    recent.revision = p.revision;
                    recent.trashed = None;
                }
            } else if recent.trashed.is_none()
                && let Some(parent) = recent.file.parent()
            {
                let suffix = format!(
                    "--{}",
                    recent
                        .file
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
                recent.trashed = std::fs::read_dir(parent.join(".astral-trash"))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .take(512)
                    .find(|entry| entry.file_name().to_string_lossy().ends_with(&suffix))
                    .map(|entry| entry.path());
            }
        }
        self.thumbnails.clear();
        let _ = self.persist();
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn populated_library_cards_stack_preview_text_and_actions() {
        for (size, count) in [
            (Vec2::new(800., 520.), 1),
            (Vec2::new(960., 620.), 4),
            (Vec2::new(1280., 800.), 7),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let service = Service::new(dir.path()).unwrap();
            let id = service
                .insert(Project::new("Placeholder", 1920, 1080, 24, 120))
                .unwrap();
            let ctx = egui::Context::default();
            theme::install(&ctx);
            ctx.enable_accesskit();
            let mut studio = Studio::from_context(ctx.clone(), service, id, Preferences::default());
            studio.manager.scan = None;
            studio.manager.settings = dir.path().join("settings");
            studio.manager.recent = (0..count)
                .map(|i| RecentProject {
                    id: Id::new_v4(),
                    file: dir
                        .path()
                        .join(format!("a-very-long-project-folder-name-{i}/film.astral")),
                    name: format!("Film {i} with a long title that must stay inside its card"),
                    width: 1920,
                    height: 1080,
                    fps: 24,
                    updated: i as u64,
                    revision: 0,
                    favorite: false,
                    last_opened: 0,
                    trashed: None,
                })
                .collect();
            let mut nodes = vec![];
            for _ in 0..4 {
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ctx| {
                        let mut ui = egui::Ui::new(
                            ctx.clone(),
                            egui::Id::unique("library-layout"),
                            egui::UiBuilder::new().max_rect(ctx.content_rect()),
                        );
                        studio.show_projects(&mut ui);
                    },
                );
                output.textures_delta.clear();
                nodes = output.platform_output.accesskit_update.unwrap().nodes;
            }
            let status = nodes
                .iter()
                .find(|(_, n)| n.value() == Some(studio.status.as_str()))
                .expect("Library status footer")
                .1
                .bounds()
                .unwrap();
            assert!(
                status.y0 >= 0. && status.y1 <= size.y as f64,
                "Status footer clipped by cards"
            );
            let mut previews = vec![];
            for entry in &studio.manager.recent {
                let name = format!("Preview {}", entry.name);
                let preview = nodes
                    .iter()
                    .find(|(_, n)| n.label() == Some(name.as_str()))
                    .expect("Accessible project preview")
                    .1
                    .bounds()
                    .unwrap();
                let title = nodes
                    .iter()
                    .find(|(_, n)| n.value() == Some(entry.name.as_str()))
                    .expect("Project title")
                    .1
                    .bounds()
                    .unwrap();
                assert!(
                    title.y0 >= preview.y1,
                    "Title overlaps thumbnail: {preview:?} {title:?}"
                );
                assert!(
                    title.x0 >= preview.x0 - 1. && title.x1 <= preview.x1 + 1.,
                    "Title escapes card: {title:?}"
                );
                let action = nodes
                    .iter()
                    .filter(|(_, n)| n.label() == Some("Open project"))
                    .filter_map(|(_, n)| n.bounds())
                    .filter(|r| {
                        r.x0 >= preview.x0 - 1. && r.x1 <= preview.x1 + 1. && r.y0 >= preview.y0
                    })
                    .min_by(|a, b| a.y0.total_cmp(&b.y0))
                    .expect("Card action");
                assert!(
                    action.y0 >= title.y1 && action.y0 < title.y1 + 100.,
                    "Actions do not follow metadata: {action:?} {title:?}"
                );
                assert!(
                    preview.x0 >= 0. && preview.x1 <= size.x as f64,
                    "Card escapes viewport"
                );
                previews.push(preview);
            }
            for (i, a) in previews.iter().enumerate() {
                for b in &previews[i + 1..] {
                    assert!(
                        a.x1 <= b.x0 || b.x1 <= a.x0 || a.y1 <= b.y0 || b.y1 <= a.y0,
                        "Card previews overlap"
                    );
                }
            }
        }
    }
}
