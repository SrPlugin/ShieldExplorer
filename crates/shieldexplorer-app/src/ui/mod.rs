//! Interfaz: línea de ruta, encabezados, lista y barra de estado.

mod list;
mod pathbar;
mod statusbar;

use gpui::{Context, IntoElement, KeyDownEvent, Render, Window, div, prelude::*, px};

use crate::assets::MONO_FONT;
use crate::config::KeyMode;
use crate::keys::{LIST_CONTEXT_EXPLORER, LIST_CONTEXT_VIM};
use crate::state::Explorer;
use crate::theme::Theme;

/// Por debajo de este ancho se ocultan las columnas de permisos y de fecha.
const NARROW_WIDTH: f32 = 720.;
/// Alto de la línea de ruta (38), los encabezados (26) y la barra de estado (24).
const CHROME_HEIGHT: f32 = 88.;

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::get(cx);
        let viewport = window.viewport_size();
        let narrow = viewport.width < px(NARROW_WIDTH);
        let height: f32 = viewport.height.into();
        self.page_rows = (((height - CHROME_HEIGHT) / list::ROW_HEIGHT) as usize).max(1);
        let context = match self.mode {
            KeyMode::Vim => LIST_CONTEXT_VIM,
            KeyMode::Explorer => LIST_CONTEXT_EXPLORER,
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg)
            .text_color(t.text)
            .font_family(MONO_FONT)
            .text_size(px(12.5))
            .child(self.render_pathbar(&t, cx))
            .child(
                div()
                    .key_context(context)
                    .track_focus(&self.focus)
                    .on_action(cx.listener(Self::on_move_down))
                    .on_action(cx.listener(Self::on_move_up))
                    .on_action(cx.listener(Self::on_page_down))
                    .on_action(cx.listener(Self::on_page_up))
                    .on_action(cx.listener(Self::on_half_down))
                    .on_action(cx.listener(Self::on_half_up))
                    .on_action(cx.listener(Self::on_first))
                    .on_action(cx.listener(Self::on_last))
                    .on_action(cx.listener(Self::on_open))
                    .on_action(cx.listener(Self::on_parent))
                    .on_action(cx.listener(Self::on_edit_path))
                    .on_action(cx.listener(Self::on_go_back))
                    .on_action(cx.listener(Self::on_go_forward))
                    .on_action(cx.listener(Self::on_dismiss))
                    .on_action(cx.listener(Self::on_toggle_hidden))
                    .on_action(cx.listener(Self::on_reload))
                    .on_action(cx.listener(Self::on_toggle_mode))
                    .on_action(cx.listener(Self::on_start_search))
                    .on_action(cx.listener(Self::on_search_next))
                    .on_action(cx.listener(Self::on_search_prev))
                    .on_key_down(
                        cx.listener(|this, event: &KeyDownEvent, _: &mut Window, cx| {
                            this.on_list_key(event, cx);
                        }),
                    )
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.render_header(&t, narrow, cx))
                    .child(self.render_body(&t, narrow, cx)),
            )
            .child(self.render_status(&t, cx))
    }
}
