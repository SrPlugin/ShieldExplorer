//! Estado del explorador y su lógica de navegación.
//!
//! La lectura de carpetas ocurre en segundo plano. Cada navegación lleva un número (`generation`):
//! si el usuario navega de nuevo antes de que termine una carga, el resultado viejo se descarta.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use gpui::{
    Context, FocusHandle, KeyDownEvent, ScrollStrategy, Subscription, Task,
    UniformListScrollHandle, Window,
};
use jiff::tz::TimeZone;
use shieldexplorer_core::{
    Entry, FsError, History, Listing, Sort, SortKey, display_path, find_match, find_prefix,
    read_dir, resolve, sort_entries,
};

use crate::config::{self, KeyMode};
use crate::keys::{
    CancelEdit, CancelSearch, ConfirmPath, ConfirmSearch, Dismiss, EditPath, First, GoBack,
    GoForward, HalfDown, HalfUp, Last, MoveDown, MoveUp, Open, PageDown, PageUp, Parent, Reload,
    SearchNext, SearchPrev, StartSearch, ToggleHidden, ToggleMode,
};
use crate::line_edit::LineEdit;

/// Si una carga tarda más que esto, la barra de estado avisa.
const SLOW_LOAD: Duration = Duration::from_millis(150);
/// Tras este tiempo sin escribir, el "escribir para saltar" empieza de cero.
const TYPEAHEAD_RESET: Duration = Duration::from_secs(1);
/// Tope del contador (`5j`): evita desbordes con un `9999999999j`.
const MAX_COUNT: usize = 999_999;

/// Cómo afecta una navegación al historial.
#[derive(Clone, Copy)]
enum Nav {
    /// Sitio nuevo: se guarda de dónde se viene.
    Visit,
    Back,
    Forward,
    /// Misma carpeta (carga inicial o recarga): el historial no cambia.
    Replace,
}

pub struct LoadError {
    pub code: &'static str,
    pub message: String,
}

/// Búsqueda con `/` en curso: el texto y la fila donde estaba el cursor al empezar.
pub struct Search {
    pub edit: LineEdit,
    origin: usize,
}

