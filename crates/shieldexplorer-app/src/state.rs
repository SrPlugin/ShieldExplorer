//! Estado del explorador y su lógica de navegación.
//!
//! La lectura de carpetas ocurre en segundo plano. Cada navegación lleva un número (`generation`):
//! si el usuario navega de nuevo antes de que termine una carga, el resultado viejo se descarta.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui::{
    ClipboardItem, Context, FocusHandle, KeyDownEvent, ScrollStrategy, Subscription, Task,
    UniformListScrollHandle, Window,
};
use jiff::tz::TimeZone;
use shieldexplorer_core::{
    Entry, FsError, History, Listing, Sort, SortKey, display_path, read_dir, resolve, sort_entries,
};

use crate::keys::{
    CancelEdit, ConfirmPath, Dismiss, EditPath, First, GoBack, GoForward, Last, MoveDown, MoveUp,
    Open, Parent,
};
use crate::path_edit::PathEdit;

/// Si una carga tarda más que esto, la barra de estado avisa.
const SLOW_LOAD: Duration = Duration::from_millis(150);

/// Cómo afecta una navegación al historial.
#[derive(Clone, Copy)]
enum Nav {
    /// Sitio nuevo: se guarda de dónde se viene.
    Visit,
    Back,
    Forward,
    /// Misma carpeta (carga inicial): el historial no cambia.
    Replace,
}

pub struct LoadError {
    pub code: &'static str,
    pub message: String,
}

pub struct Explorer {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    pub entries: Vec<Entry>,
    pub unreadable: usize,
    pub sort: Sort,
    pub selected: usize,
    pub error: Option<LoadError>,
    pub slow: bool,
    pub editing: Option<PathEdit>,
    pub tz: TimeZone,
    pub focus: FocusHandle,
    pub path_focus: FocusHandle,
    pub scroll: UniformListScrollHandle,
    history: History,
    loading: bool,
    generation: u64,
    _load: Option<Task<()>>,
    _slow: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Explorer {
    pub fn new(
        start: PathBuf,
        select: Option<OsString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let path_focus = cx.focus_handle();
        window.focus(&focus);
        // Un clic en la lista quita el foco del campo de ruta: se cancela la edición. Cambiar de
        // ventana también lo quita, y entonces se conserva lo escrito.
        let blur = cx.on_blur(&path_focus, window, |this, window, cx| {
            if this.editing.is_some() && window.is_window_active() {
                this.editing = None;
                cx.notify();
            }
        });
        let mut explorer = Self {
            cwd: start.clone(),
            home: std::env::var_os("HOME").map(PathBuf::from),
            entries: Vec::new(),
            unreadable: 0,
            sort: Sort::default(),
            selected: 0,
            error: None,
            slow: false,
            editing: None,
            tz: TimeZone::system(),
            focus,
            path_focus,
            scroll: UniformListScrollHandle::new(),
            history: History::default(),
            loading: false,
            generation: 0,
            _load: None,
            _slow: None,
            _subscriptions: vec![blur],
        };
        explorer.navigate(start, select, Nav::Replace, cx);
        explorer
    }

    pub fn can_go_back(&self) -> bool {
        self.history.can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.history.can_go_forward()
    }

    pub fn format_modified(&self, time: Option<SystemTime>) -> String {
        time.and_then(|t| jiff::Timestamp::try_from(t).ok())
            .map(|ts| {
                ts.to_zoned(self.tz.clone())
                    .strftime("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "—".to_owned())
    }

    // --- Carga ---------------------------------------------------------------------------

    fn navigate(
        &mut self,
        target: PathBuf,
        select: Option<OsString>,
        nav: Nav,
        cx: &mut Context<Self>,
    ) {
        self.generation += 1;
        let generation = self.generation;
        self.loading = true;
        self.slow = false;

        let sort = self.sort;
        let path = target.clone();
        let read = cx.background_executor().spawn(async move {
            read_dir(&path).map(|mut listing| {
                sort_entries(&mut listing.entries, sort);
                listing
            })
        });
        self._load = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            this.update(cx, |this, cx| {
                this.finish_load(generation, target, select, nav, result, cx);
            })
            .ok();
        }));
        self._slow = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SLOW_LOAD).await;
            this.update(cx, |this, cx| {
                if this.generation == generation && this.loading {
                    this.slow = true;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn finish_load(
        &mut self,
        generation: u64,
        target: PathBuf,
        select: Option<OsString>,
        nav: Nav,
        result: Result<Listing, FsError>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation {
            return; // el usuario ya navegó a otro sitio
        }
        self.loading = false;
        self.slow = false;
        match result {
            Ok(listing) => {
                match nav {
                    Nav::Visit if target != self.cwd => self.history.visit(self.cwd.clone()),
                    Nav::Back => self.history.went_back(self.cwd.clone()),
                    Nav::Forward => self.history.went_forward(self.cwd.clone()),
                    Nav::Visit | Nav::Replace => {}
                }
                self.cwd = target;
                self.entries = listing.entries;
                self.unreadable = listing.unreadable;
                self.error = None;
                self.selected = select
                    .and_then(|name| self.position_of(&name))
                    .unwrap_or_default();
                self.scroll
                    .scroll_to_item(self.selected, ScrollStrategy::Top);
            }
            Err(error) => {
                // Se queda en la carpeta anterior y el error se muestra en la lista.
                let shown = display_path(&target, self.home.as_deref());
                self.error = Some(LoadError {
                    code: error.code(),
                    message: format!("{error}: {shown}"),
                });
            }
        }
        cx.notify();
    }

    fn position_of(&self, name: &std::ffi::OsStr) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    fn selected_name(&self) -> Option<OsString> {
        self.entries.get(self.selected).map(|e| e.name.clone())
    }

    // --- Selección y orden ---------------------------------------------------------------

    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.entries.len() {
            self.selected = index;
            self.scroll.scroll_to_item(index, ScrollStrategy::Top);
            cx.notify();
        }
    }

    fn move_by(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.error.is_some() || self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() - 1;
        let next = self.selected.saturating_add_signed(delta).min(last);
        self.select(next, cx);
    }

    pub fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        let name = self.selected_name();
        self.sort = self.sort.toggled(key);
        // ponytail: se ordena en el hilo de la interfaz; con cientos de miles de entradas habría
        // que moverlo a segundo plano como la carga.
        sort_entries(&mut self.entries, self.sort);
        self.selected = name.and_then(|n| self.position_of(&n)).unwrap_or_default();
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        cx.notify();
    }

    // --- Navegación ----------------------------------------------------------------------

    pub fn go_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate(path, None, Nav::Visit, cx);
    }

    pub fn open_index(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.get(index).filter(|e| e.is_dir()) {
            let target = self.cwd.join(&entry.name);
            self.navigate(target, None, Nav::Visit, cx);
        }
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        let Some(parent) = self.cwd.parent().map(Path::to_path_buf) else {
            return;
        };
        let from = self.cwd.file_name().map(OsString::from);
        self.navigate(parent, from, Nav::Visit, cx);
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(target) = self.history.peek_back().map(Path::to_path_buf) {
            self.navigate(target, None, Nav::Back, cx);
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(target) = self.history.peek_forward().map(Path::to_path_buf) {
            self.navigate(target, None, Nav::Forward, cx);
        }
    }

    pub fn go_parent(&mut self, cx: &mut Context<Self>) {
        self.go_up(cx);
    }

    // --- Acciones de teclado -------------------------------------------------------------

    pub fn on_move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by(1, cx);
    }

    pub fn on_move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by(-1, cx);
    }

