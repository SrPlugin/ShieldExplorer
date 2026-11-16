//! Interfaz: línea de ruta, pestañas, lista con vista previa, paneles flotantes, salida de
//! comandos y barra de estado.

mod list;
mod output;
mod overlay;
mod pathbar;
mod preview;
mod statusbar;
mod tabs;

use gpui::{Context, IntoElement, KeyDownEvent, Render, Window, div, prelude::*, px};

use crate::assets::MONO_FONT;
use crate::config::KeyMode;
use crate::keys::{LIST_CONTEXT_EXPLORER, LIST_CONTEXT_VIM};
use crate::state::Explorer;
use crate::theme::Theme;

/// Por debajo de este ancho se ocultan las columnas de permisos y de fecha.
const NARROW_WIDTH: f32 = 720.;
/// Por debajo de este ancho no cabe la vista previa.
const PREVIEW_MIN_WINDOW: f32 = 900.;
/// Alto de la línea de ruta (38), los encabezados (26) y la barra de estado (24).
const CHROME_HEIGHT: f32 = 88.;

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::get(cx);
        let viewport = window.viewport_size();
        let width: f32 = viewport.width.into();
        let height: f32 = viewport.height.into();
        let narrow = width < NARROW_WIDTH;
        let preview_width = (width * 0.38).max(300.);
        let show_preview = self.preview_on && width >= PREVIEW_MIN_WINDOW;
        self.page_rows = (((height - CHROME_HEIGHT) / list::ROW_HEIGHT) as usize).max(1);
        let context = match self.mode {
            KeyMode::Vim => LIST_CONTEXT_VIM,
            KeyMode::Explorer => LIST_CONTEXT_EXPLORER,
        };

        let list = div()
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
            .on_action(cx.listener(Self::on_extend_down))
            .on_action(cx.listener(Self::on_extend_up))
            .on_action(cx.listener(Self::on_open))
            .on_action(cx.listener(Self::on_open_default))
            .on_action(cx.listener(Self::on_open_editor))
            .on_action(cx.listener(Self::on_open_terminal))
            .on_action(cx.listener(Self::on_parent))
            .on_action(cx.listener(Self::on_go_back))
            .on_action(cx.listener(Self::on_go_forward))
            .on_action(cx.listener(Self::on_go_home))
            .on_action(cx.listener(Self::on_trash_folder))
            .on_action(cx.listener(Self::on_edit_path))
            .on_action(cx.listener(Self::on_dismiss))
            .on_action(cx.listener(Self::on_toggle_hidden))
            .on_action(cx.listener(Self::on_reload))
            .on_action(cx.listener(Self::on_toggle_mode))
            .on_action(cx.listener(Self::on_toggle_preview))
            .on_action(cx.listener(Self::on_compute_sizes))
            .on_action(cx.listener(Self::on_start_search))
            .on_action(cx.listener(Self::on_search_next))
            .on_action(cx.listener(Self::on_search_prev))
            .on_action(cx.listener(Self::on_start_command))
            .on_action(cx.listener(Self::on_start_set_mark))
            .on_action(cx.listener(Self::on_start_jump))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_rename_start))
            .on_action(cx.listener(Self::on_new_file))
            .on_action(cx.listener(Self::on_new_dir))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_visual_toggle))
            .on_action(cx.listener(Self::on_toggle_mark))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_yank_path))
            .on_action(cx.listener(Self::on_yank_name))
            .on_action(cx.listener(Self::on_yank_rel))
            .on_action(cx.listener(Self::on_finder_files))
            .on_action(cx.listener(Self::on_finder_folders))
            .on_action(cx.listener(Self::on_disks))
            .on_action(cx.listener(Self::on_cancel_command))
            .on_action(cx.listener(Self::on_tab_new))
            .on_action(cx.listener(Self::on_tab_close))
            .on_action(cx.listener(Self::on_tab_next))
            .on_action(cx.listener(Self::on_tab_prev))
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, _: &mut Window, cx| {
                    this.on_list_key(event, cx);
                }),
            )
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(self.render_header(&t, narrow, cx))
            .child(self.render_body(&t, narrow, cx));

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg)
            .text_color(t.text)
            .font_family(MONO_FONT)
            .text_size(px(12.5))
            .child(self.render_pathbar(&t, width, cx))
            .children(self.render_tabs(&t, cx))
            .child(
                // El panel flotante va hermano de la lista, no dentro: así los atajos de la lista no
                // se disparan mientras se escribe en él.
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .flex()
                    .child(list)
                    .when(show_preview, |row| {
                        row.child(self.render_preview(&t, preview_width))
                    })
                    .children(self.render_overlay(&t, cx)),
            )
            .children(self.render_output(&t))
            .child(self.render_status(&t, cx))
    }
}
