//! Barra de estado: modo, contador pendiente, datos de la entrada activa (o la búsqueda en
//! curso), posición y total de la carpeta.

use gpui::{
    ClickEvent, Context, Div, FontWeight, IntoElement, KeyDownEvent, Window, div, prelude::*, px,
};
use shieldexplorer_core::{format_size, mode_string};

use crate::config::KeyMode;
use crate::keys::SEARCH_CONTEXT;
use crate::state::Explorer;
use crate::theme::Theme;

impl Explorer {
    pub(super) fn render_status(&self, t: &Theme, cx: &Context<Self>) -> impl IntoElement {
        let position = if self.error.is_none() && !self.entries.is_empty() {
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
        if !self.show_hidden {
            summary = format!("sin ocultos · {summary}");
        }
        if self.unreadable > 0 {
            summary = format!("{} ilegibles · {summary}", self.unreadable);
        }
        let chip = match (self.mode, self.search.is_some(), self.editing.is_some()) {
            (KeyMode::Explorer, ..) => "EXPLORER",
            (KeyMode::Vim, true, _) => "VIM SEARCH",
            (KeyMode::Vim, false, true) => "VIM INSERT",
            (KeyMode::Vim, false, false) => "VIM NORMAL",
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
                    .id("mode-chip")
                    .bg(t.accent)
                    .text_color(t.on_accent)
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                        this.toggle_mode(cx);
                    }))
                    .child(chip),
            )
            .when_some(self.count, |bar, count| {
                bar.child(
                    cell()
                        .text_color(t.accent)
                        .font_weight(FontWeight::BOLD)
                        .child(count.to_string()),
                )
            })
            .child(self.render_info(t, cx))
            .child(cell().child(position))
            .child(cell().child(summary))
    }

    /// Zona central: la búsqueda mientras se escribe, o los datos de la entrada activa. Siempre
    /// existe para poder recibir el foco del teclado al empezar a buscar.
    fn render_info(&self, t: &Theme, cx: &Context<Self>) -> Div {
        let base = cell()
            .flex_1()
            .min_w_0()
            .key_context(SEARCH_CONTEXT)
            .track_focus(&self.search_focus)
            .on_action(cx.listener(Self::on_confirm_search))
            .on_action(cx.listener(Self::on_cancel_search))
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, _: &mut Window, cx| {
                    this.on_search_key(event, cx);
                }),
            );
        if let Some(search) = &self.search {
            let (before, after) = search.edit.text().split_at(search.edit.cursor());
            return base
                .text_color(t.text)
                .child("/")
                .child(before.to_owned())
                .child(div().w(px(1.5)).h(px(14.)).bg(t.accent))
                .child(after.to_owned());
        }
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
        base.truncate().child(info)
    }
}

fn cell() -> Div {
    div().px(px(10.)).flex().items_center().whitespace_nowrap()
}
