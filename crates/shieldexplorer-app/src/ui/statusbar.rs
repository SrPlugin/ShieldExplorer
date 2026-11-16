//! Barra de estado: modo, rama de git, selección y portapapeles, y la zona central, que muestra
//! lo que se está escribiendo, un aviso o los datos de la entrada activa.

use gpui::{
    ClickEvent, Context, Div, FontWeight, IntoElement, KeyDownEvent, Window, div, prelude::*, px,
};
use shieldexplorer_core::{format_size, mode_string};

use crate::config::KeyMode;
use crate::keys::PROMPT_CONTEXT;
use crate::state::{Explorer, PromptKind};
use crate::theme::Theme;

impl Explorer {
    fn mode_chip(&self) -> String {
        let base = match self.mode {
            KeyMode::Explorer => "EXPLORER",
            KeyMode::Vim => "VIM",
        };
        let sub = if let Some(prompt) = &self.prompt {
            match prompt.kind {
                PromptKind::Search => " SEARCH",
                PromptKind::Command => " CMD",
                PromptKind::Rename => " RENAME",
                PromptKind::NewFile | PromptKind::NewDir => " NEW",
                PromptKind::SetMark | PromptKind::JumpMark => " MARK",
            }
        } else if self.editing.is_some() {
            " INSERT"
        } else if self.visual.is_some() {
            " VISUAL"
        } else if self.mode == KeyMode::Vim {
            " NORMAL"
        } else {
            ""
        };
        format!("{base}{sub}")
    }

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
        let branch = self.git.as_ref().map(|g| {
            let mut text = format!(" {}", g.branch.name);
            if g.dirty() {
                text.push('*');
            }
            if g.branch.ahead > 0 {
                text.push_str(&format!(" ↑{}", g.branch.ahead));
            }
            if g.branch.behind > 0 {
                text.push_str(&format!(" ↓{}", g.branch.behind));
            }
            text
        });
        let clipboard = self
            .clipboard
            .as_ref()
            .map(|c| format!("{} {}", if c.cut { "✂" } else { "⧉" }, c.paths.len()));

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
                    .child(self.mode_chip()),
            )
            .when_some(self.count, |bar, count| {
                bar.child(
                    cell()
                        .text_color(t.accent)
                        .font_weight(FontWeight::BOLD)
                        .child(count.to_string()),
                )
            })
            .when_some(branch, |bar, text| {
                bar.child(cell().text_color(t.accent).child(text))
            })
            .when(!self.marked.is_empty(), |bar| {
                bar.child(
                    cell()
                        .text_color(t.accent)
                        .child(format!("{} marcados", self.marked.len())),
                )
            })
            .when_some(clipboard, |bar, text| bar.child(cell().child(text)))
            .child(self.render_info(t, cx))
            .child(cell().child(position))
            .child(cell().child(summary))
    }

    /// Zona central. Siempre existe para poder recibir el foco del teclado al escribir.
    fn render_info(&self, t: &Theme, cx: &Context<Self>) -> Div {
        let base = cell()
            .flex_1()
            .min_w_0()
            .key_context(PROMPT_CONTEXT)
            .track_focus(&self.prompt_focus)
            .on_action(cx.listener(Self::on_confirm_prompt))
            .on_action(cx.listener(Self::on_cancel_prompt))
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window: &mut Window, cx| {
                    this.on_prompt_key(event, window, cx);
                }),
            );
        if let Some(prompt) = &self.prompt {
            let base = base
                .text_color(t.text)
                .child(div().text_color(t.accent).child(prompt.kind.label()));
            if prompt.kind.single_key() {
                return base.child(
                    div()
                        .text_color(t.faint)
                        .child("pulsa una letra o un número"),
                );
            }
            let (before, after) = prompt.edit.text().split_at(prompt.edit.cursor());
            return base
                .child(before.to_owned())
                .child(div().w(px(1.5)).h(px(14.)).bg(t.accent))
                .child(after.to_owned());
        }
        if let Some(notice) = &self.notice {
            let color = if notice.error { t.danger } else { t.text };
            return base.truncate().text_color(color).child(notice.text.clone());
        }
        let info = if let Some(error) = &self.error {
            error.code.to_owned()
        } else if self.slow {
            "cargando…".to_owned()
        } else if self.sizing {
            "calculando tamaños…".to_owned()
        } else if let Some(e) = self.entries.get(self.selected) {
            let size = if e.is_dir() {
                "dir".to_owned()
            } else {
                format!("{} B", e.size)
            };
            let link = e
                .link
                .as_ref()
                .map(|l| format!("  → {}", l.display()))
                .unwrap_or_default();
            format!(
                "{}  {}  {}{link}",
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
