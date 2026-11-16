//! Panel inferior con la salida del último comando `:!`.

use gpui::{Div, FontWeight, div, prelude::*, px};

use crate::state::Explorer;
use crate::theme::Theme;

/// Líneas visibles de la salida.
const SHOWN_LINES: usize = 12;
const LINE_HEIGHT: f32 = 18.;

impl Explorer {
    pub(super) fn render_output(&self, t: &Theme) -> Option<Div> {
        let output = self.output.as_ref()?;
        let (status, color) = match output.finished {
            None => ("ejecutando… (Ctrl+C cancela)".to_owned(), t.accent),
            Some(Some(0)) => (
                "terminó bien".to_owned(),
                t.git_color(shieldexplorer_core::GitMark::Added),
            ),
            Some(Some(code)) => (format!("terminó con código {code}"), t.danger),
            Some(None) => ("terminó por una señal".to_owned(), t.danger),
        };
        let skip = output.lines.len().saturating_sub(SHOWN_LINES);
        Some(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(t.border)
                .bg(t.bar)
                .child(
                    div()
                        .h(px(26.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .border_b_1()
                        .border_color(t.border)
                        .child(
                            div()
                                .text_color(t.accent)
                                .font_weight(FontWeight::BOLD)
                                .child("$"),
                        )
                        .child(div().flex_1().truncate().child(output.command.clone()))
                        .child(div().text_color(color).child(status))
                        .child(div().text_color(t.faint).child("Esc cierra")),
                )
                .child(
                    div()
                        .h(px(LINE_HEIGHT * SHOWN_LINES as f32 + 8.))
                        .px(px(12.))
                        .py(px(4.))
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .children(output.lines.iter().skip(skip).map(|line| {
                            div()
                                .h(px(LINE_HEIGHT))
                                .flex_none()
                                .whitespace_nowrap()
                                .text_color(t.muted)
                                .child(line.clone())
                        })),
                ),
        )
    }
}
