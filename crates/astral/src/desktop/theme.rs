use super::*;

pub const BACKGROUND: Color32 = Color32::from_rgb(19, 20, 23);
pub const CHROME: Color32 = Color32::from_rgb(27, 28, 32);
pub const SURFACE: Color32 = Color32::from_rgb(34, 35, 40);
pub const CONTROL: Color32 = Color32::from_rgb(45, 46, 53);
pub const HOVER: Color32 = Color32::from_rgb(58, 61, 71);
pub const BORDER: Color32 = Color32::from_rgb(57, 59, 67);
pub const TEXT: Color32 = Color32::from_rgb(240, 241, 245);
pub const MUTED: Color32 = Color32::from_rgb(165, 170, 184);
pub const ON_ACCENT: Color32 = Color32::from_rgb(12, 25, 44);
pub const ACCENT: Color32 = Color32::from_rgb(104, 157, 245);
pub const SELECTED: Color32 = Color32::from_rgb(48, 74, 113);

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Astral Sans".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../../vendor/text/NotoSans-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "Astral Bold".into(),
        egui::FontData::from_static(include_bytes!("../../../../vendor/text/NotoSans-Bold.ttf"))
            .into(),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "Astral Sans".into());
    fonts.families.insert(
        egui::FontFamily::Name("Astral Heading".into()),
        vec!["Astral Bold".into()],
    );
    ctx.set_fonts(fonts);
    let mut style = egui::Style {
        visuals: egui::Visuals::dark(),
        ..Default::default()
    };
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.));
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(20., egui::FontFamily::Name("Astral Heading".into())),
    );
    style.visuals.panel_fill = CHROME;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = BACKGROUND;
    style.visuals.faint_bg_color = CONTROL;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = SELECTED;
    style.visuals.selection.stroke = Stroke::new(1., Color32::WHITE);
    style.visuals.window_corner_radius = egui::CornerRadius::same(14);
    style.visuals.window_stroke = Stroke::new(1., BORDER);
    style.visuals.menu_corner_radius = egui::CornerRadius::same(9);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(9);
        widget.fg_stroke = Stroke::new(1.4, TEXT);
    }
    style.visuals.widgets.inactive.bg_fill = CONTROL;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_fill = HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.active.bg_fill = SELECTED;
    style.spacing.item_spacing = Vec2::new(6., 6.);
    style.spacing.button_padding = Vec2::new(10., 6.);
    style.spacing.interact_size = Vec2::new(32., 30.);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., BORDER);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1., MUTED);
    style.visuals.text_cursor.stroke = Stroke::new(2., ACCENT);
    style.visuals.slider_trailing_fill = true;
    style.spacing.window_margin = egui::Margin::same(16);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.slider_width = 116.;
    style.spacing.text_edit_width = 160.;
    style.spacing.combo_width = 140.;
    style.spacing.scroll.bar_width = 5.;
    ctx.set_global_style(style);
}

/// Original, resolution-independent interface glyphs. No platform icon font is required.
#[derive(Clone, Copy)]
pub enum Icon {
    Plus,
    Folder,
    Save,
    Export,
    Undo,
    Redo,
    Play,
    Pause,
    Start,
    Pencil,
    Eraser,
    Move,
    Hand,
    Settings,
    Search,
    More,
    Close,
    Check,
    Layers,
    Eye,
    EyeOff,
    Lock,
    Unlock,
    Up,
    Down,
    Previous,
    Next,
    Blank,
    Duplicate,
    Copy,
    Paste,
    Clear,
    Delete,
    Hold,
    Timeline,
    Onion,
    Fit,
    Sliders,
    Image,
    Character,
    Assist,
    Favorite,
    Text,
    Audio,
}

