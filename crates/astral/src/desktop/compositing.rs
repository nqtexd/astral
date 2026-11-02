use super::*;
impl Studio {
    pub(super) fn compositing_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        ui.collapsing(i18n::tr("Compositing"),|ui|{
   if ui.add_enabled(!p.rasters.is_empty(),egui::Button::new(i18n::tr("Compose artwork layers"))).clicked(){
    let base=Id::new_v4();let mut nodes=vec![CompositeNode{id:base,name:"Transparent".into(),op:CompositeOp::Solid{color:[0,0,0,0]}}];let mut output=base;
    for r in &p.rasters {let source=Id::new_v4();nodes.push(CompositeNode{id:source,name:r.name.clone(),op:CompositeOp::Raster{layer:r.id}});let id=Id::new_v4();nodes.push(CompositeNode{id,name:format!("Blend {}",r.name),op:CompositeOp::Blend{below:output,above:source,mode:BlendMode::Normal,opacity:1.}});output=id;}
    let mut compositions=p.compositions.clone();compositions.push(Composition{id:Id::new_v4(),name:"Composition".into(),enabled:true,layer:GraphicLayer::Background,output,nodes});self.apply(vec![Command::SetCompositions{compositions}]);
   }
   if ui.button("New empty composition").clicked(){let id=Id::new_v4();let mut compositions=p.compositions.clone();compositions.push(Composition{id:Id::new_v4(),name:"Composition".into(),enabled:true,layer:GraphicLayer::Foreground,output:id,nodes:vec![CompositeNode{id,name:"Transparent".into(),op:CompositeOp::Solid{color:[0,0,0,0]}}]});self.apply(vec![Command::SetCompositions{compositions}]);}
   for graph in &p.compositions {ui.push_id(graph.id,|ui|{ui.collapsing(&graph.name,|ui|{
    let mut next=graph.clone();let mut changed=ui.checkbox(&mut next.enabled,"Enabled").changed();changed|=ui.text_edit_singleline(&mut next.name).changed();
    let mut foreground=next.layer==GraphicLayer::Foreground;if ui.checkbox(&mut foreground,"Above scene").changed(){next.layer=if foreground{GraphicLayer::Foreground}else{GraphicLayer::Background};changed=true;}
    let names:Vec<_>=next.nodes.iter().map(|n|(n.id,n.name.clone())).collect();
    for n in &mut next.nodes {egui::Frame::group(ui.style()).show(ui,|ui|{ui.push_id(n.id,|ui|{
     ui.horizontal(|ui|{let width=(ui.available_width()-34.).max(20.);ui.add_sized([width,24.],egui::Label::new(RichText::new(&n.name).strong()).truncate()).on_hover_text(&n.name);if theme::icon_button(ui,"Set composition output",theme::Icon::Check,next.output==n.id).clicked(){next.output=n.id;changed=true;}});
     let choose=|ui:&mut egui::Ui,label:&str,value:&mut Id|->bool{let mut edit=false;egui::ComboBox::from_id_salt(label).selected_text(format!("{label}: {}",names.iter().find(|(id,_)|id==value).map_or("Missing",|(_,name)|name.as_str()))).width(ui.available_width().min(180.)).show_ui(ui,|ui|{for(id,name)in &names{if *id!=n.id{edit|=ui.selectable_value(value,*id,name).changed();}}});edit};
     match &mut n.op {
      CompositeOp::Cinematic{source,stage}=>{changed|=choose(ui,"Source",source);changed|=cinematic_controls(ui,stage,self.frame as u32);},
      CompositeOp::Comic{source,style}=>{
       changed|=choose(ui,"Source",source);
       changed|=comic_controls(ui,style,self.frame as u32);
      },
      CompositeOp::Blend{below,above,mode,opacity}|CompositeOp::Clip{below,above,mode,opacity}=>{changed|=choose(ui,"Below",below);changed|=choose(ui,"Above",above);egui::ComboBox::from_id_salt("blend").selected_text(format!("{mode:?}")).show_ui(ui,|ui|{for value in BlendMode::ALL{changed|=ui.selectable_value(mode,value,format!("{value:?}")).changed();}});changed|=ui.add(egui::Slider::new(opacity,0.0..=1.).text("Opacity")).changed();},
      CompositeOp::Mask{source,mask,invert,luminance}=>{changed|=choose(ui,"Source",source);changed|=choose(ui,"Mask",mask);changed|=ui.checkbox(invert,"Invert").changed();changed|=ui.checkbox(luminance,"Luminance").changed();},
      CompositeOp::Blur{source,radius}=>{changed|=choose(ui,"Source",source);changed|=ui.add(egui::Slider::new(radius,0..=32).text(i18n::tr("Blur"))).changed();},
      CompositeOp::Color{source,exposure,saturation}=>{changed|=choose(ui,"Source",source);changed|=ui.add(egui::Slider::new(exposure,-8.0..=8.).text(i18n::tr("Exposure"))).changed();changed|=ui.add(egui::Slider::new(saturation,0.0..=4.).text(i18n::tr("Saturation"))).changed();},
      CompositeOp::Solid{color}=>{changed|=ui.color_edit_button_srgba_unmultiplied(color).changed();},
      CompositeOp::Gradient{start,end,from,to,radial}=>{changed|=ui.color_edit_button_srgba_unmultiplied(from).changed();changed|=ui.color_edit_button_srgba_unmultiplied(to).changed();changed|=ui.checkbox(radial,"Radial").changed();for(label,point)in[("Start",start),("End",end)]{ui.label(label);changed|=ui.add(egui::Slider::new(&mut point.x,0.0..=1.).text("X")).changed();changed|=ui.add(egui::Slider::new(&mut point.y,0.0..=1.).text("Y")).changed();}},
      _=>{ui.small("Editable scene input");}
     }
    });});}
    ui.menu_button("Add scene input",|ui|{
     let mut inputs=vec![];
     inputs.extend(p.rasters.iter().map(|r|(r.name.clone(),CompositeOp::Raster{layer:r.id})));
     inputs.extend(p.layers.iter().map(|r|(r.name.clone(),CompositeOp::Drawing{layer:r.id})));
     inputs.extend(p.graphics.iter().map(|r|(format!("Graphic {}",r.id),CompositeOp::Graphic{graphic:r.id})));
     inputs.extend(p.dialogues.iter().map(|r|(format!("Text {}",r.text.chars().take(24).collect::<String>()),CompositeOp::Dialogue{dialogue:r.id})));
     for(name,op)in inputs{if ui.button(&name).clicked(){next.nodes.push(CompositeNode{id:Id::new_v4(),name,op});changed=true;ui.close();}}
    });
    ui.horizontal_wrapped(|ui|{for label in ["Mask","Blend","Clip to base"]{if ui.add_enabled(next.nodes.len()>=2,egui::Button::new(label)).clicked(){let below=next.output;let above=next.nodes.iter().rev().find(|n|n.id!=below).unwrap().id;let id=Id::new_v4();let op=match label{"Mask"=>CompositeOp::Mask{source:below,mask:above,invert:false,luminance:false},"Clip to base"=>CompositeOp::Clip{below,above,mode:BlendMode::Normal,opacity:1.},_=>CompositeOp::Blend{below,above,mode:BlendMode::Normal,opacity:1.}};next.nodes.push(CompositeNode{id,name:label.into(),op});next.output=id;changed=true;}}});
    ui.horizontal_wrapped(|ui|{for label in ["Blur","Color","Gradient fill"]{if ui.button(label).clicked(){let source=next.output;let id=Id::new_v4();let op=match label{"Blur"=>CompositeOp::Blur{source,radius:4},"Color"=>CompositeOp::Color{source,exposure:0.,saturation:1.},_=>{let gradient=Id::new_v4();next.nodes.push(CompositeNode{id:gradient,name:"Gradient".into(),op:CompositeOp::Gradient{start:Point::new(0.,0.),end:Point::new(1.,1.),from:[66,140,230,255],to:[172,93,228,255],radial:false}});CompositeOp::Mask{source:gradient,mask:source,invert:false,luminance:false}}};next.nodes.push(CompositeNode{id,name:label.into(),op});next.output=id;changed=true;}}});
    ui.horizontal_wrapped(|ui|{for label in ["RGB split","Glitch","Halftone"]{if ui.button(label).clicked(){
     let style=match label{
      "RGB split"=>ComicStyle::RgbSplit{offset:Point::new(5.,0.),strength:1.,keys:vec![]},
      "Glitch"=>ComicStyle::Glitch{displacement:20.,band_height:12,seed:42,rate:12,strength:0.7,keys:vec![]},
      _=>ComicStyle::Halftone{spacing:8.,angle:0.7853982,ink:[15,12,40,255],strength:0.7,keys:vec![]}
     };
     let id=Id::new_v4();next.nodes.push(CompositeNode{id,name:label.into(),op:CompositeOp::Comic{source:next.output,style}});next.output=id;changed=true;
    }}});
    ui.horizontal_wrapped(|ui|{for label in ["Bloom","Soft shadow","Rim light","Color grade"]{if ui.button(label).clicked(){
     let effect=match label{
      "Bloom"=>CinematicEffect::Bloom{threshold:0.65,knee:0.15,radius:18,intensity:0.6},
      "Soft shadow"=>CinematicEffect::DropShadow{offset:Point::new(8.,12.),radius:12,color:[6,9,28,255],opacity:0.6},
      "Rim light"=>CinematicEffect::RimLight{offset:Point::new(3.,-2.),radius:2,color:[255,185,110,255],intensity:0.5},
      _=>CinematicEffect::Grade{exposure:0.,contrast:1.,saturation:1.,lift:[0.;3],gamma:[1.;3],gain:[1.;3]}
     };let id=Id::new_v4();next.nodes.push(CompositeNode{id,name:label.into(),op:CompositeOp::Cinematic{source:next.output,stage:CinematicStage{effect,mix:1.,keys:vec![]}}});next.output=id;changed=true;
    }}});
    if changed{let mut compositions=p.compositions.clone();*compositions.iter_mut().find(|g|g.id==graph.id).unwrap()=next;self.apply(vec![Command::SetCompositions{compositions}]);}
    if theme::button(ui,"Remove composition",theme::Icon::Delete,false).clicked(){self.apply(vec![Command::SetCompositions{compositions:p.compositions.iter().filter(|g|g.id!=graph.id).cloned().collect()}]);}
   });});}
   ui.collapsing("Cinematic staging",|ui|{
    ui.small("Camera translation and zoom response per artwork layer. 0 locks to the anchor; 1 is normal; 2 exaggerates foreground movement.");
    let targets:Vec<_>=p.layers.iter().map(|v|(v.id,v.name.clone())).chain(p.rasters.iter().map(|v|(v.id,v.name.clone()))).chain(p.graphics.iter().map(|v|(v.id,v.name.clone()))).collect();
    for(id,name)in targets{ui.push_id((id,"depth"),|ui|{ui.collapsing(name,|ui|{
     let mut depth=p.layer_depths.get(&id).cloned().unwrap_or(LayerDepth{parallax:1.,zoom:1.,anchor:None});
     let mut change=ui.add(egui::Slider::new(&mut depth.parallax,0.0..=2.).text("Pan response")).changed();
     change|=ui.add(egui::Slider::new(&mut depth.zoom,0.0..=2.).text("Zoom response")).changed();
     if change{self.apply(vec![Command::SetLayerDepth{layer:id,depth:Some(depth)}]);}
     if p.layer_depths.contains_key(&id)&&ui.button("Reset depth").clicked(){self.apply(vec![Command::SetLayerDepth{layer:id,depth:None}]);}
    });});}
    ui.separator();ui.label("Camera segment timing");
    for key in p.camera.iter().take(p.camera.len().saturating_sub(1)){ui.push_id((key.frame,"camera-ease"),|ui|{ui.horizontal(|ui|{
     ui.label(format!("Frame {}",key.frame));let old=p.camera_easing.get(&key.frame).cloned().unwrap_or(Easing::Linear);let mut selected=match old{Easing::Smooth=>1,Easing::Hold=>2,Easing::Bezier{..}=>3,_=>0};let before=selected;
     egui::ComboBox::from_id_salt("timing").selected_text(["Linear","Ease in/out","Hold / cut","Custom Bezier"][selected]).show_ui(ui,|ui|{for (i,name) in ["Linear","Ease in/out","Hold / cut"].iter().enumerate(){ui.selectable_value(&mut selected,i,*name);}});
     if selected!=before{self.apply(vec![Command::SetCameraEasing{frame:key.frame,easing:match selected{1=>Some(Easing::Smooth),2=>Some(Easing::Hold),_=>None}}]);}
    });});}
   });
   ui.collapsing("Layer frame rates",|ui|{
    ui.small("Stepped artwork timing; camera and audio keep the project clock.");
    let targets:Vec<_>=p.layers.iter().map(|v|(v.id,v.name.clone())).chain(p.rasters.iter().map(|v|(v.id,v.name.clone()))).chain(p.graphics.iter().map(|v|(v.id,v.name.clone()))).chain(p.characters.iter().map(|v|(v.id,v.name.clone()))).collect();
    for(id,name)in targets{ui.push_id(id,|ui|{ui.horizontal(|ui|{
     ui.label(name);let mut fps=p.layer_frame_rates.get(&id).copied().unwrap_or(0);let before=fps;
     egui::ComboBox::from_id_salt("cadence").selected_text(if fps==0{"Project".into()}else{format!("{fps} FPS")}).show_ui(ui,|ui|{
      ui.selectable_value(&mut fps,0,"Project");for value in [6,8,10,12,15,24,30,60]{if value<=p.fps{ui.selectable_value(&mut fps,value,format!("{value} FPS"));}}
     });
     if fps!=before{self.apply(vec![Command::SetLayerFrameRate{layer:id,fps:if fps==0{None}else{Some(fps)}}]);}
    });});}
   });
   ui.small("Masks/blends work in premultiplied sRGB. Nodes are shared by preview and export; large graphs have a bounded memory budget.");
  });
    }
}

