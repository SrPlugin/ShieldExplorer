//! Operaciones sobre archivos: selección múltiple, copiar, cortar, pegar, papelera, renombrar,
//! crear, deshacer y copiar rutas. Todo el trabajo con el disco ocurre en segundo plano.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use gpui::{ClipboardItem, Context, Window};
use shieldexplorer_core::{
    OpError, Undo, YankKind, create_dir, create_file, paste_copy, paste_move, project_root, rename,
    trash, undo,
};

use super::Explorer;
use crate::dirs;
use crate::keys as k;

/// Lo copiado o cortado, a la espera de pegarse.
#[derive(Clone)]
pub struct Clip {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

/// Mensaje temporal de la barra de estado.
pub struct Notice {
    pub text: String,
    pub error: bool,
}

const NOTICE_TIME: Duration = Duration::from_secs(5);
const MAX_UNDO: usize = 50;

struct OpDone {
    undo: Option<Undo>,
    message: String,
    select: Option<OsString>,
}

type OpResult = Result<OpDone, OpError>;

/// Fecha y hora local para el `.trashinfo`.
fn local_date() -> String {
    jiff::Zoned::now().strftime("%Y-%m-%dT%H:%M:%S").to_string()
}

fn file_name(path: &std::path::Path) -> Option<OsString> {
    path.file_name().map(OsString::from)
}

impl Explorer {
    // --- Avisos ----------------------------------------------------------------------------

