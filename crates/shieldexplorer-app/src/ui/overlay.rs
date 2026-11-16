//! Paneles flotantes: el buscador difuso y el estado de los discos.

use gpui::{Context, Div, FontWeight, KeyDownEvent, Window, div, prelude::*, px, relative};
use shieldexplorer_core::{Disk, format_size};

use crate::keys::OVERLAY_CONTEXT;
use crate::state::Explorer;
use crate::state::{DisksView, Finder, Overlay};
use crate::theme::Theme;

/// Filas visibles a la vez.
const VISIBLE: usize = 14;

/// Ventana de `VISIBLE` filas que siempre contiene la seleccionada.
fn window_start(selected: usize) -> usize {
    (selected + 1).saturating_sub(VISIBLE)
}

impl Explorer {
    /// El panel flotante, centrado arriba y por encima de la lista. `None` si no hay ninguno.
    pub(super) fn render_overlay(&self, t: &Theme, cx: &Context<Self>) -> Option<Div> {
        let overlay = self.overlay.as_ref()?;
        let content = match overlay {
            Overlay::Finder(finder) => finder_panel(finder, self.home.as_deref(), t),
            Overlay::Disks(view) => disks_panel(view, t),
        };
        Some(
            div()
                .absolute()
                .top(px(8.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .key_context(OVERLAY_CONTEXT)
                .track_focus(&self.overlay_focus)
                .on_action(cx.listener(Self::on_overlay_down))
                .on_action(cx.listener(Self::on_overlay_up))
                .on_action(cx.listener(Self::on_overlay_confirm))
                .on_action(cx.listener(Self::on_overlay_cancel))
                .on_key_down(
                    cx.listener(|this, event: &KeyDownEvent, _: &mut Window, cx| {
                        this.on_overlay_key(event, cx);
                    }),
                )
                .child(content),
        )
    }
}

fn panel(t: &Theme) -> Div {
    div()
        .w_full()
        .max_w(px(760.))
        .mx(px(12.))
        .flex()
        .flex_col()
        .bg(t.bar)
        .border_1()
        .border_color(t.border)
        .rounded(px(6.))
        .shadow_lg()
        .overflow_hidden()
}

fn finder_panel(finder: &Finder, home: Option<&std::path::Path>, t: &Theme) -> Div {
    let (before, after) = finder.query.text().split_at(finder.query.cursor());
    let title = if finder.folders {
        "carpetas"
    } else {
        "archivos"
    };
    let status = if finder.indexing {
        "indexando…".to_owned()
    } else {
        format!("{}/{}", finder.results.len(), finder.items.len())
    };
    let start = window_start(finder.selected);
    let root = shieldexplorer_core::display_path(&finder.root, home);
    let rows = finder
        .results
        .iter()
        .enumerate()
        .skip(start)
        .take(VISIBLE)
        .map(|(rank, &index)| {
            let text = &finder.items[index];
            // Archivos: la carpeta atenuada y el nombre resaltado.
            let (dir, name) = if finder.folders {
                ("", text.as_str())
            } else {
                text.rsplit_once('/')
                    .map_or(("", text.as_str()), |(d, n)| (&text[..=d.len()], n))
            };
            div()
                .px(px(12.))
                .h(px(24.))
                .flex()
                .items_center()
                .whitespace_nowrap()
                .overflow_hidden()
                .when(rank == finder.selected, |r| {
                    r.bg(t.cursor).border_l_2().border_color(t.accent)
                })
                .child(div().text_color(t.faint).child(dir.to_owned()))
                .child(
                    div()
                        .text_color(t.text)
                        .font_weight(FontWeight::BOLD)
                        .child(name.to_owned()),
                )
        });
    panel(t)
        .child(
            div()
                .h(px(34.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(8.))
                .border_b_1()
                .border_color(t.border)
                .child(
                    div()
                        .text_color(t.accent)
                        .font_weight(FontWeight::BOLD)
                        .child("❯"),
                )
                .child(div().text_color(t.faint).child(title))
                .child(div().child(before.to_owned()))
                .child(div().w(px(1.5)).h(px(15.)).bg(t.accent))
                .child(div().flex_1().child(after.to_owned()))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.faint)
                        .child(format!("{root} · {status}")),
                ),
        )
        .children(rows)
        .when(finder.results.is_empty() && !finder.indexing, |p| {
            p.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .text_color(t.faint)
                    .child("sin resultados"),
            )
        })
}

fn usage_color(disk: &Disk, t: &Theme) -> gpui::Hsla {
    match disk.percent() {
        90.. => t.danger,
        75..90 => t.git_color(shieldexplorer_core::GitMark::Modified),
        _ => t.accent,
    }
}

fn disks_panel(view: &DisksView, t: &Theme) -> Div {
    let start = window_start(view.selected);
    let rows = view
        .disks
        .iter()
        .enumerate()
        .skip(start)
        .take(VISIBLE)
        .map(|(i, disk)| {
            let pct = f32::from(disk.percent()) / 100.0;
            div()
                .px(px(12.))
                .h(px(32.))
                .flex()
                .items_center()
                .gap(px(12.))
                .when(i == view.selected, |r| {
                    r.bg(t.cursor).border_l_2().border_color(t.accent)
                })
                .child(
                    div()
                        .w(px(220.))
                        .flex_none()
                        .truncate()
                        .font_weight(FontWeight::BOLD)
                        .child(format!(
                            "{}{}",
                            disk.mount.display(),
                            if disk.removable() { "  ⏏" } else { "" }
                        )),
                )
                .child(
                    div()
                        .w(px(170.))
                        .flex_none()
                        .truncate()
                        .text_color(t.muted)
                        .child(format!("{} · {}", disk.device, disk.fstype)),
                )
                .child(
                    div()
                        .w(px(120.))
                        .flex_none()
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(t.border)
                        .child(
                            div()
                                .h_full()
                                .w(relative(pct))
                                .rounded(px(3.))
                                .bg(usage_color(disk, t)),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(t.muted)
                        .text_right()
                        .child(format!(
                            "{} de {} · {}% · libres {}",
                            format_size(disk.used),
                            format_size(disk.size),
                            disk.percent(),
                            format_size(disk.avail)
                        )),
                )
        });
    panel(t)
        .child(
            div()
                .h(px(34.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(8.))
                .border_b_1()
                .border_color(t.border)
                .child(div().font_weight(FontWeight::BOLD).child("discos"))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.faint)
                        .child(if view.loading {
                            "leyendo…".to_owned()
                        } else {
                            format!("{} montados · Enter entra · Esc cierra", view.disks.len())
                        }),
                ),
        )
        .children(rows)
}
