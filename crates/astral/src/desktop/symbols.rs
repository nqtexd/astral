use super::*;
impl Studio {
    pub(super) fn symbols_panel(&mut self, ui: &mut egui::Ui, p: &Project) {
        ui.collapsing("Live symbols and nested animation",|ui|{
   ui.small("Instances share a saved library source. Edit its definition through SetLibrary to update every instance; transforms and local timing stay independent. Nested children use the parent's local frames. Source audio is not instanced.");
   let entries=astral_core::studio::library_entries(p).unwrap_or_default();
   ui.menu_button("Place live symbol",|ui|{for e in &entries{if ui.button(&e.name).clicked(){let start=self.frame as u32;let s=SymbolInstance{id:Id::new_v4(),name:e.name.clone(),entry:e.id,parent:None,start,end:p.duration-1,offset:0.,speed:1.,looping:true,layer:GraphicLayer::Foreground,keys:vec![GraphicKey{frame:start,position:e.origin,scale:1.,rotation:0.,opacity:1.,easing:Easing::Smooth}]};let mut instances=p.symbols.clone();instances.push(s.clone());self.apply(vec![Command::SetSymbolInstances{instances}]);self.motion.symbol_edit=Some(s);ui.close();}}});
   for s in &p.symbols {if ui.selectable_label(self.motion.symbol_edit.as_ref().is_some_and(|v|v.id==s.id),&s.name).clicked(){self.motion.symbol_edit=Some(s.clone());}}
   let Some(mut s)=self.motion.symbol_edit.take() else{return};
   ui.text_edit_singleline(&mut s.name);
   egui::ComboBox::from_id_salt("symbol.source").selected_text(entries.iter().find(|e|e.id==s.entry).map_or("Missing source",|e|e.name.as_str())).show_ui(ui,|ui|{for e in &entries{ui.selectable_value(&mut s.entry,e.id,&e.name);}});
   egui::ComboBox::from_id_salt("symbol.parent").selected_text(s.parent.and_then(|id|p.symbols.iter().find(|s|s.id==id)).map_or("Scene root",|s|s.name.as_str())).show_ui(ui,|ui|{ui.selectable_value(&mut s.parent,None,"Scene root");for parent in p.symbols.iter().filter(|v|v.id!=s.id){if ui.selectable_value(&mut s.parent,Some(parent.id),&parent.name).changed(){s.start=0;s.end=entries.iter().find(|e|e.id==parent.entry).map_or(0,|e|e.duration-1);s.keys=vec![GraphicKey{frame:0,position:entries.iter().find(|e|e.id==s.entry).map_or(Point::default(),|e|e.origin),scale:1.,rotation:0.,opacity:1.,easing:Easing::Smooth}];}}});
   ui.horizontal_wrapped(|ui|{ui.add(egui::DragValue::new(&mut s.start).prefix("Start (0-based) "));ui.add(egui::DragValue::new(&mut s.end).prefix("End "));ui.add(egui::DragValue::new(&mut s.offset).range(0.0..=1_000_000.).prefix("Source offset "));});ui.add(egui::Slider::new(&mut s.speed,0.01..=16.).text("Speed"));ui.checkbox(&mut s.looping,"Loop source");
   if ui.button("Transform key at playhead (scene roots)").clicked() && s.parent.is_none(){let frame=self.frame as u32;if !s.keys.iter().any(|k|k.frame==frame){let mut key=s.at(frame as f64);key.frame=frame;s.keys.push(key);s.keys.sort_by_key(|k|k.frame);}}
   for k in &mut s.keys{ui.push_id(k.frame,|ui|{ui.horizontal_wrapped(|ui|{ui.label(format!("Key {}",k.frame+1));ui.add(egui::DragValue::new(&mut k.position.x).prefix("X "));ui.add(egui::DragValue::new(&mut k.position.y).prefix("Y "));ui.add(egui::DragValue::new(&mut k.scale).range(0.001..=100.).speed(0.01).prefix("Scale "));ui.add(egui::DragValue::new(&mut k.rotation).speed(0.02).suffix(" rad"));ui.add(egui::Slider::new(&mut k.opacity,0.0..=1.).text("Alpha"));});});}
   ui.horizontal_wrapped(|ui|{if ui.button("Apply instance").clicked(){let mut instances=p.symbols.clone();instances.retain(|v|v.id!=s.id);instances.push(s.clone());self.apply(vec![Command::SetSymbolInstances{instances}]);}
if ui.button("Remove instance and children").clicked(){let mut removed=vec![s.id];loop{let n=removed.len();for c in &p.symbols{if c.parent.is_some_and(|id|removed.contains(&id))&&!removed.contains(&c.id){removed.push(c.id);}}if n==removed.len(){break;}}self.apply(vec![Command::SetSymbolInstances{instances:p.symbols.iter().filter(|v|!removed.contains(&v.id)).cloned().collect()}]);}});
   self.motion.symbol_edit=Some(s);
  });
    }
}
