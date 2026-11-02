use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Binding {
    pub key: egui::Key,
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}
pub(super) const ACTIONS: [(&str, &str, egui::Key, bool, bool); 18] = [
    ("new", "New project", egui::Key::N, true, false),
    ("open", "Open project", egui::Key::O, true, false),
    ("save", "Save", egui::Key::S, true, false),
    ("save_as", "Save as", egui::Key::S, true, true),
    ("close", "Close project", egui::Key::W, true, false),
    ("undo", "Undo", egui::Key::Z, true, false),
    ("redo", "Redo", egui::Key::Z, true, true),
    ("search", "Command search", egui::Key::P, true, false),
    ("play", "Play / pause", egui::Key::Space, false, false),
    (
        "previous",
        "Previous frame",
        egui::Key::ArrowLeft,
        false,
        false,
    ),
    ("next", "Next frame", egui::Key::ArrowRight, false, false),
    ("copy", "Copy", egui::Key::C, true, false),
    ("paste", "Paste", egui::Key::V, true, false),
    ("duplicate", "Duplicate", egui::Key::D, true, false),
    ("pencil", "Pencil", egui::Key::B, false, false),
    ("eraser", "Eraser", egui::Key::E, false, false),
    ("move", "Move drawing", egui::Key::G, false, false),
    ("hand", "Pan canvas", egui::Key::H, false, false),
];
pub(super) fn binding(map: &BTreeMap<String, Binding>, id: &str) -> Binding {
    map.get(id).copied().unwrap_or_else(|| {
        let a = ACTIONS
            .iter()
            .find(|a| a.0 == id)
            .expect("Known shortcut action");
        Binding {
            key: a.2,
            command: a.3,
            shift: a.4,
            alt: false,
        }
    })
}
pub(super) fn pressed(input: &egui::InputState, map: &BTreeMap<String, Binding>, id: &str) -> bool {
    let b = binding(map, id);
    input.key_pressed(b.key)
        && input.modifiers.command == b.command
        && input.modifiers.shift == b.shift
        && input.modifiers.alt == b.alt
}
pub(super) fn label(b: Binding) -> String {
    format!(
        "{}{}{}{}",
        if b.command {
            if cfg!(target_os = "macos") {
                "Cmd+"
            } else {
                "Ctrl+"
            }
        } else {
            ""
        },
        if b.shift { "Shift+" } else { "" },
        if b.alt { "Alt+" } else { "" },
        b.key.name()
    )
}
impl Studio {
    pub(super) fn shortcut_label(&self, id: &str, name: &str) -> String {
        format!(
            "{} · {}",
            i18n::tr(name),
            label(binding(&self.preferences.shortcuts, id))
        )
    }
    pub(super) fn shortcut_editor(&mut self, ui: &mut egui::Ui) {
        ui.small("Choose Change, then press a shortcut. Escape cancels. Conflicting bindings are rejected.");
        if let Some(action) = self.assist.capture_binding.clone() {
            ui.label(format!("Press a key for {action}…"));
            let event = ui.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, modifiers)) = event {
                if key == egui::Key::Escape {
                    self.assist.capture_binding = None;
                } else {
                    let proposed = Binding {
                        key,
                        command: modifiers.command,
                        shift: modifiers.shift,
                        alt: modifiers.alt,
                    };
                    if let Some(other) = ACTIONS.iter().find(|a| {
                        a.0 != action && binding(&self.preferences.shortcuts, a.0) == proposed
                    }) {
                        self.status = format!("Shortcut already used by {}", other.1);
                    } else {
                        self.preferences.shortcuts.insert(action, proposed);
                        self.assist.capture_binding = None;
                        self.status = "Shortcut updated".into();
                    }
                }
            }
        }
        egui::Grid::new("shortcut.editor")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                for (id, name, _, _, _) in ACTIONS {
                    ui.label(i18n::tr(name));
                    ui.monospace(label(binding(&self.preferences.shortcuts, id)));
                    if ui.button("Change").clicked() {
                        self.assist.capture_binding = Some(id.into());
                    }
                    ui.end_row();
                }
            });
        if ui.button("Reset shortcuts").clicked() {
            self.preferences.shortcuts.clear();
            self.assist.capture_binding = None;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_unique_and_overrides_roundtrip() {
        let mut map = BTreeMap::new();
        for (i, a) in ACTIONS.iter().enumerate() {
            for b in ACTIONS.iter().skip(i + 1) {
                assert_ne!(binding(&map, a.0), binding(&map, b.0));
            }
        }
        map.insert(
            "play".into(),
            Binding {
                key: egui::Key::F5,
                command: false,
                shift: false,
                alt: false,
            },
        );
        let json = serde_json::to_vec(&map).unwrap();
        let restored = serde_json::from_slice(&json).unwrap();
        assert_eq!(binding(&restored, "play").key, egui::Key::F5);
    }
}
