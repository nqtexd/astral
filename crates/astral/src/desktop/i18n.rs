//! UI catalogs use English source labels as stable keys; document text is never translated.
use std::{cell::RefCell, collections::BTreeMap};
thread_local! {static ACTIVE:RefCell<(String,BTreeMap<String,String>)>=const {RefCell::new((String::new(),BTreeMap::new()))};}
pub(super) fn install(language: &str) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        if active.0 != language {
            let bytes = match language {
                "es" => include_str!("locales/es.json"),
                "fr" => include_str!("locales/fr.json"),
                _ => "{}",
            };
            *active = (
                language.into(),
                serde_json::from_str(bytes).unwrap_or_default(),
            );
        }
    });
}
pub(super) fn tr(text: &str) -> String {
    ACTIVE.with(|active| {
        active
            .borrow()
            .1
            .get(text)
            .cloned()
            .unwrap_or_else(|| text.into())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogs_share_keys_and_preserve_untranslated_project_text() {
        let es: BTreeMap<String, String> =
            serde_json::from_str(include_str!("locales/es.json")).unwrap();
        let fr: BTreeMap<String, String> =
            serde_json::from_str(include_str!("locales/fr.json")).unwrap();
        assert_eq!(es.keys().collect::<Vec<_>>(), fr.keys().collect::<Vec<_>>());
        install("es");
        assert_eq!(tr("Save"), "Guardar");
        assert_eq!(tr("My original story"), "My original story");
        install("en");
    }
}