pub struct Explorer {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    /// Lo que se ve: `all` sin los ocultos si así se pidió.
    pub entries: Vec<Entry>,
    pub unreadable: usize,
    pub sort: Sort,
    pub selected: usize,
    pub error: Option<LoadError>,
    pub slow: bool,
    pub editing: Option<LineEdit>,
    pub search: Option<Search>,
    pub mode: KeyMode,
    /// Número tecleado en modo vim a la espera del movimiento (`5` de `5j`).
    pub count: Option<usize>,
    pub show_hidden: bool,
    pub tz: TimeZone,
    pub focus: FocusHandle,
    pub path_focus: FocusHandle,
    pub search_focus: FocusHandle,
    pub scroll: UniformListScrollHandle,
    /// Filas que caben en la lista; lo actualiza el dibujado y sirve para `PageDown`/`Ctrl+D`.
    pub page_rows: usize,
    all: Vec<Entry>,
    history: History,
    last_search: Option<String>,
    typeahead: String,
    typeahead_at: Instant,
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
        mode: KeyMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let path_focus = cx.focus_handle();
        let search_focus = cx.focus_handle();
        window.focus(&focus);
        // Un clic en la lista quita el foco de los campos de texto: se cancela la edición. Cambiar
        // de ventana también lo quita, y entonces se conserva lo escrito.
        let path_blur = cx.on_blur(&path_focus, window, |this, window, cx| {
            if this.editing.is_some() && window.is_window_active() {
                this.editing = None;
                cx.notify();
            }
        });
        let search_blur = cx.on_blur(&search_focus, window, |this, window, cx| {
            if this.search.is_some() && window.is_window_active() {
                this.cancel_search(cx);
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
            search: None,
            mode,
            count: None,
            show_hidden: true,
            tz: TimeZone::system(),
            focus,
            path_focus,
            search_focus,
            scroll: UniformListScrollHandle::new(),
            page_rows: 20,
            all: Vec::new(),
            history: History::default(),
            last_search: None,
            typeahead: String::new(),
            typeahead_at: Instant::now(),
            loading: false,
            generation: 0,
            _load: None,
            _slow: None,
            _subscriptions: vec![path_blur, search_blur],
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
                self.all = listing.entries;
                self.unreadable = listing.unreadable;
                self.error = None;
                self.search = None;
                self.count = None;
                self.apply_view();
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

    /// Recalcula lo visible a partir de `all` según se muestren o no los ocultos.
    fn apply_view(&mut self) {
        self.entries = if self.show_hidden {
            self.all.clone()
        } else {
            self.all
                .iter()
                .filter(|e| !e.is_hidden())
                .cloned()
                .collect()
        };
    }

    fn position_of(&self, name: &OsStr) -> Option<usize> {
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
        sort_entries(&mut self.all, self.sort);
        self.apply_view();
        self.selected = name.and_then(|n| self.position_of(&n)).unwrap_or_default();
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        cx.notify();
    }

    pub fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        let name = self.selected_name();
        let previous = self.selected;
        self.show_hidden = !self.show_hidden;
        self.apply_view();
        self.selected = name
            .and_then(|n| self.position_of(&n))
            .unwrap_or_else(|| previous.min(self.entries.len().saturating_sub(1)));
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

    pub fn go_parent(&mut self, cx: &mut Context<Self>) {
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

    // --- Modo de teclado -----------------------------------------------------------------

    pub fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        self.mode = self.mode.toggled();
        self.count = None;
        self.typeahead.clear();
        config::save_mode(self.mode);
        cx.notify();
    }

    /// Repeticiones pedidas con un número antes del movimiento; 1 si no hubo.
    fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    /// Teclas sin atajo en la lista: dígitos del contador en modo vim, y en modo explorador las
    /// letras para saltar a la entrada que empieza así.
    pub fn on_list_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let mods = &key.modifiers;
        if mods.control || mods.alt || mods.platform || self.error.is_some() {
            return;
        }
        let Some(text) = key.key_char.as_deref() else {
            return;
        };
        match self.mode {
            KeyMode::Vim => {
                let digit = text.parse::<u8>().ok().filter(|_| text.len() == 1);
                // El 0 solo cuenta si ya hay un número delante (`10j`); solo, no hace nada.
                if let Some(d) = digit.filter(|&d| d != 0 || self.count.is_some()) {
                    let next = self
                        .count
                        .unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add(d.into());
                    self.count = Some(next.min(MAX_COUNT));
                    cx.notify();
                }
            }
            KeyMode::Explorer => self.type_ahead(text, cx),
        }
    }

    fn type_ahead(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.chars().any(char::is_control) {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.typeahead_at) > TYPEAHEAD_RESET {
            self.typeahead.clear();
        }
        self.typeahead_at = now;
        self.typeahead.push_str(text);
        // Una sola letra repetida avanza a la siguiente que empiece igual; si se sigue
        // escribiendo, se afina desde la fila actual.
        let from = if self.typeahead.chars().count() == 1 {
            self.selected + 1
        } else {
            self.selected
        };
        if let Some(index) = find_prefix(&self.entries, &self.typeahead, from) {
            self.select(index, cx);
        }
    }

    // --- Acciones de teclado -------------------------------------------------------------

    pub fn on_move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.take_count();
        self.move_by(n.try_into().unwrap_or(isize::MAX), cx);
    }

    pub fn on_move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.take_count();
        self.move_by(-isize::try_from(n).unwrap_or(isize::MAX), cx);
    }

