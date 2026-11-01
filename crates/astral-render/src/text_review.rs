use crate::*;
#[derive(Clone, Debug, serde::Serialize)]
pub struct TextIssue {
    pub dialogue: Id,
    pub kind: String,
    pub message: String,
}
fn fits(d: &Dialogue) -> bool {
    let pad = d.formatting.padding;
    let y = (pad - 2.).max(0.)
        + if d.speaker.is_empty() {
            0.
        } else {
            d.font_size * 1.2
        };
    !crate::text::overflows(
        &d.text,
        d.font_size,
        d.size.x - pad * 2.,
        d.size.y - y - (pad - 4.).max(0.),
        &d.formatting,
    ) && (d.speaker.is_empty()
        || !crate::text::overflows(
            &d.speaker,
            d.font_size * 0.75,
            d.size.x - pad * 2.,
            d.font_size * 1.1,
            &TextFormatting::default(),
        ))
}
pub fn review_text(p: &Project, start: u32, end: u32) -> Vec<TextIssue> {
    p.dialogues.iter().filter(|d|d.start<=end&&d.end>=start&&!fits(d)).map(|d|TextIssue{dialogue:d.id,kind:"text_overflow".into(),message:"The shaped text does not fit this dialogue box; the preview may show an ellipsis. Use fit and place before exporting.".into()}).collect()
}
/// A non-mutating proposal. Keep wording editable and preserve the preferred font where possible.
pub fn fit_and_place(p: &Project, id: Id) -> Result<(Dialogue, Vec<String>)> {
    let mut dialogue = p
        .dialogues
        .iter()
        .find(|d| d.id == id)
        .context("Dialogue missing")?
        .clone();
    let mut warnings = vec![];
    let max_width = (p.width as f32 - 48.).max(32.);
    let max_height = (p.height as f32 * 0.45).max(32.);
    dialogue.size.x = dialogue.size.x.min(max_width);
    while !fits(&dialogue) && dialogue.size.y < max_height {
        dialogue.size.y = (dialogue.size.y + dialogue.font_size * dialogue.formatting.line_height)
            .min(max_height);
    }
    while !fits(&dialogue) && dialogue.font_size > 8. {
        dialogue.font_size = (dialogue.font_size - 1.).max(8.);
    }
    if !fits(&dialogue) {
        warnings.push(
            "Text still exceeds the readable box budget. Split the dialogue into multiple cues."
                .into(),
        );
    }
    let mut candidate = p.clone();
    *candidate.dialogues.iter_mut().find(|d| d.id == id).unwrap() = dialogue;
    let (placed, notes) = composition::place_dialogue(&candidate, id)?;
    warnings.extend(notes);
    Ok((placed, warnings))
}
