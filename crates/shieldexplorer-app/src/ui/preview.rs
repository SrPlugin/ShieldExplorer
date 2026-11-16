//! Panel de vista previa a la derecha: contenido de la carpeta o archivo seleccionado.

use gpui::{Div, FontWeight, div, img, prelude::*, px};
use shieldexplorer_core::{Preview, format_size};

use crate::state::Explorer;
use crate::theme::Theme;

impl Explorer {
    pub(super) fn render_preview(&self, t: &Theme, width: f32) -> Div {
        let panel = div()
            .w(px(width))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_l_1()
            .border_color(t.border);
        let Some((path, preview)) = &self.preview else {
            return panel;
        };
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let (kind, detail) = match preview {
            Preview::Dir { total, .. } => ("carpeta", format!("{total} elementos")),
            Preview::Text { lines, truncated } => (
                "texto",
                format!(
                    "{} líneas{}",
                    lines.len(),
                    if *truncated { " (recortado)" } else { "" }
                ),
            ),
            Preview::Image => ("imagen", String::new()),
            Preview::Binary { size } => ("binario", format_size(*size)),
            Preview::Empty => ("vacío", String::new()),
            Preview::Error(_) => ("error", String::new()),
        };
        let header = div()
            .flex_none()
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(t.border)
            .flex()
            .flex_col()
            .child(div().truncate().font_weight(FontWeight::BOLD).child(name))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.faint)
                    .child(format!("{kind} {detail}")),
            );
        panel
            .child(header)
            .child(self.preview_body(path, preview, t))
    }

    fn preview_body(&self, path: &std::path::Path, preview: &Preview, t: &Theme) -> Div {
        let body = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .px(px(12.))
            .py(px(8.))
            .flex()
            .flex_col();
        match preview {
            Preview::Dir { entries, total } => {
                let shown = entries.len();
                body.children(entries.iter().map(|(name, is_dir)| {
                    let (label, color) = if *is_dir {
                        (
                            format!("{name}/"),
                            t.class_color(shieldexplorer_core::Class::Dir),
                        )
                    } else {
                        (name.clone(), t.text)
                    };
                    div()
                        .w_full()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_color(color)
                        .when(*is_dir, |d| d.font_weight(FontWeight::BOLD))
                        .child(label)
                }))
                .when(*total > shown, |b| {
                    b.child(
                        div()
                            .mt(px(6.))
                            .text_color(t.faint)
                            .child(format!("… y {} más", total - shown)),
                    )
                })
                .when(*total == 0, |b| {
                    b.child(div().text_color(t.faint).child("carpeta vacía"))
                })
            }
            Preview::Text { lines, truncated } => body
                .children(lines.iter().enumerate().map(|(i, line)| {
                    div()
                        .flex()
                        .gap(px(10.))
                        .child(
                            div()
                                .w(px(30.))
                                .flex_none()
                                .text_right()
                                .text_color(t.faint)
                                .child((i + 1).to_string()),
                        )
                        .child(div().whitespace_nowrap().child(line.clone()))
                }))
                .when(*truncated, |b| {
                    b.child(div().mt(px(6.)).text_color(t.faint).child("… recortado"))
                }),
            Preview::Image => body.child(img(path.to_path_buf()).w_full().max_h(px(560.))),
            Preview::Binary { size } => body.child(
                div()
                    .text_color(t.faint)
                    .child(format!("archivo binario · {}", format_size(*size))),
            ),
            Preview::Empty => body.child(div().text_color(t.faint).child("archivo vacío")),
            Preview::Error(message) => {
                body.child(div().text_color(t.danger).child(message.clone()))
            }
        }
    }
}
