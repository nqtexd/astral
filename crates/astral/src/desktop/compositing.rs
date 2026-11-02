use super::*;
impl Studio {
    pub(super) fn compositing_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        ui.collapsing(i18n::tr("Compositing"),|ui|{
   if ui.add_enabled(!p.rasters.is_empty(),egui::Button::new(i18n::tr("Compose artwork layers"))).clicked(){
    let base=Id::new_v4();let mut nodes=vec![CompositeNode{id:base,name:"Transparent".into(),op:CompositeOp::Solid{color:[0,0,0,0]}}];let mut output=base;
    for r in &p.rasters {let source=Id::new_v4();nodes.push(CompositeNode{id:source,name:r.name.clone(),op:CompositeOp::Raster{layer:r.id}});let id=Id::new_v4();nodes.push(CompositeNode{id,name:format!("Blend {}",r.name),op:CompositeOp::Blend{below:output,above:source,mode:BlendMode::Normal,opacity:1.}});output=id;}
    let mut compositions=p.compositions.clone();compositions.push(Composition{id:Id::new_v4(),name:"Composition".into(),enabled:true,layer:GraphicLayer::Background,output,nodes});self.apply(vec![Command::SetCompositions{compositions}]);
   }
   for graph in &p.compositions {ui.push_id(graph.id,|ui|{ui.collapsing(&graph.name,|ui|{
    let mut next=graph.clone();let mut changed=ui.checkbox(&mut next.enabled,"Enabled").changed();changed|=ui.text_edit_singleline(&mut next.name).changed();
    let mut foreground=next.layer==GraphicLayer::Foreground;if ui.checkbox(&mut foreground,"Above scene").changed(){next.layer=if foreground{GraphicLayer::Foreground}else{GraphicLayer::Background};changed=true;}
    let names:Vec<_>=next.nodes.iter().map(|n|(n.id,n.name.clone())).collect();
    for n in &mut next.nodes {egui::Frame::group(ui.style()).show(ui,|ui|{ui.push_id(n.id,|ui|{
     ui.horizontal(|ui|{let width=(ui.available_width()-34.).max(20.);ui.add_sized([width,24.],egui::Label::new(RichText::new(&n.name).strong()).truncate()).on_hover_text(&n.name);if theme::icon_button(ui,"Set composition output",theme::Icon::Check,next.output==n.id).clicked(){next.output=n.id;changed=true;}});
     let choose=|ui:&mut egui::Ui,label:&str,value:&mut Id|->bool{let mut edit=false;egui::ComboBox::from_id_salt(label).selected_text(format!("{label}: {}",names.iter().find(|(id,_)|id==value).map_or("Missing",|(_,name)|name.as_str()))).width(ui.available_width().min(180.)).show_ui(ui,|ui|{for(id,name)in &names{if *id!=n.id{edit|=ui.selectable_value(value,*id,name).changed();}}});edit};
     match &mut n.op {
      CompositeOp::Blend{below,above,mode,opacity}=>{changed|=choose(ui,"Below",below);changed|=choose(ui,"Above",above);egui::ComboBox::from_id_salt("blend").selected_text(format!("{mode:?}")).show_ui(ui,|ui|{for value in BlendMode::ALL{changed|=ui.selectable_value(mode,value,format!("{value:?}")).changed();}});changed|=ui.add(egui::Slider::new(opacity,0.0..=1.).text("Opacity")).changed();},
      CompositeOp::Mask{source,mask,invert,luminance}=>{changed|=choose(ui,"Source",source);changed|=choose(ui,"Mask",mask);changed|=ui.checkbox(invert,"Invert").changed();changed|=ui.checkbox(luminance,"Luminance").changed();},
      CompositeOp::Blur{source,radius}=>{changed|=choose(ui,"Source",source);changed|=ui.add(egui::Slider::new(radius,0..=32).text(i18n::tr("Blur"))).changed();},
      CompositeOp::Color{source,exposure,saturation}=>{changed|=choose(ui,"Source",source);changed|=ui.add(egui::Slider::new(exposure,-8.0..=8.).text(i18n::tr("Exposure"))).changed();changed|=ui.add(egui::Slider::new(saturation,0.0..=4.).text(i18n::tr("Saturation"))).changed();},
      CompositeOp::Solid{color}=>{changed|=ui.color_edit_button_srgba_unmultiplied(color).changed();},
      CompositeOp::Gradient{start,end,from,to,radial}=>{changed|=ui.color_edit_button_srgba_unmultiplied(from).changed();changed|=ui.color_edit_button_srgba_unmultiplied(to).changed();changed|=ui.checkbox(radial,"Radial").changed();for(label,point)in[("Start",start),("End",end)]{ui.label(label);changed|=ui.add(egui::Slider::new(&mut point.x,0.0..=1.).text("X")).changed();changed|=ui.add(egui::Slider::new(&mut point.y,0.0..=1.).text("Y")).changed();}},
      _=>{ui.small("Editable scene input");}
     }
    });});}
    ui.horizontal_wrapped(|ui|{for label in ["Blur","Color","Gradient fill"]{if ui.button(label).clicked(){let source=next.output;let id=Id::new_v4();let op=match label{"Blur"=>CompositeOp::Blur{source,radius:4},"Color"=>CompositeOp::Color{source,exposure:0.,saturation:1.},_=>{let gradient=Id::new_v4();next.nodes.push(CompositeNode{id:gradient,name:"Gradient".into(),op:CompositeOp::Gradient{start:Point::new(0.,0.),end:Point::new(1.,1.),from:[66,140,230,255],to:[172,93,228,255],radial:false}});CompositeOp::Mask{source:gradient,mask:source,invert:false,luminance:false}}};next.nodes.push(CompositeNode{id,name:label.into(),op});next.output=id;changed=true;}}});
    if changed{let mut compositions=p.compositions.clone();*compositions.iter_mut().find(|g|g.id==graph.id).unwrap()=next;self.apply(vec![Command::SetCompositions{compositions}]);}
    if theme::button(ui,"Remove composition",theme::Icon::Delete,false).clicked(){self.apply(vec![Command::SetCompositions{compositions:p.compositions.iter().filter(|g|g.id!=graph.id).cloned().collect()}]);}
   });});}
   ui.small("Masks/blends work in premultiplied sRGB. Nodes are shared by preview and export; large graphs have a bounded memory budget.");
  });
    }
}