    pub fn show_notice(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.notice_id += 1;
        let id = self.notice_id;
        self.notice = Some(Notice {
            text: text.into(),
            error,
        });
        self.tasks.notice = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_TIME).await;
            this.update(cx, |this, cx| {
                if this.notice_id == id {
                    this.notice = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    // --- Ejecutar en segundo plano ---------------------------------------------------------

    fn spawn_op(
        &mut self,
        work: impl FnOnce() -> OpResult + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let task = cx.background_executor().spawn(async move { work() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| this.finish_op(result, cx)).ok();
        })
        .detach();
    }

    fn finish_op(&mut self, result: OpResult, cx: &mut Context<Self>) {
        match result {
            Ok(done) => {
                if let Some(undo) = done.undo {
                    self.undo_stack.push(undo);
                    if self.undo_stack.len() > MAX_UNDO {
                        self.undo_stack.remove(0);
                    }
                }
                self.show_notice(done.message, false, cx);
                self.refresh(done.select, cx);
            }
            Err(error) => {
                self.show_notice(error.to_string(), true, cx);
                self.refresh(None, cx);
            }
        }
    }

    // --- Selección múltiple ----------------------------------------------------------------

    /// Sobre qué actúan las operaciones: lo marcado o, si no hay marcas, la fila actual.
    pub fn target_paths(&self) -> Vec<PathBuf> {
        if self.marked.is_empty() {
            return self.selected_path().into_iter().collect();
        }
        self.entries
            .iter()
            .filter(|e| self.marked.contains(&e.name))
            .map(|e| self.cwd.join(&e.name))
            .collect()
    }

    fn clear_selection(&mut self) {
        self.marked.clear();
        self.visual = None;
    }

    /// En selección visual, marca todo entre donde empezó y el cursor.
    pub fn sync_visual(&mut self) {
        let Some(anchor) = self.visual else {
            return;
        };
        let last = self.entries.len().saturating_sub(1);
        let (lo, hi) = (
            anchor.min(self.selected).min(last),
            anchor.max(self.selected).min(last),
        );
        if let Some(range) = self.entries.get(lo..=hi) {
            self.marked = range.iter().map(|e| e.name.clone()).collect();
        }
    }

    pub fn on_visual_toggle(
        &mut self,
        _: &k::VisualToggle,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.count = None;
        if self.error.is_some() || self.entries.is_empty() {
            return;
        }
        if self.visual.take().is_none() {
            self.visual = Some(self.selected);
            self.sync_visual();
        }
        cx.notify();
    }

    pub fn on_toggle_mark(&mut self, _: &k::ToggleMark, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        let Some(name) = self.selected_name().filter(|_| self.error.is_none()) else {
            return;
        };
        if !self.marked.remove(&name) {
            self.marked.insert(name);
        }
        self.visual = None;
        self.move_by(1, cx);
        cx.notify();
    }

    pub fn on_select_all(&mut self, _: &k::SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_some() {
            return;
        }
        self.marked = self.entries.iter().map(|e| e.name.clone()).collect();
        self.visual = None;
        cx.notify();
    }

    // --- Copiar, cortar y pegar ------------------------------------------------------------

    fn put_in_clipboard(&mut self, cut: bool, cx: &mut Context<Self>) {
        self.count = None;
        let paths = self.target_paths();
        if paths.is_empty() || self.error.is_some() {
            return;
        }
        let verb = if cut { "cortado(s)" } else { "copiado(s)" };
        self.show_notice(format!("{} {verb}", paths.len()), false, cx);
        self.clipboard = Some(Clip { paths, cut });
        self.clear_selection();
    }

    pub fn on_copy(&mut self, _: &k::Copy, _: &mut Window, cx: &mut Context<Self>) {
        self.put_in_clipboard(false, cx);
    }

    pub fn on_cut(&mut self, _: &k::Cut, _: &mut Window, cx: &mut Context<Self>) {
        self.put_in_clipboard(true, cx);
    }

    pub fn on_paste(&mut self, _: &k::Paste, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        let Some(clip) = self.clipboard.clone() else {
            self.show_notice("no hay nada copiado", true, cx);
            return;
        };
        let dest = self.cwd.clone();
        if clip.cut {
            self.clipboard = None; // lo cortado se pega una sola vez
        }
        self.spawn_op(
            move || {
                if clip.cut {
                    let moved = paste_move(&clip.paths, &dest)?;
                    let select = moved.first().and_then(|(_, to)| file_name(to));
                    let message = format!("{} movido(s)", moved.len());
                    let undo = (!moved.is_empty()).then_some(Undo::Moved(moved));
                    Ok(OpDone {
                        undo,
                        message,
                        select,
                    })
                } else {
                    let created = paste_copy(&clip.paths, &dest)?;
                    let select = created.first().and_then(|p| file_name(p));
                    let message = format!("{} copiado(s) aquí", created.len());
                    let undo = (!created.is_empty()).then_some(Undo::Created(created));
                    Ok(OpDone {
                        undo,
                        message,
                        select,
                    })
                }
            },
            cx,
        );
    }

    // --- Papelera, renombrar, crear y deshacer ---------------------------------------------

    pub fn delete_targets(&mut self, cx: &mut Context<Self>) {
        self.count = None;
        let targets = self.target_paths();
        if targets.is_empty() || self.error.is_some() {
            return;
        }
        let Some(root) = dirs::trash_root() else {
            self.show_notice("no se encontró la carpeta personal (HOME)", true, cx);
            return;
        };
        self.clear_selection();
        self.spawn_op(
            move || {
                let date = local_date();
                let mut items = Vec::new();
                let mut failed = 0;
                let mut first_error = None;
                for path in &targets {
                    match trash(path, &root, &date) {
                        Ok(item) => items.push(item),
                        Err(error) => {
                            failed += 1;
                            first_error.get_or_insert(error);
                        }
                    }
                }
                if items.is_empty() {
                    return Err(first_error.unwrap_or(OpError::Invalid("nada que borrar")));
                }
                let extra = if failed > 0 {
                    format!(", {failed} fallaron")
                } else {
                    String::new()
                };
                Ok(OpDone {
                    message: format!(
                        "{} a la papelera{extra} (deshacer: u / Ctrl+Z)",
                        items.len()
                    ),
                    undo: Some(Undo::Trashed(items)),
                    select: None,
                })
            },
            cx,
        );
    }

    pub fn on_delete(&mut self, _: &k::Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_targets(cx);
    }

    pub fn do_rename(&mut self, new_name: String, cx: &mut Context<Self>) {
        let Some(from) = self.selected_path() else {
            return;
        };
        self.spawn_op(
            move || {
                let to = rename(&from, &new_name)?;
                let select = file_name(&to);
                if to == from {
                    return Ok(OpDone {
                        undo: None,
                        message: "sin cambios".into(),
                        select,
                    });
                }
                Ok(OpDone {
                    message: format!("renombrado a {}", new_name.trim()),
                    undo: Some(Undo::Renamed { from, to }),
                    select,
                })
            },
            cx,
        );
    }

    pub fn do_create(&mut self, name: String, dir: bool, cx: &mut Context<Self>) {
        let cwd = self.cwd.clone();
        self.spawn_op(
            move || {
                let created = if dir {
                    create_dir(&cwd, &name)?
                } else {
                    create_file(&cwd, &name)?
                };
                // Con `a/b/c` se selecciona el primer nivel, que es lo que aparece en esta carpeta.
                let select = created
                    .strip_prefix(&cwd)
                    .ok()
                    .and_then(|rel| rel.components().next())
                    .map(|c| c.as_os_str().to_owned());
                let what = if dir { "carpeta" } else { "archivo" };
                Ok(OpDone {
                    message: format!("{what} creado: {}", name.trim()),
                    undo: Some(Undo::Created(vec![created])),
                    select,
                })
            },
            cx,
        );
    }

    pub fn undo_last(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.undo_stack.pop() else {
            self.show_notice("no hay nada que deshacer", true, cx);
            return;
        };
        let Some(root) = dirs::trash_root() else {
            self.show_notice("no se encontró la carpeta personal (HOME)", true, cx);
            return;
        };
        self.spawn_op(
            move || {
                let message = undo(&action, &root, &local_date())?;
                Ok(OpDone {
                    undo: None,
                    message,
                    select: None,
                })
            },
            cx,
        );
    }

    pub fn on_undo(&mut self, _: &k::Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.undo_last(cx);
    }

    // --- Copiar rutas al portapapeles del sistema ------------------------------------------

    pub fn yank(&mut self, kind: YankKind, cx: &mut Context<Self>) {
        self.count = None;
        let targets = self.target_paths();
        if targets.is_empty() || self.error.is_some() {
            return;
        }
        // Relativa: a la raíz del proyecto git o, si no hay, al directorio personal.
        let root = project_root(&self.cwd);
        let base = if root.join(".git").exists() {
            Some(root)
        } else {
            self.home.clone()
        };
        let text = targets
            .iter()
            .map(|p| match kind {
                YankKind::Path => p.display().to_string(),
                YankKind::Name => p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                YankKind::Relative => {
                    let rel = base
                        .as_deref()
                        .and_then(|b| p.strip_prefix(b).ok())
                        .unwrap_or(p);
                    rel.display().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let shown: String = text.chars().take(60).collect();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_notice(format!("copiado al portapapeles: {shown}"), false, cx);
    }

    pub fn on_yank_path(&mut self, _: &k::YankPath, _: &mut Window, cx: &mut Context<Self>) {
        self.yank(YankKind::Path, cx);
    }

    pub fn on_yank_name(&mut self, _: &k::YankName, _: &mut Window, cx: &mut Context<Self>) {
        self.yank(YankKind::Name, cx);
    }

    pub fn on_yank_rel(&mut self, _: &k::YankRel, _: &mut Window, cx: &mut Context<Self>) {
        self.yank(YankKind::Relative, cx);
    }

    // --- Papelera como carpeta -------------------------------------------------------------

    pub fn go_trash(&mut self, cx: &mut Context<Self>) {
        match dirs::trash_root()
            .map(|r| r.join("files"))
            .filter(|p| p.is_dir())
        {
            Some(files) => self.go_to(files, cx),
            None => self.show_notice("la papelera está vacía o no existe todavía", true, cx),
        }
    }
}