    pub fn on_page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by(self.page_rows as isize, cx);
    }

    pub fn on_page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by(-(self.page_rows as isize), cx);
    }

    pub fn on_half_down(&mut self, _: &HalfDown, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by((self.page_rows / 2).max(1) as isize, cx);
    }

    pub fn on_half_up(&mut self, _: &HalfUp, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.move_by(-((self.page_rows / 2).max(1) as isize), cx);
    }

    /// `gg` va a la primera fila; con número (`5gg`), a esa fila.
    pub fn on_first(&mut self, _: &First, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_some() {
            return;
        }
        let line = self.count.take().map_or(0, |c| c.saturating_sub(1));
        self.select(line.min(self.entries.len().saturating_sub(1)), cx);
    }

    /// `G` va a la última fila; con número (`5G`), a esa fila.
    pub fn on_last(&mut self, _: &Last, _: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_some() || self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() - 1;
        let line = self.count.take().map_or(last, |c| c.saturating_sub(1));
        self.select(line.min(last), cx);
    }

    pub fn on_open(&mut self, _: &Open, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        if self.error.is_none() {
            self.open_index(self.selected, cx);
        }
    }

    pub fn on_parent(&mut self, _: &Parent, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.go_parent(cx);
    }

    pub fn on_go_back(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        self.go_back(cx);
    }

    pub fn on_go_forward(&mut self, _: &GoForward, _: &mut Window, cx: &mut Context<Self>) {
        self.go_forward(cx);
    }

    /// `Esc`: borra el contador pendiente o, si no hay, cierra el mensaje de error.
    pub fn on_dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.count.take().is_some() || self.error.take().is_some() {
            cx.notify();
        }
    }

    pub fn on_toggle_hidden(&mut self, _: &ToggleHidden, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_hidden(cx);
    }

    pub fn on_reload(&mut self, _: &Reload, _: &mut Window, cx: &mut Context<Self>) {
        let name = self.selected_name();
        self.navigate(self.cwd.clone(), name, Nav::Replace, cx);
    }

    pub fn on_toggle_mode(&mut self, _: &ToggleMode, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mode(cx);
    }

    // --- Búsqueda (`/`, `n`, `N`) --------------------------------------------------------

    pub fn on_start_search(
        &mut self,
        _: &StartSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.count = None;
        if self.error.is_some() || self.entries.is_empty() {
            return;
        }
        self.search = Some(Search {
            edit: LineEdit::new(String::new()),
            origin: self.selected,
        });
        window.focus(&self.search_focus);
        cx.notify();
    }

    /// Teclas mientras se escribe la búsqueda: salta a la coincidencia a cada letra.
    pub fn on_search_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        if !apply_edit_key(&mut search.edit, event, cx) {
            return;
        }
        let (text, origin) = (search.edit.text().to_owned(), search.origin);
        let target = find_match(&self.entries, &text, origin, true).unwrap_or(origin);
        self.select(target, cx);
        cx.notify();
    }

    pub fn on_confirm_search(
        &mut self,
        _: &ConfirmSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(search) = self.search.take()
            && !search.edit.text().is_empty()
        {
            self.last_search = Some(search.edit.text().to_owned());
        }
        window.focus(&self.focus);
        cx.notify();
    }

    pub fn on_cancel_search(
        &mut self,
        _: &CancelSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_search(cx);
        window.focus(&self.focus);
    }

    /// Cancela la búsqueda y devuelve el cursor a donde estaba.
    fn cancel_search(&mut self, cx: &mut Context<Self>) {
        if let Some(search) = self.search.take() {
            self.select(search.origin, cx);
        }
        cx.notify();
    }

    pub fn on_search_next(&mut self, _: &SearchNext, _: &mut Window, cx: &mut Context<Self>) {
        self.repeat_search(true, cx);
    }

    pub fn on_search_prev(&mut self, _: &SearchPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.repeat_search(false, cx);
    }

    /// `n` / `N`: siguiente o anterior coincidencia de la última búsqueda, `count` veces.
    fn repeat_search(&mut self, forward: bool, cx: &mut Context<Self>) {
        let times = self.take_count();
        let (Some(pattern), false) = (self.last_search.clone(), self.entries.is_empty()) else {
            return;
        };
        let len = self.entries.len();
        let mut at = self.selected;
        for _ in 0..times {
            let from = if forward {
                (at + 1) % len
            } else {
                (at + len - 1) % len
            };
            match find_match(&self.entries, &pattern, from, forward) {
                Some(found) => at = found,
                None => break,
            }
        }
        self.select(at, cx);
    }

    // --- Edición de la ruta --------------------------------------------------------------

    pub fn begin_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        let text = display_path(&self.cwd, self.home.as_deref());
        self.editing = Some(LineEdit::new(text));
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
        if apply_edit_key(edit, event, cx) {
            cx.notify();
        }
    }
}

/// Aplica una tecla a un campo de una línea. Devuelve `true` si algo cambió.
fn apply_edit_key(edit: &mut LineEdit, event: &KeyDownEvent, cx: &mut Context<Explorer>) -> bool {
    let key = &event.keystroke;
    let mods = &key.modifiers;
    if mods.control && key.key == "v" {
        return match cx.read_from_clipboard().and_then(|item| item.text()) {
            Some(text) => {
                edit.insert(&text);
                true
            }
            None => false,
        };
    }
    if mods.control || mods.alt || mods.platform {
        return false;
    }
    match key.key.as_str() {
        "backspace" => edit.backspace(),
        "delete" => edit.delete(),
        "left" => edit.left(),
        "right" => edit.right(),
        "home" => edit.home(),
        "end" => edit.end(),
        _ => match &key.key_char {
            Some(text) => edit.insert(text),
            None => return false,
        },
    }
    true
}