pub struct IconButton<'a> {
    label: &'a str,
    icon: Icon,
    selected: bool,
    compact: bool,
}
impl<'a> IconButton<'a> {
    pub fn new(label: &'a str, icon: Icon) -> Self {
        Self {
            label,
            icon,
            selected: false,
            compact: false,
        }
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}
impl egui::Widget for IconButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let translated = super::i18n::tr(self.label);
        let label = translated.as_str();
        let size = if self.compact { 26. } else { 32. };
        let response = ui.add(
            egui::Button::new("")
                .min_size(Vec2::splat(size))
                .frame_when_inactive(self.selected)
                .selected(self.selected),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), self.selected, label)
        });
        let color = ui.style().interact(&response).fg_stroke.color;
        paint_icon(ui.painter(), self.icon, response.rect.center(), color);
        response.on_hover_text(label)
    }
}
pub fn icon_button(ui: &mut egui::Ui, label: &str, icon: Icon, selected: bool) -> egui::Response {
    ui.add(IconButton::new(label, icon).selected(selected))
}
pub fn icon_menu(
    ui: &mut egui::Ui,
    label: &str,
    icon: Icon,
    contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let response = icon_button(ui, label, icon, false);
    egui::Popup::menu(&response).show(contents);
    response
}
pub fn button(ui: &mut egui::Ui, label: &str, icon: Icon, primary: bool) -> egui::Response {
    let translated = super::i18n::tr(label);
    let label = translated.as_str();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let width = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), TEXT)
        .size()
        .x
        + 46.;
    let mut button = egui::Button::new("").min_size(Vec2::new(width, 32.));
    if primary {
        button = button.fill(ui.visuals().hyperlink_color);
    }
    let response = ui.add(button);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let color = if primary {
        accent_foreground(ui.visuals().hyperlink_color)
    } else {
        ui.style().interact(&response).fg_stroke.color
    };
    paint_icon(
        ui.painter(),
        icon,
        Pos2::new(response.rect.left() + 17., response.rect.center().y),
        color,
    );
    ui.painter().text(
        Pos2::new(response.rect.left() + 32., response.rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        color,
    );
    response
}
pub fn section(ui: &mut egui::Ui, label: &str) {
    let translated = super::i18n::tr(label);
    let label = translated.as_str();
    ui.add_space(8.);
    ui.label(RichText::new(label).size(11.).strong().color(MUTED));
    ui.add_space(2.);
}

/// Keep slider captions and numeric editors inside narrow inspectors.
pub fn slider(ui: &mut egui::Ui, label: &str, slider: egui::Slider<'_>) -> egui::Response {
    let translated = super::i18n::tr(label);
    let label = translated.as_str();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text_width = ui
        .painter()
        .layout_no_wrap(label.into(), font, TEXT)
        .size()
        .x;
    if ui.available_width() < text_width + 152. {
        ui.vertical(|ui| {
            let caption = ui.label(label);
            ui.spacing_mut().slider_width = (ui.available_width() - 64.).clamp(32., 160.);
            ui.add(slider).labelled_by(caption.id)
        })
        .inner
    } else {
        ui.scope(|ui| {
            ui.spacing_mut().slider_width =
                (ui.available_width() - text_width - 64.).clamp(32., 116.);
            ui.add(slider.text(label))
        })
        .inner
    }
}
/// Preserve the chosen scale while keeping the editor usable in a small window.
pub fn effective_scale(viewport: Vec2, requested: f32, fit: bool) -> f32 {
    let requested = if requested.is_finite() {
        requested.clamp(0.75, 2.)
    } else {
        1.
    };
    if fit {
        requested.min((viewport.x / 800.).min(viewport.y / 520.).clamp(0.75, 2.))
    } else {
        requested
    }
}

