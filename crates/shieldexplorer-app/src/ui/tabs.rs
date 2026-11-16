//! Barra de pestañas (solo se ve si hay más de una).

use gpui::{ClickEvent, Context, Div, FontWeight, div, prelude::*, px};

use crate::state::Explorer;
use crate::theme::Theme;

impl Explorer {
    pub(super) fn render_tabs(&self, t: &Theme, cx: &Context<Self>) -> Option<Div> {
        if self.tabs.len() < 2 {
            return None;
        }
        let hover = t.cursor;
        Some(
            div()
                .h(px(28.))
                .flex_none()
                .flex()
                .items_end()
                .gap(px(2.))
                .px(px(8.))
                .bg(t.bar)
                .border_b_1()
                .border_color(t.border)
                .children((0..self.tabs.len()).map(|i| {
                    let active = i == self.active_tab;
                    div()
                        .id(("tab", i))
                        .h(px(24.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .max_w(px(200.))
                        .truncate()
                        .rounded_t(px(4.))
                        .cursor_pointer()
                        .when(active, |d| {
                            d.bg(t.bg)
                                .text_color(t.accent)
                                .font_weight(FontWeight::BOLD)
                        })
                        .when(!active, |d| {
                            d.text_color(t.muted).hover(move |s| s.bg(hover))
                        })
                        .on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| this.switch_tab(i, cx)),
                        )
                        .child(self.tab_title(i))
                })),
        )
    }
}
