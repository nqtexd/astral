use super::*;
use crate::export::{ExportSpec, Format, Framing, Plan, Preset, Quality};

pub struct ExportPanel {
    pub open: bool,
    spec: ExportSpec,
    format: Format,
    current_frame: u32,
    range: bool,
    assembly_open: bool,
    assembly: crate::export::AssemblySpec,
    texture: Option<egui::TextureHandle>,
}
impl Default for ExportPanel {
    fn default() -> Self {
        Self {
            open: false,
            spec: ExportSpec {
                hardware: true,
                ..Default::default()
            },
            format: Format::Mp4,
            current_frame: 0,
            range: false,
            assembly_open: false,
            assembly: crate::export::AssemblySpec {
                path: "renders/finished.mp4".into(),
                clips: vec![],
                width: 1920,
                height: 1080,
                fps: 30,
            },
            texture: None,
        }
    }
}
impl ExportPanel {
    pub fn open(&mut self, p: &Project, frame: f64, ctx: &egui::Context) {
        self.open = true;
        self.current_frame = frame.floor() as u32;
        self.range = false;
        self.spec.start = None;
        self.spec.end = None;
        self.path(p);
        let scale = (512. / p.width.max(p.height) as f32).min(1.);
        self.texture = astral_render::rasterize(
            &display_list(p, frame),
            (p.width as f32 * scale).max(1.) as u32,
            (p.height as f32 * scale).max(1.) as u32,
        )
        .ok()
        .map(|pix| {
            ctx.load_texture(
                "export-preview",
                egui::ColorImage::from_rgba_premultiplied(
                    [pix.width() as usize, pix.height() as usize],
                    pix.data(),
                ),
                egui::TextureOptions::LINEAR,
            )
        });
    }
    fn path(&mut self, p: &Project) {
        self.spec.path = format!(
            "renders/{}-{}.{}",
            p.name
                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-"),
            serde_json::to_value(self.spec.preset)
                .unwrap()
                .as_str()
                .unwrap(),
            self.format.extension()
        );
    }
    pub fn show(&mut self, ctx: &egui::Context, p: &Project) -> Option<ExportSpec> {
        if !self.open {
            return None;
        }
        let mut open = true;
        let mut output = None;
        egui::Window::new(i18n::tr("Export")).id(egui::Id::unique("astral.export.v2")).open(&mut open).collapsible(false).resizable(true).default_size(theme::dialog_size(ctx,Vec2::new(680.,640.))).max_size(theme::dialog_size(ctx,Vec2::new(840.,760.))).anchor(egui::Align2::CENTER_CENTER,[0.,0.]).show(ctx,|ui| {
            egui::ScrollArea::vertical().max_height((theme::dialog_size(ctx,Vec2::new(680.,640.)).y-90.).max(120.)).show(ui,|ui|{
            if ui.button(i18n::tr("Assemble rendered shots")).on_hover_text("Join corrected endings and existing renders without an external editor").clicked(){self.assembly_open=true;}
            theme::section(ui,"DESTINATION");
            ui.add_space(8.);
            ui.horizontal_top(|ui|{
                ui.vertical(|ui|{
                    ui.set_width(338.);
                    egui::Grid::new("destinations").num_columns(2).spacing([8.,8.]).show(ui,|ui|{
                        for (i,preset) in Preset::ALL.into_iter().enumerate(){
                            if ui.add_sized([162.,36.],egui::Button::new(preset.label()).selected(self.spec.preset==preset)).clicked(){self.spec.preset=preset;self.spec.width=None;self.spec.height=None;self.spec.fps=None;self.path(p);}
                            if i%2==1{ui.end_row();}
                        }
                    });
                    ui.add_space(8.);
                    let previous=self.format;
                    egui::ComboBox::from_id_salt("export.format").selected_text(self.format.label()).width(320.).show_ui(ui,|ui|{for format in Format::ALL {ui.selectable_value(&mut self.format,format,format.label());}});
                    if previous!=self.format{self.path(p);}
                    ui.horizontal(|ui|{ui.selectable_value(&mut self.spec.framing,Framing::Fit,"Fit - keep scene");ui.selectable_value(&mut self.spec.framing,Framing::Fill,"Fill - crop edges");});
                    ui.horizontal(|ui|{ui.label(i18n::tr("Quality"));for (quality,label) in [(Quality::Draft,"Draft"),(Quality::Standard,"Standard"),(Quality::High,"High")] {ui.selectable_value(&mut self.spec.quality,quality,label);}});
                    ui.horizontal(|ui|{ui.label(i18n::tr("Renderer"));for (backend,label) in [(crate::export::RenderBackend::Auto,"Auto"),(crate::export::RenderBackend::Cpu,"CPU"),(crate::export::RenderBackend::Gpu,"GPU")] {ui.selectable_value(&mut self.spec.renderer,backend,label);}});
                    if self.format==Format::Mp4{ui.checkbox(&mut self.spec.hardware,i18n::tr("Use hardware encoder when available"));}
                });
                ui.vertical(|ui|{
                    ui.set_width(260.);
                    if let Ok(plan)=Plan::new(p,&self.spec){
                        let size=Vec2::new(plan.width as f32,plan.height as f32);let scale=(240./size.x).min(230./size.y);let size=size*scale;
                        let (outer,_)=ui.allocate_exact_size(Vec2::new(250.,240.),Sense::hover());
                        let rect=Rect::from_center_size(outer.center(),size);
                        ui.painter().rect_filled(outer,10.,theme::BACKGROUND);
                        ui.painter().rect_filled(rect,0.,Color32::BLACK);
                        if let Some(texture)=&self.texture{
                            let source=texture.size_vec2();
                            let factor=if self.spec.framing==Framing::Fit{(size.x/source.x).min(size.y/source.y)}else{(size.x/source.x).max(size.y/source.y)};
                            let image=Rect::from_center_size(rect.center(),source*factor);
                            ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).image(texture.id(),image,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);
                        }
                        ui.painter().rect_stroke(rect,0.,Stroke::new(1.,theme::BORDER),egui::StrokeKind::Inside);
                        ui.label(RichText::new(format!("{} × {}  -  {} fps",plan.width,plan.height,plan.fps)).strong());
                        ui.label(RichText::new(format!("{:.2} seconds - {} frames",plan.duration_seconds,plan.frames)).color(MUTED));
                        ui.small(i18n::tr("Framing preview at the current playhead."));
                    }
                });
            });
            ui.separator();
            if self.format.still(){self.spec.start=Some(self.current_frame);self.spec.end=Some(self.current_frame);ui.label(format!("Still image at frame {}",self.current_frame));}
            else {
                if ui.checkbox(&mut self.range,i18n::tr("Export selected source-frame range")).changed(){self.spec.start=if self.range{Some(0)}else{None};self.spec.end=if self.range{Some(p.duration-1)}else{None};}
                if self.range{ui.horizontal(|ui|{ui.label(i18n::tr("From"));ui.add(egui::DragValue::new(self.spec.start.as_mut().unwrap()).range(0..=p.duration-1));ui.label(i18n::tr("through"));ui.add(egui::DragValue::new(self.spec.end.as_mut().unwrap()).range(0..=p.duration-1));});}
                else{self.spec.start=None;self.spec.end=None;}
            }
            ui.collapsing(i18n::tr("Custom dimensions and frame rate"),|ui|{
                let defaults=Plan::new(p,&self.spec).ok();let (w,h)=defaults.as_ref().map(|plan|(plan.width,plan.height)).unwrap_or(self.spec.preset.dimensions(p));let mut width=self.spec.width.unwrap_or(w);let mut height=self.spec.height.unwrap_or(h);let mut fps=self.spec.fps.unwrap_or(defaults.as_ref().map(|plan|plan.fps).unwrap_or(self.spec.preset.fps(p)));
                ui.horizontal(|ui|{ui.label(i18n::tr("Width"));if ui.add(egui::DragValue::new(&mut width).range(1..=8192)).changed(){self.spec.width=Some(width);}ui.label(i18n::tr("Height"));if ui.add(egui::DragValue::new(&mut height).range(1..=8192)).changed(){self.spec.height=Some(height);}ui.label(i18n::tr("FPS"));if ui.add(egui::DragValue::new(&mut fps).range(1..=240)).changed(){self.spec.fps=Some(fps);}});
                if ui.small_button("Reset to preset").clicked(){self.spec.width=None;self.spec.height=None;self.spec.fps=None;}
            });
            ui.label(i18n::tr("File in project folder"));ui.add(egui::TextEdit::singleline(&mut self.spec.path).desired_width(f32::INFINITY));
            ui.small(i18n::tr("Existing files are replaced after a successful export."));
            match Plan::new(p,&self.spec){
                Ok(plan) if plan.format != self.format => {ui.colored_label(Color32::LIGHT_RED,format!("Use .{} for the selected format, or choose a different format above.",self.format.extension()));}
                Ok(plan)=>{for warning in &plan.warnings{ui.colored_label(Color32::from_rgb(242,199,116),warning);}}
                Err(e)=>{ui.colored_label(Color32::LIGHT_RED,e.to_string());}
            }
            });
            ui.separator();
            let ready=Plan::new(p,&self.spec).is_ok_and(|plan|plan.format==self.format);
            ui.add_enabled_ui(ready,|ui|{ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if theme::button(ui,"Export",theme::Icon::Export,true).clicked(){output=Some(self.spec.clone());}});});
        });
        self.open = open && output.is_none();
        output
    }
}

