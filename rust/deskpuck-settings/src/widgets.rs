use crate::theme::{CARD_RADIUS, CONTROL_RADIUS, palette, text_on};
use eframe::egui::{
    Align, CornerRadius, Frame, Id, Layout, Margin, Rect, Response, RichText, Sense, Stroke,
    TextStyle, Ui, Vec2, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};

pub const ROW_HEIGHT: f32 = 32.0;
const ROW_PADDING: i8 = 14;
const SEGMENT_HEIGHT: f32 = 26.0;
const SEGMENT_PADDING: f32 = 12.0;

pub fn section(ui: &mut Ui, title: &str, add_rows: impl FnOnce(&mut Card<'_>)) {
    ui.label(RichText::new(title).text_style(TextStyle::Heading).weak());
    ui.add_space(2.0);
    let colors = palette(ui.ctx().theme());
    Frame::new()
        .fill(colors.card)
        .stroke(Stroke::new(1.0, colors.card_stroke))
        .corner_radius(CARD_RADIUS)
        .inner_margin(Margin::symmetric(ROW_PADDING, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add_rows(&mut Card { ui, rows: 0 });
        });
    ui.add_space(16.0);
}

pub struct Card<'a> {
    ui: &'a mut Ui,
    rows: usize,
}

impl Card<'_> {
    /// One row: `label` on the left, what `control` adds on the right. In the
    /// right-to-left control area, the first widget added sits rightmost.
    pub fn row<R>(
        &mut self,
        label: impl Into<WidgetText>,
        control: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        if self.rows > 0 {
            let y = self.ui.cursor().top();
            let line = Stroke::new(1.0, palette(self.ui.ctx().theme()).separator);
            self.ui.painter().hline(self.ui.max_rect().x_range(), y, line);
        }
        self.rows += 1;
        let width = self.ui.available_width();
        let row = Layout::left_to_right(Align::Center);
        self.ui
            .allocate_ui_with_layout(vec2(width, ROW_HEIGHT), row, |ui| {
                ui.set_min_size(vec2(width, ROW_HEIGHT));
                ui.label(label);
                ui.with_layout(Layout::right_to_left(Align::Center), control).inner
            })
            .inner
    }
}

pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let checked = *on;
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, checked, label));
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, checked);
        let colors = palette(ui.ctx().theme());
        let accent = ui.visuals().selection.bg_fill;
        let track = if checked { accent } else { colors.toggle_off };
        let radius = rect.height() / 2.0;
        let painter = ui.painter();
        painter.rect_filled(rect, radius, track);
        if response.has_focus() {
            let ring = Stroke::new(2.0, accent);
            painter.rect_stroke(
                rect.expand(2.0),
                radius + 2.0,
                ring,
                eframe::egui::StrokeKind::Outside,
            );
        }
        let knob_x = eframe::egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        painter.circle_filled(
            pos2(knob_x, rect.center().y),
            radius - 3.0,
            eframe::egui::Color32::WHITE,
        );
    }
    response
}

pub fn segments(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    labels: &[&str],
    selected: impl Fn(usize) -> bool,
) -> Option<usize> {
    let id = Id::new(id);
    let font = TextStyle::Button.resolve(ui.style());
    let text_color = ui.visuals().widgets.inactive.fg_stroke.color;
    let galleys: Vec<_> = labels
        .iter()
        .map(|label| ui.painter().layout_no_wrap((*label).to_owned(), font.clone(), text_color))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 2.0 * SEGMENT_PADDING).collect();
    let size = Vec2::new(widths.iter().sum(), SEGMENT_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let colors = palette(ui.ctx().theme());
    let accent = ui.visuals().selection.bg_fill;
    let radius = f32::from(CONTROL_RADIUS);
    ui.painter().rect(
        rect,
        radius,
        colors.control,
        Stroke::new(1.0, colors.control_stroke),
        eframe::egui::StrokeKind::Inside,
    );
    let mut clicked = None;
    let mut left = rect.left();
    for (i, (galley, width)) in galleys.into_iter().zip(widths).enumerate() {
        let segment = Rect::from_min_size(pos2(left, rect.top()), vec2(width, SEGMENT_HEIGHT));
        left += width;
        let response = ui.interact(segment, id.with(i), Sense::click());
        let on = selected(i);
        response
            .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, on, labels[i]));
        if response.clicked() {
            clicked = Some(i);
        }
        let fill = if on {
            Some(accent)
        } else if response.hovered() {
            Some(colors.control_hover)
        } else {
            None
        };
        if let Some(fill) = fill {
            let corners = CornerRadius {
                nw: if i == 0 { CONTROL_RADIUS } else { 0 },
                sw: if i == 0 { CONTROL_RADIUS } else { 0 },
                ne: if i + 1 == labels.len() { CONTROL_RADIUS } else { 0 },
                se: if i + 1 == labels.len() { CONTROL_RADIUS } else { 0 },
            };
            ui.painter().rect_filled(segment.shrink(1.0), corners, fill);
        }
        if response.has_focus() {
            let ring = Stroke::new(2.0, accent);
            ui.painter().rect_stroke(segment, radius, ring, eframe::egui::StrokeKind::Inside);
        }
        let color = if on { text_on(accent) } else { text_color };
        let at = segment.center() - galley.size() / 2.0;
        ui.painter().galley_with_override_text_color(at, galley, color);
    }
    clicked
}
