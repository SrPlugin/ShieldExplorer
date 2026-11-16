//! Interfaz: línea de ruta, encabezados, lista y barra de estado.

mod list;
mod pathbar;
mod statusbar;

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};

use crate::assets::MONO_FONT;
use crate::keys::LIST_CONTEXT;
use crate::state::Explorer;
use crate::theme::Theme;

/// Por debajo de este ancho se ocultan las columnas de permisos y de fecha.
const NARROW_WIDTH: f32 = 720.;

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::get(cx);
        let narrow = window.viewport_size().width < px(NARROW_WIDTH);

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
                    .key_context(LIST_CONTEXT)
                    .track_focus(&self.focus)
                    .on_action(cx.listener(Self::on_move_down))
                    .on_action(cx.listener(Self::on_move_up))
                    .on_action(cx.listener(Self::on_first))
                    .on_action(cx.listener(Self::on_last))
                    .on_action(cx.listener(Self::on_open))
                    .on_action(cx.listener(Self::on_parent))
                    .on_action(cx.listener(Self::on_edit_path))
                    .on_action(cx.listener(Self::on_go_back))
                    .on_action(cx.listener(Self::on_go_forward))
                    .on_action(cx.listener(Self::on_dismiss))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.render_header(&t, narrow, cx))
                    .child(self.render_body(&t, narrow, cx)),
            )
            .child(self.render_status(&t))
    }
}
