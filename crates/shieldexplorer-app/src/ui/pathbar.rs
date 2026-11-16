//! Línea de ruta: botones de navegación, prompt y migas de pan (o el campo de edición).

use gpui::{
    ClickEvent, Context, Div, IntoElement, KeyDownEvent, Stateful, Window, div, prelude::*, px,
};
use shieldexplorer_core::breadcrumbs;

use crate::config::KeyMode;
use crate::keys::PATH_CONTEXT;
use crate::state::Explorer;
use crate::theme::Theme;

impl Explorer {
    pub(super) fn render_pathbar(
        &self,
        t: &Theme,
        width: f32,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let can_up = self.cwd.parent().is_some();
        div()
            .key_context(PATH_CONTEXT)
            .track_focus(&self.path_focus)
            .on_action(cx.listener(Self::on_confirm_path))
            .on_action(cx.listener(Self::on_cancel_edit))
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, _: &mut Window, cx| {
                    this.on_path_key(event, cx);
                }),
            )
            .h(px(38.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .bg(t.bar)
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .child(nav_button(
                        "back",
                        "←",
                        self.can_go_back(),
                        t,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.go_back(cx)),
                    ))
                    .child(nav_button(
                        "forward",
                        "→",
                        self.can_go_forward(),
                        t,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.go_forward(cx)),
                    ))
                    .child(nav_button(
                        "up",
                        "↑",
                        can_up,
                        t,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.go_parent(cx)),
                    )),
            )
            .child(
                div()
                    .text_color(t.accent)
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("❯"),
            )
            .child(self.render_path_field(t, width, cx))
            .when(self.editing.is_none(), |bar| {
                let hint = match self.mode {
                    KeyMode::Vim => "i editar ruta · / buscar",
                    KeyMode::Explorer => "Ctrl+L editar ruta",
                };
                bar.child(div().text_size(px(11.)).text_color(t.faint).child(hint))
            })
    }

    fn render_path_field(&self, t: &Theme, width: f32, cx: &Context<Self>) -> Stateful<Div> {
        let field = div()
            .id("path-field")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center();
        if let Some(edit) = &self.editing {
            let (before, after) = edit.text().split_at(edit.cursor());
            return field
                .cursor_text()
                .child(div().child(before.to_owned()))
                .child(div().w(px(1.5)).h(px(15.)).bg(t.accent))
                .child(div().child(after.to_owned()));
        }
        let mut crumbs = div()
            .flex()
            .items_center()
            .overflow_hidden()
            .whitespace_nowrap();
        let all = breadcrumbs(&self.cwd, self.home.as_deref());
        // Si la ruta no cabe, se abrevia el principio y se deja visible el final. Se reserva sitio
        // para los botones y la pista, a unos 7,6 px por carácter.
        let budget = (((width - 330.) / 7.6).max(12.)) as usize;
        let mut used = 0;
        let mut first = all.len();
        for (i, (label, _)) in all.iter().enumerate().rev() {
            let cost = label.chars().count() + 1;
            if used + cost > budget && first < all.len() {
                break;
            }
            used += cost;
            first = i;
        }
        let hidden_parent = (first > 0).then(|| all[first - 1].1.clone());
        let mut after_root = true; // sin separador antes del primer segmento ni tras `/`
        if let Some(parent) = hidden_parent {
            let hover = t.cursor;
            crumbs = crumbs.child(
                div()
                    .id("crumb-hidden")
                    .px(px(3.))
                    .rounded(px(3.))
                    .cursor_pointer()
                    .text_color(t.faint)
                    .hover(move |s| s.bg(hover))
                    .child("…")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.go_to(parent.clone(), cx);
                    })),
            );
            after_root = false;
        }
        for (i, (label, path)) in all.into_iter().enumerate().skip(first) {
            if !after_root {
                crumbs = crumbs.child(div().text_color(t.faint).child("/"));
            }
            after_root = label == "/";
            let hover = t.cursor;
            let accent = t.accent;
            crumbs = crumbs.child(
                div()
                    .id(("crumb", i))
                    .px(px(3.))
                    .rounded(px(3.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover).text_color(accent))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.go_to(path.clone(), cx);
                    })),
            );
        }
        field
            .cursor_text()
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.begin_edit(window, cx)))
            .child(crumbs)
    }
}

fn nav_button(
    id: &'static str,
    glyph: &'static str,
    enabled: bool,
    t: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> Stateful<Div> {
    let (muted, text, border, bg) = (t.muted, t.text, t.border, t.bg);
    let button = div()
        .id(id)
        .w(px(26.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_color(muted)
        .child(glyph);
    if enabled {
        button
            .cursor_pointer()
            .hover(move |s| s.text_color(text).border_color(border).bg(bg))
            .on_click(on_click)
    } else {
        button.opacity(0.35)
    }
}
