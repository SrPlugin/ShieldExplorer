//! Encabezados y filas del listado, con las filas virtualizadas.

use std::ops::Range;

use gpui::{
    ClickEvent, Context, Div, FontWeight, IntoElement, Stateful, div, prelude::*, px,
    transparent_black, uniform_list,
};
use shieldexplorer_core::{Class, Kind, SortKey, format_size, mode_string};

use crate::state::Explorer;
use crate::theme::Theme;

const ROW_HEIGHT: f32 = 24.;
const W_LINE: f32 = 44.;
const W_PERMS: f32 = 92.;
const W_SIZE: f32 = 84.;
const W_TIME: f32 = 142.;

impl Explorer {
    pub(super) fn render_header(
        &self,
        t: &Theme,
        narrow: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .h(px(26.))
            .flex_none()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(t.border)
            .text_size(px(10.5))
            .child(
                div()
                    .w(px(W_LINE))
                    .px(px(8.))
                    .text_color(t.faint)
                    .child("#"),
            )
            .when(!narrow, |row| {
                row.child(
                    div()
                        .w(px(W_PERMS))
                        .px(px(8.))
                        .text_color(t.faint)
                        .child("PERMISOS"),
                )
            })
            .child(
                self.sort_cell("sort-size", SortKey::Size, "TAMAÑO", t, cx)
                    .w(px(W_SIZE))
                    .justify_end(),
            )
            .when(!narrow, |row| {
                row.child(
                    self.sort_cell("sort-time", SortKey::Modified, "MODIFICADO", t, cx)
                        .w(px(W_TIME)),
                )
            })
            .child(
                self.sort_cell("sort-name", SortKey::Name, "NOMBRE", t, cx)
                    .flex_1()
                    .min_w_0(),
            )
    }

    fn sort_cell(
        &self,
        id: &'static str,
        key: SortKey,
        label: &'static str,
        t: &Theme,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let active = self.sort.key == key;
        let arrow = match (active, self.sort.ascending) {
            (false, _) => "",
            (true, true) => " ▲",
            (true, false) => " ▼",
        };
        let hover = t.text;
        div()
            .id(id)
            .h_full()
            .flex()
            .items_center()
            .px(px(8.))
            .cursor_pointer()
            .text_color(if active { t.accent } else { t.faint })
            .hover(move |s| s.text_color(hover))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_sort(key, cx)))
            .child(format!("{label}{arrow}"))
    }

    /// Cuerpo bajo los encabezados: la lista, o un aviso si hubo error o la carpeta está vacía.
    pub(super) fn render_body(
        &self,
        t: &Theme,
        narrow: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let body = div().flex_1().min_h_0();
        if let Some(error) = &self.error {
            let text = format!("{} — te quedas en {}", error.message, self.shown_cwd());
            return body.child(notice(t, format!("error[{}]", error.code), text, t.danger));
        }
        if self.entries.is_empty() {
            return body.child(notice(
                t,
                "carpeta vacía".into(),
                "0 entradas".into(),
                t.muted,
            ));
        }
        body.child(
            uniform_list(
                "entries",
                self.entries.len(),
                cx.processor(move |this, range: Range<usize>, _window, cx| {
                    let t = Theme::get(cx);
                    range
                        .map(|ix| this.render_row(ix, &t, narrow, cx))
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(self.scroll.clone())
            .size_full(),
        )
    }

    fn shown_cwd(&self) -> String {
        shieldexplorer_core::display_path(&self.cwd, self.home.as_deref())
    }

    fn render_row(&self, ix: usize, t: &Theme, narrow: bool, cx: &Context<Self>) -> Stateful<Div> {
        let row = div()
            .id(ix)
            .w_full()
            .h(px(ROW_HEIGHT))
            .flex()
            .items_center();
        let Some(entry) = self.entries.get(ix) else {
            return row;
        };
        let selected = ix == self.selected;

        let mut name = entry.name.to_string_lossy().into_owned();
        match entry.kind {
            Kind::Dir => name.push('/'),
            Kind::Symlink(_) => name.push('@'),
            Kind::File => {}
        }
        let line = if selected {
            (ix + 1).to_string()
        } else {
            ix.abs_diff(self.selected).to_string()
        };
        let size = if entry.is_dir() {
            "—".to_owned()
        } else {
            format_size(entry.size)
        };
        let class = entry.class();
        let hover = t.cursor;

        row.border_l_2()
            .border_color(if selected {
                t.accent
            } else {
                transparent_black()
            })
            .when(selected, |r| r.bg(t.cursor))
            .when(!selected, |r| r.hover(move |s| s.bg(hover)))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus);
                this.select(ix, cx);
                if event.click_count() >= 2 {
                    this.open_index(ix, cx);
                }
            }))
            .child(
                div()
                    .w(px(W_LINE - 2.))
                    .px(px(8.))
                    .when(selected, |c| {
                        c.text_color(t.accent).font_weight(FontWeight::BOLD)
                    })
                    .when(!selected, |c| c.text_color(t.faint).text_right())
                    .child(line),
            )
            .when(!narrow, |r| {
                r.child(
                    div()
                        .w(px(W_PERMS))
                        .px(px(8.))
                        .text_color(t.muted)
                        .child(mode_string(entry.mode)),
                )
            })
            .child(
                div()
                    .w(px(W_SIZE))
                    .px(px(8.))
                    .text_color(t.muted)
                    .text_right()
                    .child(size),
            )
            .when(!narrow, |r| {
                r.child(
                    div()
                        .w(px(W_TIME))
                        .px(px(8.))
                        .text_color(t.muted)
                        .child(self.format_modified(entry.modified)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(8.))
                    .truncate()
                    .text_color(t.class_color(class))
                    .when(class == Class::Dir, |c| c.font_weight(FontWeight::BOLD))
                    .when(entry.is_hidden(), |c| c.opacity(0.55))
                    .child(name),
            )
    }
}

fn notice(t: &Theme, title: String, detail: String, color: gpui::Hsla) -> Div {
    div()
        .px(px(56.))
        .py(px(20.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_color(color)
                .font_weight(FontWeight::BOLD)
                .child(title),
        )
        .child(div().text_color(t.muted).child(detail))
}