    pub fn on_first(&mut self, _: &First, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_none() {
            self.select(0, cx);
        }
    }

    pub fn on_last(&mut self, _: &Last, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_none() && !self.entries.is_empty() {
            self.select(self.entries.len() - 1, cx);
        }
    }

    pub fn on_open(&mut self, _: &Open, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_none() {
            self.open_index(self.selected, cx);
        }
    }

    pub fn on_parent(&mut self, _: &Parent, _: &mut Window, cx: &mut Context<Self>) {
        self.go_up(cx);
    }

    pub fn on_go_back(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        self.go_back(cx);
    }

    pub fn on_go_forward(&mut self, _: &GoForward, _: &mut Window, cx: &mut Context<Self>) {
        self.go_forward(cx);
    }

    pub fn on_dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.take().is_some() {
            cx.notify();
        }
    }

    // --- Edición de la ruta --------------------------------------------------------------

    pub fn begin_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = display_path(&self.cwd, self.home.as_deref());
        self.editing = Some(PathEdit::new(text));
        window.focus(&self.path_focus);
        cx.notify();
    }

    pub fn on_edit_path(&mut self, _: &EditPath, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_edit(window, cx);
    }

    pub fn on_confirm_path(
        &mut self,
        _: &ConfirmPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        window.focus(&self.focus);
        let target = resolve(edit.text(), &self.cwd, self.home.as_deref());
        self.navigate(target, None, Nav::Visit, cx);
    }

    pub fn on_cancel_edit(&mut self, _: &CancelEdit, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = None;
        window.focus(&self.focus);
        cx.notify();
    }

    /// Teclas mientras se edita la ruta. `Enter` y `Esc` llegan como acciones.
    pub fn on_path_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        let key = &event.keystroke;
        let mods = &key.modifiers;
        if mods.control && key.key == "v" {
            if let Some(text) = cx
                .read_from_clipboard()
                .and_then(|item: ClipboardItem| item.text())
            {
                edit.insert(&text);
            }
        } else if mods.control || mods.alt || mods.platform {
            return;
        } else {
            match key.key.as_str() {
                "backspace" => edit.backspace(),
                "delete" => edit.delete(),
                "left" => edit.left(),
                "right" => edit.right(),
                "home" => edit.home(),
                "end" => edit.end(),
                _ => match &key.key_char {
                    Some(text) => edit.insert(text),
                    None => return,
                },
            }
        }
        cx.notify();
    }
}
