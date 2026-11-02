use super::*;
use crate::assets::AssetRequest;
#[derive(Default)]
pub(super) struct WorkspaceTools {
    pub assets_open: bool,
    pub xsheet_open: bool,
    package_path: String,
    unpack_path: String,
    unpack_folder: String,
    pending: Option<std::sync::mpsc::Receiver<anyhow::Result<serde_json::Value>>>,
    job: Option<Id>,
    range_start: u32,
    range_end: u32,
}
impl Studio {
    fn media_request(&mut self, request: AssetRequest) {
        if self.workspace_tools.pending.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.workspace_tools.pending = Some(rx);
        let service = self.service.clone();
        std::thread::spawn(move || {
            let _ = tx.send(service.asset_tools(request));
        });
    }
    pub(super) fn workspace_tools_windows(&mut self, ctx: &egui::Context, p: &Project) {
        if let Some(rx) = &self.workspace_tools.pending
            && let Ok(result) = rx.try_recv()
        {
            self.workspace_tools.pending = None;
            match result {
                Ok(value) => {
                    if let Some(job) = value
                        .get("job")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                    {
                        self.workspace_tools.job = Some(job);
                    }
                    self.status = format!("Media: {value}");
                }
                Err(e) => self.status = format!("Media: {e}"),
            }
        }
        if self.manager.home {
            return;
        }
        let mut open = self.workspace_tools.assets_open;
        if open {
            egui::Window::new("Media and packages").open(&mut open).default_size(theme::dialog_size(ctx,Vec2::new(600.,500.))).max_size(theme::dialog_size(ctx,Vec2::new(800.,650.))).vscroll(true).show(ctx,|ui|{
   ui.small("Linked media stays separate from embedded artwork. Relinking preserves IDs and timeline timing. Re-import to change embedded pixels.");
   let workspace=self.service.project_workspace(p.id);
   for asset in &p.assets {ui.push_id(asset.id,|ui|{egui::Frame::group(ui.style()).show(ui,|ui|{
    let available=workspace.path(&asset.path).is_ok_and(|file|file.is_file());
    ui.horizontal_wrapped(|ui|{ui.label(RichText::new(&asset.name).strong());ui.label(if available{"Available"}else{"Missing"});
     if ui.button("Relink…").clicked() && self.workspace_tools.pending.is_none() && let Some(file)=rfd::FileDialog::new().pick_file(){
      let service=self.service.clone();let project=p.id;let revision=p.revision;let id=asset.id;let (tx,rx)=std::sync::mpsc::channel();self.workspace_tools.pending=Some(rx);
      std::thread::spawn(move||{let result=(||->anyhow::Result<serde_json::Value>{anyhow::ensure!(file.metadata()?.len()<=astral_storage::MAX_PROJECT_BYTES as u64,"Replacement exceeds 256 MiB");let name=file.file_name().unwrap_or_default().to_string_lossy();let path=format!("assets/relinked/{}-{name}",Id::new_v4());service.project_workspace(project).atomic_write(&path,&std::fs::read(file)?)?;service.asset_tools(AssetRequest::Relink{project,expected_revision:revision,asset:id,path})})();let _=tx.send(result);});
     }
    });ui.add(egui::Label::new(&asset.path).truncate()).on_hover_text(&asset.path);
   });});}
   ui.separator();ui.heading("Portable project");
   if self.workspace_tools.package_path.is_empty(){self.workspace_tools.package_path=format!("exports/{}-r{}.astralpack",p.id,p.revision);}
   ui.label("Package path, relative to this project's folder");ui.text_edit_singleline(&mut self.workspace_tools.package_path);
   if ui.add_enabled(self.workspace_tools.pending.is_none() && self.workspace_tools.job.is_none(),egui::Button::new("Create verified package")).clicked(){self.media_request(AssetRequest::Package{project:p.id,expected_revision:p.revision,path:self.workspace_tools.package_path.clone()});}
   ui.separator();ui.label("Unpack a package in the configured automation workspace");ui.text_edit_singleline(&mut self.workspace_tools.unpack_path).on_hover_text("Workspace-relative .astralpack path");ui.text_edit_singleline(&mut self.workspace_tools.unpack_folder).on_hover_text("New workspace-relative folder");
   if ui.add_enabled(!self.workspace_tools.unpack_path.is_empty() && !self.workspace_tools.unpack_folder.is_empty() && self.workspace_tools.job.is_none(),egui::Button::new("Verify and unpack")).clicked(){self.media_request(AssetRequest::Unpack{path:self.workspace_tools.unpack_path.clone(),destination:self.workspace_tools.unpack_folder.clone()});}
   if let Some(job)=self.workspace_tools.job && let Ok(value)=self.service.request(Request::JobStatus{job}){ui.label(value.to_string());let state=value["state"].as_str().unwrap_or("");if ["complete","completed","failed","cancelled"].contains(&state){self.workspace_tools.job=None;}else{ctx.request_repaint_after(Duration::from_millis(100));if ui.button("Cancel").clicked(){let _=self.service.request(Request::CancelJob{job});}}}
   if self.workspace_tools.pending.is_some(){ui.spinner();ctx.request_repaint_after(Duration::from_millis(50));}
   ui.add(egui::Label::new(&self.status).wrap());
  });
        }
        self.workspace_tools.assets_open = open;
        let mut open = self.workspace_tools.xsheet_open;
        if open {
            egui::Window::new("Exposure sheet").open(&mut open).default_size(theme::dialog_size(ctx,Vec2::new(760.,580.))).max_size(theme::dialog_size(ctx,Vec2::new(1000.,760.))).show(ctx,|ui|{
   ui.small("Frame numbers start at 1. ● drawing key · ↳ hold · — blank. Click a cell to select its layer and frame. Shift-click extends the frame range.");
   ui.horizontal_wrapped(|ui|{ui.label(format!("Selection {}–{}",self.workspace_tools.range_start+1,self.workspace_tools.range_end+1));
    if let Some(layer)=self.selected_layer.filter(|id|p.layers.iter().any(|l|l.id==*id&&!l.locked)){
     if ui.button("Expose drawing through range").clicked(){let start=self.workspace_tools.range_start.min(p.duration-1);let end=self.workspace_tools.range_end.max(start).min(p.duration-1);let strokes=p.cel_at(layer,start).map_or(vec![],|c|c.strokes.clone());let restore=p.cel_at(layer,end.saturating_add(1)).map_or(vec![],|c|c.strokes.clone());let mut cels=p.cels.iter().filter(|c|c.layer==layer && (c.frame<start||c.frame>end)).cloned().collect::<Vec<_>>();cels.push(DrawingCel{id:Id::new_v4(),layer,frame:start,strokes});if end+1<p.duration && !cels.iter().any(|c|c.frame==end+1){cels.push(DrawingCel{id:Id::new_v4(),layer,frame:end+1,strokes:restore});}cels.sort_by_key(|c|c.frame);self.apply(vec![Command::SetCels{layer,cels}]);}
     if ui.button("Blank selected frames").clicked(){let start=self.workspace_tools.range_start;let end=self.workspace_tools.range_end;let mut commands=vec![];for frame in start..=end.min(p.duration-1){commands.push(Command::SetFrameDrawing{layer,frame,strokes:vec![]});}if commands.len()<=10000{self.apply(commands);}else{self.status="Select at most 10,000 frames".into();}}
    }
   });
   egui::ScrollArea::both().id_salt("xsheet.scroll").show(ui,|ui|{
    ui.horizontal(|ui|{ui.add_sized([88.,24.],egui::Label::new("Frame / time"));for layer in &p.layers{ui.add_sized([100.,24.],egui::Label::new(&layer.name).truncate()).on_hover_text(&layer.name);}ui.add_sized([180.,24.],egui::Label::new("Dialogue / sound / markers"));});
    egui::ScrollArea::vertical().max_height(440.).show_rows(ui,26.,p.duration as usize,|ui,rows|{for frame in rows {let frame=frame as u32;ui.horizontal(|ui|{ui.add_sized([88.,24.],egui::Label::new(format!("{} · {:.2}s",frame+1,frame as f32/p.fps as f32)));for layer in &p.layers{let cel=p.cel_at(layer.id,frame);let label=cel.map_or("—",|c|if c.strokes.is_empty(){"—"}else if c.frame==frame{"●"}else{"↳"});let selected=self.selected_layer==Some(layer.id)&&frame>=self.workspace_tools.range_start&&frame<=self.workspace_tools.range_end;let response=ui.add_sized([100.,24.],egui::Button::new(label).selected(selected));if response.clicked(){if ui.input(|i|i.modifiers.shift){self.workspace_tools.range_end=frame.max(self.workspace_tools.range_start);}else{self.workspace_tools.range_start=frame;self.workspace_tools.range_end=frame;}self.selected_layer=Some(layer.id);self.seek_frame(frame);}}
      let mut notes=p.markers.iter().filter(|(_,f)|**f==frame).map(|(n,_)|n.clone()).collect::<Vec<_>>();notes.extend(p.dialogues.iter().filter(|d|d.start==frame).map(|d|d.text.clone()));notes.extend(p.audio.iter().filter(|a|a.start==frame).map(|a|format!("♪ {}",a.name)));ui.add_sized([180.,24.],egui::Label::new(notes.join(" · ")).truncate());});}});
   });
  });
        }
        self.workspace_tools.xsheet_open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_and_exposure_controls_fit_a_compact_window() {
        let (_dir, mut studio, _) = super::super::tests::studio();
        let ctx = studio.context.clone();
        theme::install(&ctx);
        ctx.enable_accesskit();
        for media in [true, false] {
            studio.workspace_tools.assets_open = media;
            studio.workspace_tools.xsheet_open = !media;
            let mut nodes = vec![];
            for _ in 0..4 {
                let p = studio.service.snapshot(studio.project).unwrap();
                let mut out = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 520.))),
                        ..Default::default()
                    },
                    |ctx| studio.workspace_tools_windows(ctx, &p),
                );
                out.textures_delta.clear();
                nodes = out.platform_output.accesskit_update.unwrap().nodes;
            }
            let label = if media {
                "Create verified package"
            } else {
                "Expose drawing through range"
            };
            let node = nodes
                .iter()
                .find(|(_, n)| n.role() == egui::Role::Button && n.label() == Some(label))
                .unwrap_or_else(|| panic!("Missing {label}"));
            let r = node.1.bounds().unwrap();
            assert!(
                r.x0 >= 0. && r.y0 >= 0. && r.x1 <= 800. && r.y1 <= 520.,
                "{label} escapes window: {r:?}"
            );
        }
    }
}
