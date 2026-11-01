//! Bundled OFL Noto Sans, HarfRust shaping and portable vector glyph outlines.
use astral_core::{Point, TextAlign, TextFont, TextFormatting};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
use unicode_segmentation::UnicodeSegmentation;
const FONT: &[u8] = include_bytes!("../../../vendor/text/NotoSans-Regular.ttf");
type Contours = Vec<Vec<Point>>;
struct Glyph {
    reveal: usize,
    contours: Contours,
}
struct Layout {
    glyphs: Vec<Glyph>,
    truncated: bool,
}
#[derive(Default)]
struct TextCache {
    entries: BTreeMap<String, Arc<Layout>>,
    bytes: usize,
}
static CACHE: OnceLock<Mutex<TextCache>> = OnceLock::new();

pub fn outlines(text: &str, size: f32, width: f32, height: f32, reveal: usize) -> Contours {
    outlines_styled(
        text,
        size,
        width,
        height,
        reveal,
        &TextFormatting::default(),
    )
}
pub fn outlines_styled(
    text: &str,
    size: f32,
    width: f32,
    height: f32,
    reveal: usize,
    format: &TextFormatting,
) -> Contours {
    let key = format!(
        "{:?}:{:?}:{}:{}:{}:{}:{text}",
        format.font,
        format.align,
        format.line_height.to_bits(),
        size.to_bits(),
        width.to_bits(),
        height.to_bits()
    );
    let cache = CACHE.get_or_init(Default::default);
    let cached = {
        cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entries
            .get(&key)
            .cloned()
    };
    let layout = cached.unwrap_or_else(|| {
        let layout = Arc::new(layout(text, size, width, height, format));
        let mut cache = cache.lock().unwrap_or_else(|p| p.into_inner());
        let bytes = layout
            .glyphs
            .iter()
            .flat_map(|g| &g.contours)
            .map(|c| c.len() * std::mem::size_of::<Point>())
            .sum::<usize>()
            + key.len();
        if cache.entries.len() >= 128 || cache.bytes + bytes > 32 * 1024 * 1024 {
            cache.entries.clear();
            cache.bytes = 0;
        }
        if bytes <= 32 * 1024 * 1024 {
            cache.bytes += bytes;
            cache.entries.insert(key, layout.clone());
        }
        layout
    });
    layout
        .glyphs
        .iter()
        .filter(|g| g.reveal < reveal)
        .flat_map(|g| g.contours.clone())
        .collect()
}
fn layout(text: &str, size: f32, width: f32, height: f32, format: &TextFormatting) -> Layout {
    let bytes: &[u8] = match format.font {
        TextFont::Regular => FONT,
        TextFont::Bold => include_bytes!("../../../vendor/text/NotoSans-Bold.ttf"),
        TextFont::Italic => include_bytes!("../../../vendor/text/NotoSans-Italic.ttf"),
        TextFont::BoldItalic => include_bytes!("../../../vendor/text/NotoSans-BoldItalic.ttf"),
    };
    let face = ttf_parser::Face::parse(bytes, 0).expect("Bundled Noto Sans");
    let font = harfrust::FontRef::new(bytes).expect("Bundled Noto Sans");
    let data = harfrust::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let shape = |s: &str| {
        let mut buffer = harfrust::UnicodeBuffer::new();
        buffer.push_str(s);
        buffer.guess_segment_properties();
        shaper.shape(buffer, harfrust::ShapeOptions::new())
    };
    let scale = size / face.units_per_em() as f32;
    let measure = |s: &str| {
        shape(s)
            .glyph_positions()
            .iter()
            .map(|p| p.x_advance as f32 * scale)
            .sum::<f32>()
    };
    let mut lines = vec![];
    for paragraph in text.split('\n') {
        let mut current = String::new();
        for word in paragraph.split_inclusive(char::is_whitespace) {
            let candidate = format!("{current}{word}");
            if measure(&candidate) <= width {
                current = candidate;
                continue;
            }
            if !current.trim().is_empty() {
                lines.push(current.trim_end().to_string());
                current.clear();
            }
            for grapheme in word.trim_start().graphemes(true) {
                let next = format!("{current}{grapheme}");
                if !current.is_empty() && measure(&next) > width {
                    lines.push(std::mem::take(&mut current));
                }
                current.push_str(grapheme);
            }
        }
        lines.push(current.trim_end().to_string());
    }
    let line_height = size * format.line_height;
    let max_lines = (height / line_height).floor().max(0.) as usize;
    let truncated = lines.len() > max_lines || lines.iter().any(|line| measure(line) > width + 0.1);
    if lines.len() > max_lines && max_lines > 0 {
        lines.truncate(max_lines);
        let last = lines.last_mut().unwrap();
        while !last.is_empty() && measure(&format!("{last}…")) > width {
            let end = last.grapheme_indices(true).next_back().map_or(0, |v| v.0);
            last.truncate(end);
        }
        last.push('…');
    }
    let mut glyphs = vec![];
    let mut prior = 0;
    for (row, line) in lines.iter().take(max_lines).enumerate() {
        let buffer = shape(line);
        let mut x = match format.align {
            TextAlign::Left => 0.,
            TextAlign::Center => (width - measure(line)).max(0.) / 2.,
            TextAlign::Right => (width - measure(line)).max(0.),
        };
        let baseline = face.ascender() as f32 * scale + row as f32 * line_height;
        for (info, pos) in buffer.glyph_infos().iter().zip(buffer.glyph_positions()) {
            let mut builder = Outline {
                contours: vec![],
                current: vec![],
                last: Point::default(),
                origin: Point::new(
                    x + pos.x_offset as f32 * scale,
                    baseline - pos.y_offset as f32 * scale,
                ),
                scale,
            };
            let _ = face.outline_glyph(ttf_parser::GlyphId(info.glyph_id as u16), &mut builder);
            builder.finish();
            let cluster = (info.cluster as usize).min(line.len());
            let index = prior + line.get(..cluster).unwrap_or("").graphemes(true).count();
            glyphs.push(Glyph {
                reveal: index,
                contours: builder.contours,
            });
            x += pos.x_advance as f32 * scale;
        }
        prior += line.graphemes(true).count() + 1;
    }
    Layout { glyphs, truncated }
}
struct Outline {
    contours: Contours,
    current: Vec<Point>,
    last: Point,
    origin: Point,
    scale: f32,
}
impl Outline {
    fn point(&self, x: f32, y: f32) -> Point {
        Point::new(
            self.origin.x + x * self.scale,
            self.origin.y - y * self.scale,
        )
    }
    fn finish(&mut self) {
        if self.current.len() > 2 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        self.last = self.point(x, y);
        self.current.push(self.last);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.last = self.point(x, y);
        self.current.push(self.last);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let a = self.last;
        let b = self.point(x1, y1);
        let c = self.point(x, y);
        for i in 1..=8 {
            let t = i as f32 / 8.;
            let u = 1. - t;
            self.current.push(Point::new(
                u * u * a.x + 2. * u * t * b.x + t * t * c.x,
                u * u * a.y + 2. * u * t * b.y + t * t * c.y,
            ));
        }
        self.last = c;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let a = self.last;
        let b = self.point(x1, y1);
        let c = self.point(x2, y2);
        let d = self.point(x, y);
        for i in 1..=12 {
            let t = i as f32 / 12.;
            let u = 1. - t;
            self.current.push(Point::new(
                u * u * u * a.x + 3. * u * u * t * b.x + 3. * u * t * t * c.x + t * t * t * d.x,
                u * u * u * a.y + 3. * u * u * t * b.y + 3. * u * t * t * c.y + t * t * t * d.y,
            ));
        }
        self.last = d;
    }
    fn close(&mut self) {
        self.finish();
    }
}

/// Uses exactly the same shaping and wrapping as the exported glyph outlines.
pub fn overflows(text: &str, size: f32, width: f32, height: f32, format: &TextFormatting) -> bool {
    width <= 0. || height <= 0. || layout(text, size, width, height, format).truncated
}
