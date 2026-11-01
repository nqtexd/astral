use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
#[derive(Clone, Copy)]
pub enum Icon {
    Projects,
    Save,
    Undo,
    Redo,
    Settings,
    Draw,
    Erase,
    Pose,
    Pan,
    AddRig,
    Play,
    Pause,
    Previous,
    Next,
    Blank,
    Duplicate,
}
pub fn button(ui: &mut egui::Ui, label: &str, icon: Icon, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(48.), egui::Sense::click());
    #[cfg(test)]
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<(String, Rect, Rect)>>(egui::Id::unique(
            "touch.control-audit",
        ))
        .push((label.to_owned(), rect, ui.clip_rect()))
    });
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    let painter = ui.painter_at(rect);
    let fill = if selected {
        Color32::from_rgb(51, 78, 119)
    } else if response.hovered() {
        Color32::from_rgb(47, 52, 63)
    } else {
        Color32::from_rgb(35, 39, 48)
    };
    painter.rect_filled(rect, 10., fill);
    let box_ = rect.shrink(13.);
    let stroke = Stroke::new(1.7, Color32::from_rgb(221, 226, 235));
    let p = |x: f32, y: f32| {
        Pos2::new(
            box_.left() + box_.width() * x,
            box_.top() + box_.height() * y,
        )
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let path = |v: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            v.iter().map(|v| p(v.0, v.1)).collect(),
            stroke,
        ));
    };
    let square = |a: (f32, f32), b: (f32, f32)| {
        path(&[a, (b.0, a.1), b, (a.0, b.1), a]);
    };
    match icon {
        Icon::Projects => path(&[
            (0.05, 0.3),
            (0.05, 0.12),
            (0.4, 0.12),
            (0.52, 0.3),
            (0.95, 0.3),
            (0.95, 0.88),
            (0.05, 0.88),
            (0.05, 0.3),
        ]),
        Icon::Save => {
            path(&[
                (0.12, 0.08),
                (0.76, 0.08),
                (0.92, 0.24),
                (0.92, 0.92),
                (0.12, 0.92),
                (0.12, 0.08),
            ]);
            square((0.3, 0.08), (0.7, 0.38));
            square((0.29, 0.58), (0.75, 0.92));
        }
        Icon::Undo | Icon::Redo => {
            let mirror = matches!(icon, Icon::Redo);
            let map = |x| if mirror { 1. - x } else { x };
            path(&[(map(0.32), 0.12), (map(0.08), 0.35), (map(0.32), 0.58)]);
            path(&[
                (map(0.08), 0.35),
                (map(0.61), 0.35),
                (map(0.86), 0.48),
                (map(0.89), 0.72),
                (map(0.73), 0.88),
            ]);
        }
        Icon::Settings => {
            for (y, x) in [(0.2, 0.3), (0.5, 0.7), (0.8, 0.4)] {
                line((0., y), (1., y));
                painter.circle_filled(p(x, y), 3.5, fill);
                painter.circle_stroke(p(x, y), 3.5, stroke);
            }
        }
        Icon::Draw => {
            path(&[
                (0.12, 0.88),
                (0.2, 0.57),
                (0.74, 0.04),
                (0.96, 0.26),
                (0.42, 0.8),
                (0.12, 0.88),
            ]);
            line((0.64, 0.14), (0.86, 0.36));
            line((0.2, 0.57), (0.42, 0.8));
        }
        Icon::Erase => {
            path(&[
                (0.05, 0.62),
                (0.56, 0.08),
                (0.96, 0.45),
                (0.55, 0.88),
                (0.31, 0.88),
                (0.05, 0.62),
            ]);
            line((0.33, 0.33), (0.73, 0.7));
            line((0.31, 0.88), (1., 0.88));
        }
        Icon::Pose => {
            line((0.18, 0.78), (0.49, 0.47));
            line((0.49, 0.47), (0.84, 0.18));
            for (x, y) in [(0.18, 0.78), (0.49, 0.47), (0.84, 0.18)] {
                painter.circle_filled(p(x, y), 3.5, fill);
                painter.circle_stroke(p(x, y), 3.5, stroke);
            }
        }
        Icon::Pan => {
            line((0.5, 0.05), (0.5, 0.95));
            line((0.05, 0.5), (0.95, 0.5));
            path(&[(0.35, 0.2), (0.5, 0.05), (0.65, 0.2)]);
            path(&[(0.35, 0.8), (0.5, 0.95), (0.65, 0.8)]);
            path(&[(0.2, 0.35), (0.05, 0.5), (0.2, 0.65)]);
            path(&[(0.8, 0.35), (0.95, 0.5), (0.8, 0.65)]);
        }
        Icon::AddRig => {
            painter.circle_stroke(p(0.35, 0.2), 4., stroke);
            line((0.35, 0.4), (0.35, 0.68));
            path(&[(0.08, 0.57), (0.35, 0.42), (0.59, 0.58)]);
            path(&[(0.12, 0.96), (0.35, 0.68), (0.58, 0.96)]);
            line((0.65, 0.23), (0.98, 0.23));
            line((0.82, 0.07), (0.82, 0.4));
        }
        Icon::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(0.25, 0.08), p(0.91, 0.5), p(0.25, 0.92)],
                stroke.color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            painter.rect_filled(
                Rect::from_two_pos(p(0.2, 0.1), p(0.38, 0.9)),
                1.,
                stroke.color,
            );
            painter.rect_filled(
                Rect::from_two_pos(p(0.62, 0.1), p(0.8, 0.9)),
                1.,
                stroke.color,
            );
        }
        Icon::Previous => {
            line((0.14, 0.1), (0.14, 0.9));
            path(&[(0.85, 0.13), (0.33, 0.5), (0.85, 0.87), (0.85, 0.13)]);
        }
        Icon::Next => {
            line((0.86, 0.1), (0.86, 0.9));
            path(&[(0.15, 0.13), (0.67, 0.5), (0.15, 0.87), (0.15, 0.13)]);
        }
        Icon::Blank => {
            square((0.12, 0.05), (0.88, 0.95));
            line((0.32, 0.5), (0.68, 0.5));
            line((0.5, 0.32), (0.5, 0.68));
        }
        Icon::Duplicate => {
            square((0.05, 0.08), (0.7, 0.73));
            painter.rect_filled(Rect::from_two_pos(p(0.32, 0.34), p(0.97, 0.99)), 1., fill);
            square((0.32, 0.34), (0.97, 0.99));
        }
    }
    response.on_hover_text(label)
}