fn comic_controls(ui: &mut egui::Ui, style: &mut ComicStyle, frame: u32) -> bool {
    let mut changed = false;
    match style {
        ComicStyle::RgbSplit { offset, .. } => {
            changed |= ui
                .add(egui::Slider::new(&mut offset.x, -64.0..=64.).text("Red/blue X"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(&mut offset.y, -64.0..=64.).text("Red/blue Y"))
                .changed();
        }
        ComicStyle::Glitch {
            displacement,
            band_height,
            seed,
            rate,
            ..
        } => {
            changed |= ui
                .add(egui::Slider::new(displacement, 0.0..=128.).text("Displacement"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(band_height, 1..=128).text("Band height"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(rate, 1..=60).text("Updates/sec"))
                .changed();
            changed |= ui.add(egui::DragValue::new(seed).prefix("Seed ")).changed();
        }
        ComicStyle::Halftone {
            spacing,
            angle,
            ink,
            ..
        } => {
            changed |= ui
                .add(egui::Slider::new(spacing, 2.0..=64.).text("Dot spacing"))
                .changed();
            changed |= ui
                .add(
                    egui::Slider::new(angle, -std::f32::consts::PI..=std::f32::consts::PI)
                        .text("Angle (radians)"),
                )
                .changed();
            changed |= ui.color_edit_button_srgba_unmultiplied(ink).changed();
        }
    }
    let (strength, keys) = match style {
        ComicStyle::RgbSplit { strength, keys, .. }
        | ComicStyle::Glitch { strength, keys, .. }
        | ComicStyle::Halftone { strength, keys, .. } => (strength, keys),
    };
    changed |= ui
        .add(egui::Slider::new(strength, 0.0..=1.).text("Strength"))
        .changed();
    if ui.button("Key strength at playhead").clicked() {
        if let Some(k) = keys.iter_mut().find(|k| k.frame == frame) {
            k.strength = *strength;
        } else {
            keys.push(ComicKey {
                frame,
                strength: *strength,
            });
            keys.sort_by_key(|k| k.frame);
        }
        changed = true;
    }
    if !keys.is_empty() {
        ui.small(format!("{} strength keys (linear)", keys.len()));
        if ui.button("Clear strength keys").clicked() {
            keys.clear();
            changed = true;
        }
    }
    changed
}

fn cinematic_controls(ui: &mut egui::Ui, stage: &mut CinematicStage, frame: u32) -> bool {
    let mut changed = false;
    match &mut stage.effect {
        CinematicEffect::Bloom {
            threshold,
            knee,
            radius,
            intensity,
        } => {
            changed |= ui
                .add(egui::Slider::new(threshold, 0.0..=1.).text("Linear threshold"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(knee, 0.0..=1.).text("Soft knee"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(radius, 0..=64).text("Bloom radius"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(intensity, 0.0..=8.).text("Intensity"))
                .changed();
        }
        CinematicEffect::DropShadow {
            offset,
            radius,
            color,
            opacity,
        } => {
            changed |= ui
                .add(egui::Slider::new(&mut offset.x, -128.0..=128.).text("Shadow X"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(&mut offset.y, -128.0..=128.).text("Shadow Y"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(radius, 0..=64).text("Softness"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(opacity, 0.0..=1.).text("Shadow opacity"))
                .changed();
            changed |= ui.color_edit_button_srgba_unmultiplied(color).changed();
        }
        CinematicEffect::RimLight {
            offset,
            radius,
            color,
            intensity,
        } => {
            changed |= ui
                .add(egui::Slider::new(&mut offset.x, -32.0..=32.).text("Light X"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(&mut offset.y, -32.0..=32.).text("Light Y"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(radius, 0..=32).text("Light softness"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(intensity, 0.0..=8.).text("Light intensity"))
                .changed();
            changed |= ui.color_edit_button_srgba_unmultiplied(color).changed();
        }
        CinematicEffect::Grade {
            exposure,
            contrast,
            saturation,
            lift,
            gamma,
            gain,
        } => {
            changed |= ui
                .add(egui::Slider::new(exposure, -8.0..=8.).text("Exposure stops"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(contrast, 0.0..=3.).text("Contrast"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(saturation, 0.0..=4.).text("Saturation"))
                .changed();
            for (name, values, lo, hi) in [
                ("Lift", lift, -0.25, 0.25),
                ("Gamma", gamma, 0.1, 4.),
                ("Gain", gain, 0., 4.),
            ] {
                ui.collapsing(name, |ui| {
                    for (c, value) in values.iter_mut().enumerate() {
                        changed |= ui
                            .add(
                                egui::Slider::new(value, lo..=hi).text(["Red", "Green", "Blue"][c]),
                            )
                            .changed();
                    }
                });
            }
        }
    }
    changed |= ui
        .add(egui::Slider::new(&mut stage.mix, 0.0..=1.).text("Effect mix"))
        .changed();
    if ui.button("Key mix at playhead").clicked() {
        if let Some(k) = stage.keys.iter_mut().find(|k| k.frame == frame) {
            k.strength = stage.mix;
        } else {
            stage.keys.push(ComicKey {
                frame,
                strength: stage.mix,
            });
            stage.keys.sort_by_key(|k| k.frame);
        }
        changed = true;
    }
    if !stage.keys.is_empty() {
        ui.small(format!("{} linear mix keys", stage.keys.len()));
        if ui.button("Clear mix keys").clicked() {
            stage.keys.clear();
            changed = true;
        }
    }
    changed
}