/// Fit a panel to the current logical viewport, including high UI zoom.
pub fn dialog_size(ctx: &egui::Context, preferred: Vec2) -> Vec2 {
    let available = ctx.content_rect().size() - Vec2::new(40., 88.);
    Vec2::new(
        preferred.x.min(available.x.max(180.)),
        preferred.y.min(available.y.max(140.)),
    )
}
pub fn paint_icon(p: &egui::Painter, icon: Icon, center: Pos2, color: Color32) {
    let at = |x: f32, y: f32| center + Vec2::new(x - 8., y - 8.);
    let stroke = Stroke::new(1.45, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)], closed: bool| {
        let points = points.iter().map(|&(x, y)| at(x, y)).collect();
        p.add(if closed {
            egui::Shape::closed_line(points, stroke)
        } else {
            egui::Shape::line(points, stroke)
        });
    };
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            Rect::from_min_size(at(x, y), Vec2::new(w, h)),
            2.,
            stroke,
            egui::StrokeKind::Inside,
        );
    };
    let circle = |x: f32, y: f32, r: f32| {
        p.circle_stroke(at(x, y), r, stroke);
    };
    match icon {
        Icon::Plus => {
            line((8., 2.), (8., 14.));
            line((2., 8.), (14., 8.));
        }
        Icon::Folder => path(
            &[
                (1., 4.),
                (6., 4.),
                (8., 6.),
                (15., 6.),
                (13., 14.),
                (1., 14.),
            ],
            true,
        ),
        Icon::Save => {
            rect(2., 2., 12., 12.);
            line((5., 2.), (5., 6.));
            line((5., 6.), (11., 6.));
            line((5., 11.), (11., 11.));
        }
        Icon::Export => {
            path(&[(2., 9.), (2., 14.), (14., 14.), (14., 9.)], false);
            line((8., 11.), (8., 1.));
            path(&[(4., 5.), (8., 1.), (12., 5.)], false);
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![at(4., 2.), at(14., 8.), at(4., 14.)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            p.rect_filled(Rect::from_min_max(at(3., 2.), at(6., 14.)), 1., color);
            p.rect_filled(Rect::from_min_max(at(10., 2.), at(13., 14.)), 1., color);
        }
        Icon::Start => {
            line((2., 2.), (2., 14.));
            path(&[(13., 2.), (5., 8.), (13., 14.)], true);
        }
        Icon::Undo | Icon::Redo => {
            let flip = matches!(icon, Icon::Redo);
            let q = |x, y| if flip { at(16. - x, y) } else { at(x, y) };
            p.add(egui::Shape::line(
                vec![q(2., 5.), q(9., 5.), q(14., 9.), q(13., 13.)],
                stroke,
            ));
            p.add(egui::Shape::line(
                vec![q(6., 1.), q(2., 5.), q(6., 9.)],
                stroke,
            ));
        }
        Icon::Pencil => {
            path(
                &[
                    (2., 11.),
                    (10., 3.),
                    (14., 7.),
                    (6., 15.),
                    (1., 15.),
                    (2., 11.),
                ],
                false,
            );
            line((8., 5.), (12., 9.));
        }
        Icon::Eraser => {
            path(&[(2., 9.), (9., 2.), (15., 8.), (8., 15.), (5., 15.)], true);
            line((6., 5.), (12., 11.));
        }
        Icon::Move => {
            line((8., 1.), (8., 15.));
            line((1., 8.), (15., 8.));
            for points in [
                &[(5., 4.), (8., 1.), (11., 4.)][..],
                &[(5., 12.), (8., 15.), (11., 12.)],
                &[(4., 5.), (1., 8.), (4., 11.)],
                &[(12., 5.), (15., 8.), (12., 11.)],
            ] {
                path(points, false);
            }
        }
        Icon::Hand => path(
            &[
                (3., 9.),
                (2., 6.),
                (4., 5.),
                (6., 8.),
                (6., 2.),
                (8., 1.),
                (9., 7.),
                (10., 2.),
                (12., 3.),
                (12., 8.),
                (14., 5.),
                (15., 7.),
                (13., 13.),
                (10., 15.),
                (6., 14.),
                (3., 9.),
            ],
            false,
        ),
        Icon::Settings => {
            circle(8., 8., 4.5);
            circle(8., 8., 1.5);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                line(
                    (8. + a.cos() * 5., 8. + a.sin() * 5.),
                    (8. + a.cos() * 7.5, 8. + a.sin() * 7.5),
                );
            }
        }
        Icon::Search => {
            circle(6.5, 6.5, 4.5);
            line((10., 10.), (15., 15.));
        }
        Icon::More => {
            for x in [3., 8., 13.] {
                p.circle_filled(at(x, 8.), 1.2, color);
            }
        }
        Icon::Close => {
            line((4., 4.), (12., 12.));
            line((12., 4.), (4., 12.));
        }
        Icon::Check => path(&[(2., 8.), (6., 12.), (14., 3.)], false),
        Icon::Layers => {
            path(&[(1., 5.), (8., 1.), (15., 5.), (8., 9.)], true);
            path(&[(1., 9.), (8., 13.), (15., 9.)], false);
        }
        Icon::Eye | Icon::EyeOff => {
            path(
                &[
                    (1., 8.),
                    (4., 4.),
                    (8., 3.),
                    (12., 4.),
                    (15., 8.),
                    (12., 12.),
                    (8., 13.),
                    (4., 12.),
                ],
                true,
            );
            circle(8., 8., 2.);
            if matches!(icon, Icon::EyeOff) {
                line((2., 2.), (14., 14.));
            }
        }
        Icon::Lock | Icon::Unlock => {
            rect(3., 7., 10., 8.);
            if matches!(icon, Icon::Lock) {
                path(&[(5., 7.), (5., 3.), (8., 1.), (11., 3.), (11., 7.)], false);
            } else {
                path(&[(5., 7.), (5., 3.), (8., 1.), (12., 3.)], false);
            }
            line((8., 10.), (8., 12.));
        }
        Icon::Up => path(&[(3., 10.), (8., 5.), (13., 10.)], false),
        Icon::Down => path(&[(3., 6.), (8., 11.), (13., 6.)], false),
        Icon::Previous => {
            path(&[(11., 3.), (5., 8.), (11., 13.)], false);
            line((2., 3.), (2., 13.));
        }
        Icon::Next => {
            path(&[(5., 3.), (11., 8.), (5., 13.)], false);
            line((14., 3.), (14., 13.));
        }
        Icon::Blank => {
            rect(3., 1., 10., 14.);
            line((5., 8.), (11., 8.));
            line((8., 5.), (8., 11.));
        }
        Icon::Duplicate | Icon::Copy => {
            rect(1., 1., 10., 11.);
            rect(5., 5., 10., 10.);
            if matches!(icon, Icon::Duplicate) {
                line((7., 10.), (13., 10.));
                line((10., 7.), (10., 13.));
            }
        }
        Icon::Paste => {
            rect(3., 3., 10., 12.);
            rect(5., 1., 6., 4.);
            line((6., 8.), (10., 8.));
            line((6., 11.), (10., 11.));
        }
        Icon::Clear => {
            rect(2., 2., 12., 12.);
            line((5., 5.), (11., 11.));
            line((11., 5.), (5., 11.));
        }
        Icon::Delete => {
            path(&[(4., 5.), (5., 15.), (11., 15.), (12., 5.)], false);
            line((2., 4.), (14., 4.));
            path(&[(6., 4.), (6., 1.), (10., 1.), (10., 4.)], false);
            line((7., 7.), (7., 12.));
            line((9., 7.), (9., 12.));
        }
        Icon::Hold => {
            line((2., 3.), (2., 13.));
            line((14., 3.), (14., 13.));
            line((2., 8.), (14., 8.));
            circle(8., 8., 2.);
        }
        Icon::Timeline => {
            rect(1., 2., 14., 12.);
            line((1., 6.), (15., 6.));
            line((5., 6.), (5., 14.));
            line((10., 6.), (10., 14.));
        }
        Icon::Onion => {
            circle(6., 8., 4.5);
            circle(10., 8., 4.5);
        }
        Icon::Fit => {
            for points in [
                &[(1., 6.), (1., 1.), (6., 1.)][..],
                &[(10., 1.), (15., 1.), (15., 6.)],
                &[(15., 10.), (15., 15.), (10., 15.)],
                &[(6., 15.), (1., 15.), (1., 10.)],
            ] {
                path(points, false);
            }
        }
        Icon::Sliders => {
            for (x, y) in [(3., 5.), (8., 11.), (13., 6.)] {
                line((x, 1.), (x, 15.));
                p.circle_filled(at(x, y), 2.2, SURFACE);
                circle(x, y, 2.2);
            }
        }
        Icon::Image => {
            rect(1., 2., 14., 12.);
            circle(5., 6., 1.5);
            path(
                &[(2., 13.), (7., 8.), (10., 11.), (12., 9.), (15., 12.)],
                false,
            );
        }
        Icon::Character => {
            circle(8., 3., 2.);
            path(&[(3., 9.), (8., 6.), (13., 9.)], false);
            line((8., 6.), (8., 11.));
            path(&[(4., 15.), (8., 11.), (12., 15.)], false);
        }
        Icon::Assist => {
            path(
                &[(2., 3.), (5., 1.), (8., 3.), (8., 6.), (5., 8.), (2., 6.)],
                true,
            );
            path(
                &[
                    (8., 10.),
                    (11., 8.),
                    (14., 10.),
                    (14., 13.),
                    (11., 15.),
                    (8., 13.),
                ],
                true,
            );
            line((5., 8.), (8., 10.));
        }
        Icon::Favorite => {
            let points: Vec<_> = (0..10)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::PI / 5. - std::f32::consts::FRAC_PI_2;
                    let r = if i % 2 == 0 { 7. } else { 3.2 };
                    at(8. + a.cos() * r, 8. + a.sin() * r)
                })
                .collect();
            p.add(egui::Shape::closed_line(points, stroke));
        }
        Icon::Text => {
            line((2., 2.), (14., 2.));
            line((8., 2.), (8., 14.));
            line((5., 14.), (11., 14.));
        }
        Icon::Audio => {
            for (x, y) in [(2., 5.), (5., 2.), (8., 6.), (11., 0.), (14., 4.)] {
                line((x, y + 1.), (x, 15. - y));
            }
        }
    }
}

