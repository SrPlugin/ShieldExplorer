//! Barra de estado: modo, datos de la entrada activa, posición y total de la carpeta.

use gpui::{Div, FontWeight, IntoElement, Styled, div, prelude::*, px};
use shieldexplorer_core::{format_size, mode_string};

use crate::state::Explorer;
use crate::theme::Theme;

impl Explorer {
    pub(super) fn render_status(&self, t: &Theme) -> impl IntoElement {
        let shown = self.error.is_none();
        let info = if let Some(error) = &self.error {
            error.code.to_owned()
        } else if self.slow {
            "cargando…".to_owned()
        } else if let Some(e) = self.entries.get(self.selected) {
            let size = if e.is_dir() {
                "dir".to_owned()
            } else {
                format!("{} B", e.size)
            };
            format!(
                "{}  {}  {}",
                mode_string(e.mode),
                size,
                e.name.to_string_lossy()
            )
        } else {
            String::new()
        };
        let position = if shown && !self.entries.is_empty() {
            format!("{}/{}", self.selected + 1, self.entries.len())
        } else {
            "0/0".to_owned()
        };
        let total = self
            .entries
            .iter()
            .filter(|e| !e.is_dir())
            .fold(0u64, |a, e| a.saturating_add(e.size));
        let mut summary = format!("Σ {} · UTF-8", format_size(total));
        if self.unreadable > 0 {
            summary = format!("{} ilegibles · {summary}", self.unreadable);
        }
        let mode = if self.editing.is_some() {
            "INSERT"
        } else {
            "NORMAL"
        };

        div()
            .h(px(24.))
            .flex_none()
            .flex()
            .border_t_1()
            .border_color(t.border)
            .bg(t.bar)
            .text_size(px(11.5))
            .text_color(t.muted)
            .child(
                cell()
                    .bg(t.accent)
                    .text_color(t.on_accent)
                    .font_weight(FontWeight::BOLD)
                    .child(mode),
            )
            .child(cell().flex_1().min_w_0().truncate().child(info))
            .child(cell().child(position))
            .child(cell().child(summary))
    }
}

fn cell() -> Div {
    div().px(px(10.)).flex().items_center().whitespace_nowrap()
}
