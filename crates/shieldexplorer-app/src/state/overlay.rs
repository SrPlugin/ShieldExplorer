//! Paneles que se abren sobre la lista: el buscador difuso (archivos del proyecto o carpetas
//! visitadas) y el estado de los discos.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{Context, KeyDownEvent, Window};
use shieldexplorer_core::{
    Disk, MAX_FILES, display_path, index_files, list_disks, project_root, rank,
};

use super::{Explorer, Nav, apply_edit_key};
use crate::keys as k;
use crate::line_edit::LineEdit;

/// Cuántos resultados se muestran como máximo.
const RESULT_LIMIT: usize = 100;

pub struct Finder {
    /// `true`: carpetas visitadas; `false`: archivos del proyecto.
    pub folders: bool,
    pub query: LineEdit,
    /// Candidatos tal como se muestran (rutas relativas a `root`, o carpetas con `~`).
    pub items: Arc<Vec<String>>,
    pub root: PathBuf,
    /// Índices en `items`, del mejor al peor.
    pub results: Vec<usize>,
    pub selected: usize,
    pub indexing: bool,
    /// Rutas reales de las carpetas (solo en el modo carpetas).
    paths: Vec<PathBuf>,
    cancel: Arc<AtomicBool>,
    generation: u64,
}

pub struct DisksView {
    pub disks: Vec<Disk>,
    pub selected: usize,
    pub loading: bool,
}

pub enum Overlay {
    Finder(Finder),
    Disks(DisksView),
}

impl Explorer {
    // --- Abrir y cerrar --------------------------------------------------------------------

    pub fn open_finder(&mut self, folders: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.close_overlay(cx);
        let cancel = Arc::new(AtomicBool::new(false));
        let (root, items, paths) = if folders {
            let list: Vec<PathBuf> = self
                .memory
                .ranked(super::side::now_secs())
                .into_iter()
                .map(|(path, _)| path)
                .filter(|p| p.is_dir())
                .collect();
            let shown = list
                .iter()
                .map(|p| display_path(p, self.home.as_deref()))
                .collect();
            (self.cwd.clone(), shown, list)
        } else {
            (project_root(&self.cwd), Vec::new(), Vec::new())
        };
        self.overlay = Some(Overlay::Finder(Finder {
            folders,
            query: LineEdit::new(String::new()),
            items: Arc::new(items),
            root: root.clone(),
            results: Vec::new(),
            selected: 0,
            indexing: !folders,
            paths,
            cancel: Arc::clone(&cancel),
            generation: 0,
        }));
        window.focus(&self.overlay_focus);
        self.rank_finder(cx);
        if !folders {
            let flag = Arc::clone(&cancel);
            let indexing = cx
                .background_executor()
                .spawn(async move { index_files(&root, &flag, MAX_FILES) });
            cx.spawn(async move |this, cx| {
                let files = indexing.await;
                this.update(cx, |this, cx| {
                    let still_ours = matches!(&this.overlay, Some(Overlay::Finder(f)) if Arc::ptr_eq(&f.cancel, &cancel));
                    if still_ours && let Some(Overlay::Finder(f)) = this.overlay.as_mut() {
                        f.items = Arc::new(files);
                        f.indexing = false;
                        this.rank_finder(cx);
                    }
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    pub fn open_disks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_overlay(cx);
        self.overlay = Some(Overlay::Disks(DisksView {
            disks: Vec::new(),
            selected: 0,
            loading: true,
        }));
        window.focus(&self.overlay_focus);
        let listing = cx.background_executor().spawn(async move { list_disks() });
        cx.spawn(async move |this, cx| {
            let disks = listing.await;
            this.update(cx, |this, cx| {
                if let Some(Overlay::Disks(view)) = this.overlay.as_mut() {
                    view.disks = disks;
                    view.loading = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn close_overlay(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::Finder(finder)) = &self.overlay {
            finder.cancel.store(true, Ordering::Relaxed);
        }
        if self.overlay.take().is_some() {
            cx.notify();
        }
    }

    // --- Resultados del buscador -----------------------------------------------------------

    /// Reordena los candidatos para lo escrito, en segundo plano.
    fn rank_finder(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::Finder(finder)) = self.overlay.as_mut() else {
            return;
        };
        finder.generation += 1;
        let generation = finder.generation;
        let items = Arc::clone(&finder.items);
        let query = finder.query.text().to_owned();
        let ranking = cx
            .background_executor()
            .spawn(async move { rank(&items, &query, RESULT_LIMIT) });
        cx.spawn(async move |this, cx| {
            let results = ranking.await;
            this.update(cx, |this, cx| {
                if let Some(Overlay::Finder(f)) = this.overlay.as_mut()
                    && f.generation == generation
                {
                    f.results = results;
                    f.selected = 0;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // --- Teclado ---------------------------------------------------------------------------

    pub fn on_overlay_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match self.overlay.as_mut() {
            Some(Overlay::Finder(finder)) => {
                if apply_edit_key(&mut finder.query, event, cx) {
                    self.rank_finder(cx);
                    cx.notify();
                }
            }
            Some(Overlay::Disks(_)) => match event.keystroke.key_char.as_deref() {
                Some("j") => self.move_overlay(1, cx),
                Some("k") => self.move_overlay(-1, cx),
                _ => {}
            },
            None => {}
        }
    }

    fn move_overlay(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (selected, len) = match self.overlay.as_mut() {
            Some(Overlay::Finder(f)) => (&mut f.selected, f.results.len()),
            Some(Overlay::Disks(d)) => (&mut d.selected, d.disks.len()),
            None => return,
        };
        *selected = selected
            .saturating_add_signed(delta)
            .min(len.saturating_sub(1));
        cx.notify();
    }

    pub fn on_overlay_down(&mut self, _: &k::OverlayDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_overlay(1, cx);
    }

    pub fn on_overlay_up(&mut self, _: &k::OverlayUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_overlay(-1, cx);
    }

    pub fn on_overlay_cancel(
        &mut self,
        _: &k::OverlayCancel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_overlay(cx);
        window.focus(&self.focus);
    }

    pub fn on_overlay_confirm(
        &mut self,
        _: &k::OverlayConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(overlay) = self.overlay.take() else {
            return;
        };
        window.focus(&self.focus);
        match overlay {
            Overlay::Finder(finder) => {
                if let Some(&index) = finder.results.get(finder.selected) {
                    if finder.folders {
                        if let Some(path) = finder.paths.get(index).cloned() {
                            self.go_to(path, cx);
                        }
                    } else {
                        let file = finder.root.join(&finder.items[index]);
                        let name = file.file_name().map(std::ffi::OsString::from);
                        if let Some(dir) = file.parent() {
                            self.navigate(dir.to_path_buf(), name, Nav::Visit, cx);
                        }
                    }
                }
                finder.cancel.store(true, Ordering::Relaxed);
            }
            Overlay::Disks(view) => {
                if let Some(disk) = view.disks.get(view.selected) {
                    self.go_to(disk.mount.clone(), cx);
                }
            }
        }
        cx.notify();
    }

    pub fn on_finder_files(
        &mut self,
        _: &k::FinderFiles,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_finder(false, window, cx);
    }

    pub fn on_finder_folders(
        &mut self,
        _: &k::FinderFolders,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_finder(true, window, cx);
    }

    pub fn on_disks(&mut self, _: &k::Disks, window: &mut Window, cx: &mut Context<Self>) {
        self.open_disks(window, cx);
    }
}