pub(super) fn customize(ctx: &egui::Context, preferences: &super::Preferences) {
    ctx.global_style_mut(|style| {
        let accent = Color32::from_rgba_unmultiplied(
            preferences.accent[0],
            preferences.accent[1],
            preferences.accent[2],
            255,
        );
        style.visuals.hyperlink_color = accent;
        style.visuals.selection.bg_fill = accent.gamma_multiply(0.4);
        style.visuals.selection.stroke = Stroke::new(1., accent);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1., accent);
        style.visuals.widgets.active.bg_fill = accent.gamma_multiply(0.45);
        style.visuals.text_cursor.stroke = Stroke::new(2., accent);
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(preferences.corners.min(18));
        }
        style.spacing.item_spacing =
            Vec2::splat(if preferences.compact_controls { 4. } else { 6. });
        style.spacing.button_padding = if preferences.compact_controls {
            Vec2::new(8., 4.)
        } else {
            Vec2::new(10., 6.)
        };
        style.spacing.interact_size = if preferences.compact_controls {
            Vec2::new(28., 26.)
        } else {
            Vec2::new(32., 30.)
        };
        style.visuals.widgets.inactive.bg_stroke = if preferences.high_contrast {
            Stroke::new(1., Color32::from_rgb(140, 148, 165))
        } else {
            Stroke::new(1., BORDER)
        };
        style.visuals.window_stroke = Stroke::new(
            1.,
            if preferences.high_contrast {
                Color32::from_rgb(155, 165, 184)
            } else {
                BORDER
            },
        );
        style.visuals.weak_text_color = Some(if preferences.high_contrast {
            Color32::from_rgb(213, 220, 233)
        } else {
            MUTED
        });
    });
}

/// Choose readable text for user-selected accent colors.
pub fn accent_foreground(background: Color32) -> Color32 {
    let channel = |c: u8| {
        let s = c as f32 / 255.;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    let light = 0.2126 * channel(background.r())
        + 0.7152 * channel(background.g())
        + 0.0722 * channel(background.b());
    if light > 0.179 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(color: Color32) -> f32 {
        let linear = |c: u8| {
            let c = c as f32 / 255.;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
    #[test]
    fn normal_text_and_primary_actions_have_readable_contrast() {
        for (foreground, background) in [
            (TEXT, SURFACE),
            (MUTED, SURFACE),
            (TEXT, SELECTED),
            (ON_ACCENT, ACCENT),
        ] {
            let a = luminance(foreground);
            let b = luminance(background);
            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
        }
    }
}
