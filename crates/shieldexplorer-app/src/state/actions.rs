//! Acciones de teclado de navegación: moverse, abrir, subir, historial y mostrar u ocultar cosas.

use gpui::{Context, Window};

use super::Explorer;
use crate::keys as k;

impl Explorer {
    pub fn on_move_down(&mut self, _: &k::MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.take_count();
        self.move_by(n.try_into().unwrap_or(isize::MAX), cx);
    }

    pub fn on_move_up(&mut self, _: &k::MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.take_count();
        self.move_by(-isize::try_from(n).unwrap_or(isize::MAX), cx);
    }

    pub fn on_page_down(&mut self, _: &k::PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by(self.page_rows as isize, cx);
    }

    pub fn on_page_up(&mut self, _: &k::PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by(-(self.page_rows as isize), cx);
    }

    pub fn on_half_down(&mut self, _: &k::HalfDown, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by((self.page_rows / 2).max(1) as isize, cx);
    }

    pub fn on_half_up(&mut self, _: &k::HalfUp, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by(-((self.page_rows / 2).max(1) as isize), cx);
    }

    /// `gg` va a la primera fila; con número (`5gg`), a esa fila.
    pub fn on_first(&mut self, _: &k::First, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_some() {
            return;
        }
        let line = self.count.take().map_or(0, |c| c.saturating_sub(1));
        self.select(line.min(self.entries.len().saturating_sub(1)), cx);
    }

    /// `G` va a la última fila; con número (`5G`), a esa fila.
    pub fn on_last(&mut self, _: &k::Last, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_some() || self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() - 1;
        let line = self.count.take().map_or(last, |c| c.saturating_sub(1));
        self.select(line.min(last), cx);
    }

    /// Mayús+flecha: empieza la selección visual (si no estaba) y se mueve.
    pub fn on_extend_down(&mut self, _: &k::ExtendDown, _: &mut Window, cx: &mut Context<Self>) {
        self.extend(1, cx);
    }

    pub fn on_extend_up(&mut self, _: &k::ExtendUp, _: &mut Window, cx: &mut Context<Self>) {
        self.extend(-1, cx);
    }

    fn extend(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.error.is_some() || self.entries.is_empty() {
            return;
        }
        if self.visual.is_none() {
            self.visual = Some(self.selected);
            self.sync_visual();
        }
        self.move_by(delta, cx);
    }

    pub fn on_open(&mut self, _: &k::Open, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        if self.error.is_none() {
            self.open_index(self.selected, cx);
        }
    }

    pub fn on_parent(&mut self, _: &k::Parent, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.go_parent(cx);
    }

    pub fn on_go_back(&mut self, _: &k::GoBack, _: &mut Window, cx: &mut Context<Self>) {
        self.go_back(cx);
    }

    pub fn on_go_forward(&mut self, _: &k::GoForward, _: &mut Window, cx: &mut Context<Self>) {
        self.go_forward(cx);
    }

    pub fn on_go_home(&mut self, _: &k::GoHome, _: &mut Window, cx: &mut Context<Self>) {
        self.go_home(cx);
    }

    pub fn on_trash_folder(&mut self, _: &k::TrashFolder, _: &mut Window, cx: &mut Context<Self>) {
        self.go_trash(cx);
    }

    /// `Esc`, de lo más inmediato a lo menos: número pendiente, mensaje de error, panel de salida,
    /// selección visual, marcas y por último el aviso.
    pub fn on_dismiss(&mut self, _: &k::Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.count.take().is_some()
            || self.error.take().is_some()
            || self.visual.take().is_some()
            || (!self.marked.is_empty() && {
                self.marked.clear();
                true
            })
            || self.notice.take().is_some()
        {
            cx.notify();
            return;
        }
        if self.output.is_some() {
            self.close_output();
            cx.notify();
        }
    }

    pub fn on_toggle_hidden(
        &mut self,
        _: &k::ToggleHidden,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_hidden(cx);
    }

    pub fn on_reload(&mut self, _: &k::Reload, _: &mut Window, cx: &mut Context<Self>) {
        self.refresh(None, cx);
    }

    pub fn on_toggle_mode(&mut self, _: &k::ToggleMode, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mode(cx);
    }
}
