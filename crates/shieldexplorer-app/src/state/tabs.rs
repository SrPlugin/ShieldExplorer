//! Pestañas. La pestaña activa vive en el propio `Explorer`; las demás son instantáneas (carpeta,
//! historial, orden y fila) que se vuelven a leer del disco al activarlas.

use std::ffi::OsString;
use std::path::PathBuf;

use gpui::{Context, Window};
use shieldexplorer_core::{History, Sort};

use super::{Explorer, Nav};
use crate::keys as k;

#[derive(Clone)]
pub struct TabSnap {
    pub cwd: PathBuf,
    history: History,
    select: Option<OsString>,
    sort: Sort,
}

impl TabSnap {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            history: History::default(),
            select: None,
            sort: Sort::default(),
        }
    }
}

impl Explorer {
    fn snapshot(&self) -> TabSnap {
        TabSnap {
            cwd: self.cwd.clone(),
            history: self.history.clone(),
            select: self.selected_name(),
            sort: self.sort,
        }
    }

    /// Título de la pestaña `index`: el nombre de su carpeta.
    pub fn tab_title(&self, index: usize) -> String {
        let cwd = if index == self.active_tab {
            &self.cwd
        } else {
            &self.tabs[index].cwd
        };
        cwd.file_name()
            .map_or_else(|| "/".to_owned(), |n| n.to_string_lossy().into_owned())
    }

    /// Pestaña nueva: una copia de la actual, a su derecha.
    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let snap = self.snapshot();
        self.tabs[self.active_tab] = snap.clone();
        self.tabs.insert(self.active_tab + 1, snap);
        self.active_tab += 1;
        cx.notify();
    }

    pub fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || index == self.active_tab {
            return;
        }
        self.tabs[self.active_tab] = self.snapshot();
        self.load_tab(index, cx);
    }

    fn load_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let snap = self.tabs[index].clone();
        self.active_tab = index;
        self.history = snap.history;
        self.sort = snap.sort;
        self.navigate(snap.cwd, snap.select, Nav::Replace, cx);
    }

    pub fn close_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() == 1 {
            self.show_notice("es la única pestaña", true, cx);
            return;
        }
        self.tabs.remove(self.active_tab);
        self.load_tab(self.active_tab.min(self.tabs.len() - 1), cx);
    }

    fn cycle_tab(&mut self, step: isize, cx: &mut Context<Self>) {
        let len = self.tabs.len() as isize;
        let next = (self.active_tab as isize + step).rem_euclid(len) as usize;
        self.switch_tab(next, cx);
    }

    pub fn on_tab_new(&mut self, _: &k::TabNew, _: &mut Window, cx: &mut Context<Self>) {
        self.new_tab(cx);
    }

    pub fn on_tab_close(&mut self, _: &k::TabClose, _: &mut Window, cx: &mut Context<Self>) {
        self.close_tab(cx);
    }

    pub fn on_tab_next(&mut self, _: &k::TabNext, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(1, cx);
    }

    pub fn on_tab_prev(&mut self, _: &k::TabPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(-1, cx);
    }
}
