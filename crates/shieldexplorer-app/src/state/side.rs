//! Lo que se calcula al margen de la lista: vista previa, estado de git, tamaño de carpetas,
//! refresco automático cuando cambia el disco y la memoria de marcas y carpetas visitadas.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use gpui::{Context, Window};
use notify::{RecursiveMode, Watcher};
use shieldexplorer_core::{Kind, dir_size, load_preview, read_git};

use super::Explorer;
use crate::config;
use crate::keys as k;

/// Al mover el cursor rápido no se carga la vista previa de cada fila por la que se pasa.
const PREVIEW_DELAY: Duration = Duration::from_millis(60);
/// Los eventos del disco llegan a ráfagas: se espera a que pare antes de recargar.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(250);

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Explorer {
    // --- Vista previa ----------------------------------------------------------------------

    pub(super) fn request_preview(&mut self, cx: &mut Context<Self>) {
        if !self.preview_on {
            return;
        }
        let Some(path) = self.selected_path().filter(|_| self.error.is_none()) else {
            self.preview = None;
            return;
        };
        self.preview_gen += 1;
        let generation = self.preview_gen;
        self.tasks.preview = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PREVIEW_DELAY).await;
            if this
                .update(cx, |this, _| this.preview_gen != generation)
                .unwrap_or(true)
            {
                return;
            }
            let target = path.clone();
            let loaded = cx
                .background_executor()
                .spawn(async move { load_preview(&target) })
                .await;
            this.update(cx, |this, cx| {
                if this.preview_gen == generation {
                    this.preview = Some((path, loaded));
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    pub fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        self.preview_on = !self.preview_on;
        config::save_preview(self.preview_on);
        if self.preview_on {
            self.request_preview(cx);
        } else {
            self.preview = None;
        }
        cx.notify();
    }

    pub fn on_toggle_preview(
        &mut self,
        _: &k::TogglePreview,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_preview(cx);
    }

    // --- Git -------------------------------------------------------------------------------

    pub(super) fn request_git(&mut self, cx: &mut Context<Self>) {
        self.git_gen += 1;
        let generation = self.git_gen;
        let cwd = self.cwd.clone();
        let read = cx
            .background_executor()
            .spawn(async move { read_git(&cwd) });
        self.tasks.git = Some(cx.spawn(async move |this, cx| {
            let info = read.await;
            this.update(cx, |this, cx| {
                if this.git_gen == generation {
                    this.git = info;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    // --- Tamaño de carpetas ----------------------------------------------------------------

    /// Calcula el tamaño real de cada carpeta de la lista, en paralelo y cancelable.
    pub fn compute_sizes(&mut self, cx: &mut Context<Self>) {
        self.count = None;
        self.du_cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.du_cancel = Arc::clone(&cancel);
        self.du_gen += 1;
        let generation = self.du_gen;
        let folders: Vec<(OsString, PathBuf)> = self
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Dir)
            .map(|e| (e.name.clone(), self.cwd.join(&e.name)))
            .collect();
        if folders.is_empty() {
            self.show_notice("no hay carpetas en esta ubicación", true, cx);
            return;
        }
        self.sizing = true;
        self.du_pending = folders.len();
        for (name, path) in folders {
            let flag = Arc::clone(&cancel);
            let size = cx
                .background_executor()
                .spawn(async move { dir_size(&path, &flag) });
            cx.spawn(async move |this, cx| {
                let size = size.await;
                this.update(cx, |this, cx| {
                    if this.du_gen != generation {
                        return;
                    }
                    if let Some(size) = size {
                        this.dir_sizes.insert(name, size);
                    }
                    this.du_pending = this.du_pending.saturating_sub(1);
                    this.sizing = this.du_pending > 0;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    pub fn on_compute_sizes(
        &mut self,
        _: &k::ComputeSizes,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.compute_sizes(cx);
    }

    // --- Refresco automático ---------------------------------------------------------------

    /// Vigila la carpeta actual y la recarga cuando algo cambia en el disco.
    pub(super) fn watch_cwd(&mut self, cx: &mut Context<Self>) {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = tx.unbounded_send(());
            }
        });
        let Ok(mut watcher) = watcher else {
            self.watcher = None;
            self.tasks.watch = None;
            return;
        };
        if watcher
            .watch(&self.cwd, RecursiveMode::NonRecursive)
            .is_err()
        {
            self.watcher = None;
            self.tasks.watch = None;
            return;
        }
        self.watcher = Some(watcher);
        let cwd = self.cwd.clone();
        self.tasks.watch = Some(cx.spawn(async move |this, cx| {
            while rx.next().await.is_some() {
                cx.background_executor().timer(WATCH_DEBOUNCE).await;
                while let Ok(Some(())) = rx.try_next() {}
                let alive = this
                    .update(cx, |this, cx| {
                        if this.cwd == cwd && !this.loading {
                            this.refresh(None, cx);
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        }));
    }

    // --- Memoria: marcas y carpetas visitadas ----------------------------------------------

    pub(super) fn record_visit(&mut self, cx: &mut Context<Self>) {
        self.memory.visit(&self.cwd, now_secs());
        self.save_memory(cx);
    }

    pub(super) fn save_memory(&self, cx: &mut Context<Self>) {
        let Some(path) = self.memory_path.clone() else {
            return;
        };
        let memory = self.memory.clone();
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = memory.save(&path) {
                    eprintln!("shieldexplorer: no se pudo guardar la memoria: {error}");
                }
            })
            .detach();
    }
}