impl ExportPanel {
    pub fn show_assembly(
        &mut self,
        ctx: &egui::Context,
        workspace: &astral_storage::Workspace,
    ) -> Option<crate::export::AssemblySpec> {
        if !self.assembly_open {
            return None;
        }
        let mut open = true;
        let mut output = None;
        egui::Window::new(i18n::tr("Assemble shots")).open(&mut open).default_width(560.).max_size(theme::dialog_size(ctx,Vec2::new(720.,700.))).vscroll(true).show(ctx,|ui|{
            ui.label(i18n::tr("Hard cuts with synchronized audio. Set each source trim in seconds; all shots are normalized and re-encoded."));
            let mut move_up=None;let mut remove=None;
            for (i,clip) in self.assembly.clips.iter_mut().enumerate() {
                ui.push_id(i,|ui|{ui.separator();ui.horizontal_wrapped(|ui|{ui.label(format!("Shot {}",i+1));if i>0&&theme::icon_button(ui,"Move earlier",theme::Icon::Previous,false).clicked(){move_up=Some(i);}
if theme::icon_button(ui,"Remove shot",theme::Icon::Delete,false).clicked(){remove=Some(i);}});
                    ui.add(egui::TextEdit::singleline(&mut clip.path).desired_width(ui.available_width()).hint_text("Relative workspace video path"));
                    ui.horizontal_wrapped(|ui|{ui.add(egui::DragValue::new(&mut clip.start_seconds).range(0.0..=36000.).speed(0.1).prefix("In ").suffix(" s"));ui.add(egui::DragValue::new(&mut clip.end_seconds).range(0.0..=36000.).speed(0.1).prefix("Out ").suffix(" s"));ui.checkbox(&mut clip.audio,i18n::tr("Source audio"));});
                });
            }
            if let Some(i)=move_up {self.assembly.clips.swap(i,i-1);}
if let Some(i)=remove{self.assembly.clips.remove(i);}
            if ui.add_enabled(self.assembly.clips.len()<32,egui::Button::new("Add shot")).clicked(){self.assembly.clips.push(crate::export::AssemblyClip{path:String::new(),start_seconds:0.,end_seconds:1.,audio:true});}
            ui.separator();ui.label(i18n::tr("Output (relative workspace path)"));ui.text_edit_singleline(&mut self.assembly.path);
            ui.horizontal_wrapped(|ui|{ui.add(egui::DragValue::new(&mut self.assembly.width).range(16..=3840).speed(2.).prefix("Width "));ui.add(egui::DragValue::new(&mut self.assembly.height).range(16..=3840).speed(2.).prefix("Height "));ui.add(egui::DragValue::new(&mut self.assembly.fps).range(1..=60).prefix("FPS "));});
            match self.assembly.validate(workspace){Ok(())=>{if theme::button(ui,"Assemble MP4",theme::Icon::Export,true).clicked(){output=Some(self.assembly.clone());}},Err(e)=>{ui.colored_label(Color32::from_rgb(242,199,116),e.to_string());}}
        });
        self.assembly_open = open && output.is_none();
        output
    }
}
