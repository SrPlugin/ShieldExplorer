//! Estado del explorador y su lógica.
//!
//! La lectura de carpetas, la vista previa, git, los tamaños y las operaciones de archivos ocurren
//! en segundo plano. Cada carga lleva un número (`generation`): si el usuario navega de nuevo
//! antes de que termine una, el resultado viejo se descarta.

mod actions;
mod fileops;
mod launch;
mod overlay;
mod prompt;
mod side;
mod tabs;

use std::collections::{BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime};

use gpui::{
    Context, FocusHandle, KeyDownEvent, ScrollStrategy, Subscription, Task,
    UniformListScrollHandle, Window,
};
use jiff::tz::TimeZone;
use notify::RecommendedWatcher;
use shieldexplorer_core::{
    Entry, FsError, GitInfo, History, Listing, Memory, Preview, Sort, SortKey, Undo, display_path,
    find_prefix, read_dir, sort_entries,
};

use crate::config::{self, Columns, Config, KeyMode};
use crate::dirs;
use crate::line_edit::LineEdit;

pub use fileops::{Clip, Notice};
pub use launch::Output;
pub use overlay::{DisksView, Finder, Overlay};
pub use prompt::{Prompt, PromptKind};
pub use tabs::TabSnap;

/// Si una carga tarda más que esto, la barra de estado avisa.
const SLOW_LOAD: Duration = Duration::from_millis(150);
/// Tras este tiempo sin escribir, el "escribir para saltar" empieza de cero.
const TYPEAHEAD_RESET: Duration = Duration::from_secs(1);
/// Tope del contador (`5j`): evita desbordes con un `9999999999j`.
const MAX_COUNT: usize = 999_999;

/// Cómo afecta una navegación al historial y a lo que se conserva.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    /// Sitio nuevo: se guarda de dónde se viene.
    Visit,
    Back,
    Forward,
    /// Carga inicial o cambio de pestaña: el historial no cambia.
    Replace,
    /// Misma carpeta recargada (cambió el disco o terminó una operación): se conserva la selección
    /// múltiple, el cursor y lo que se esté escribiendo.
    Refresh,
}

pub struct LoadError {
    pub code: &'static str,
    pub message: String,
}

/// Tareas en segundo plano. Soltar una la cancela, así que cada una vive aquí hasta que se
/// reemplaza por otra.
#[derive(Default)]
struct Tasks {
    load: Option<Task<()>>,
    slow: Option<Task<()>>,
    preview: Option<Task<()>>,
    git: Option<Task<()>>,
    notice: Option<Task<()>>,
    watch: Option<Task<()>>,
    shell: Option<Task<()>>,
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
    pub show_hidden: bool,
    pub columns: Columns,
    pub tz: TimeZone,

    pub mode: KeyMode,
    /// Número tecleado en modo vim a la espera del movimiento (`5` de `5j`).
    pub count: Option<usize>,
    pub editing: Option<LineEdit>,
    pub prompt: Option<Prompt>,

    /// Nombres marcados en la carpeta actual (selección múltiple).
    pub marked: BTreeSet<OsString>,
    /// Fila donde empezó la selección visual.
    pub visual: Option<usize>,
    pub clipboard: Option<Clip>,
    pub notice: Option<Notice>,

    pub git: Option<GitInfo>,
    pub preview_on: bool,
    pub preview: Option<(PathBuf, Preview)>,
    pub dir_sizes: HashMap<OsString, u64>,
    pub sizing: bool,
    pub overlay: Option<Overlay>,
    pub output: Option<Output>,
    pub tabs: Vec<TabSnap>,
    pub active_tab: usize,

    pub focus: FocusHandle,
    pub path_focus: FocusHandle,
    pub prompt_focus: FocusHandle,
    pub overlay_focus: FocusHandle,
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
    undo_stack: Vec<Undo>,
    memory: Memory,
    memory_path: Option<PathBuf>,
    notice_id: u64,
    preview_gen: u64,
    git_gen: u64,
    du_gen: u64,
    du_pending: usize,
    du_cancel: Arc<AtomicBool>,
    tasks: Tasks,
    watcher: Option<RecommendedWatcher>,
    _subscriptions: Vec<Subscription>,
}

impl Explorer {
    pub fn new(
        start: PathBuf,
        select: Option<OsString>,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let path_focus = cx.focus_handle();
        let prompt_focus = cx.focus_handle();
        let overlay_focus = cx.focus_handle();
        window.focus(&focus);
        // Un clic en la lista quita el foco de los campos de texto: se cancela la edición. Cambiar
        // de ventana también lo quita, y entonces se conserva lo escrito.
        let path_blur = cx.on_blur(&path_focus, window, |this, window, cx| {
            if this.editing.is_some() && window.is_window_active() {
                this.editing = None;
                cx.notify();
            }
        });
        let prompt_blur = cx.on_blur(&prompt_focus, window, |this, window, cx| {
            if this.prompt.is_some() && window.is_window_active() {
                this.close_prompt(true, cx);
            }
        });
        let overlay_blur = cx.on_blur(&overlay_focus, window, |this, window, cx| {
            if this.overlay.is_some() && window.is_window_active() {
                this.close_overlay(cx);
            }
        });
        let memory_path = dirs::state_dir().map(|d| d.join("memory"));
        let tab = TabSnap::new(start.clone());
        let mut explorer = Self {
            cwd: start.clone(),
            home: std::env::var_os("HOME").map(PathBuf::from),
            entries: Vec::new(),
            unreadable: 0,
            sort: Sort::default(),
            selected: 0,
            error: None,
            slow: false,
            show_hidden: config.show_hidden.unwrap_or(true),
            columns: config.columns,
            tz: TimeZone::system(),
            mode: config.mode,
            count: None,
            editing: None,
            prompt: None,
            marked: BTreeSet::new(),
            visual: None,
            clipboard: None,
            notice: None,
            git: None,
            preview_on: config.preview.unwrap_or(true),
            preview: None,
            dir_sizes: HashMap::new(),
            sizing: false,
            overlay: None,
            output: None,
            tabs: vec![tab],
            active_tab: 0,
            focus,
            path_focus,
            prompt_focus,
            overlay_focus,
            scroll: UniformListScrollHandle::new(),
            page_rows: 20,
            all: Vec::new(),
            history: History::default(),
            last_search: None,
            typeahead: String::new(),
            typeahead_at: Instant::now(),
            loading: false,
            generation: 0,
            undo_stack: Vec::new(),
            memory: memory_path.as_deref().map(Memory::load).unwrap_or_default(),
            memory_path,
            notice_id: 0,
            preview_gen: 0,
            git_gen: 0,
            du_gen: 0,
            du_pending: 0,
            du_cancel: Arc::new(AtomicBool::new(false)),
            tasks: Tasks::default(),
            watcher: None,
            _subscriptions: vec![path_blur, prompt_blur, overlay_blur],
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

    pub fn shown_cwd(&self) -> String {
        display_path(&self.cwd, self.home.as_deref())
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
        self.tasks.load = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            this.update(cx, |this, cx| {
                this.finish_load(generation, target, select, nav, result, cx);
            })
            .ok();
        }));
        self.tasks.slow = Some(cx.spawn(async move |this, cx| {
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
            Ok(listing) => self.apply_listing(target, select, nav, listing, cx),
            Err(error) if nav == Nav::Refresh => {
                // La carpeta desapareció o dejó de poder leerse mientras estaba abierta.
                if matches!(error, FsError::NotFound)
                    && let Some(parent) = self.cwd.parent().map(Path::to_path_buf)
                {
                    self.show_notice("la carpeta ya no existe: se sube un nivel", true, cx);
                    self.navigate(parent, None, Nav::Visit, cx);
                } else {
                    self.show_notice(format!("{}: {}", error.code(), error), true, cx);
                }
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

    fn apply_listing(
        &mut self,
        target: PathBuf,
        select: Option<OsString>,
        nav: Nav,
        listing: Listing,
        cx: &mut Context<Self>,
    ) {
        match nav {
            Nav::Visit if target != self.cwd => self.history.visit(self.cwd.clone()),
            Nav::Back => self.history.went_back(self.cwd.clone()),
            Nav::Forward => self.history.went_forward(self.cwd.clone()),
            Nav::Visit | Nav::Replace | Nav::Refresh => {}
        }
        let previous = self.selected;
        let same_folder = nav == Nav::Refresh && target == self.cwd;
        self.cwd = target;
        self.all = listing.entries;
        self.unreadable = listing.unreadable;
        self.error = None;
        self.apply_view();
        if same_folder {
            // Conserva la selección múltiple de lo que siga existiendo.
            let names: BTreeSet<_> = self.entries.iter().map(|e| e.name.clone()).collect();
            self.marked.retain(|n| names.contains(n));
        } else {
            self.marked.clear();
            self.visual = None;
            self.count = None;
            self.prompt = None;
            self.dir_sizes.clear();
            self.du_cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            self.sizing = false;
        }
        let last = self.entries.len().saturating_sub(1);
        self.selected = select
            .and_then(|name| self.position_of(&name))
            .unwrap_or(if same_folder { previous.min(last) } else { 0 });
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        if nav != Nav::Refresh {
            self.record_visit(cx);
        }
        self.request_git(cx);
        self.request_preview(cx);
        if !same_folder || self.watcher.is_none() {
            self.watch_cwd(cx);
        }
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

    pub(crate) fn position_of(&self, name: &OsStr) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    pub(crate) fn selected_name(&self) -> Option<OsString> {
        self.entries.get(self.selected).map(|e| e.name.clone())
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.entries
            .get(self.selected)
            .map(|e| self.cwd.join(&e.name))
    }

    // --- Selección y orden ---------------------------------------------------------------

    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.entries.len() {
            self.selected = index;
            self.scroll.scroll_to_item(index, ScrollStrategy::Top);
            self.sync_visual();
            self.request_preview(cx);
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
        let sort = self.sort.toggled(key);
        self.apply_sort(sort, cx);
    }

    pub fn apply_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        let name = self.selected_name();
        self.sort = sort;
        // ponytail: se ordena en el hilo de la interfaz; con cientos de miles de entradas habría
        // que moverlo a segundo plano como la carga.
        sort_entries(&mut self.all, self.sort);
        self.apply_view();
        self.selected = name.and_then(|n| self.position_of(&n)).unwrap_or_default();
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        self.visual = None;
        cx.notify();
    }

    pub fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        let name = self.selected_name();
        let previous = self.selected;
        self.show_hidden = !self.show_hidden;
        config::save_show_hidden(self.show_hidden);
        self.apply_view();
        self.selected = name
            .and_then(|n| self.position_of(&n))
            .unwrap_or_else(|| previous.min(self.entries.len().saturating_sub(1)));
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        self.visual = None;
        self.request_preview(cx);
        cx.notify();
    }

    // --- Navegación ----------------------------------------------------------------------

    pub fn go_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate(path, None, Nav::Visit, cx);
    }

    /// Entra en una carpeta, o abre un archivo con el programa por defecto.
    pub fn open_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let path = self.cwd.join(&entry.name);
        if entry.is_dir() {
            self.navigate(path, None, Nav::Visit, cx);
        } else {
            self.open_default_path(&path, cx);
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

    pub fn go_home(&mut self, cx: &mut Context<Self>) {
        if let Some(home) = self.home.clone() {
            self.go_to(home, cx);
        }
    }

    /// Vuelve a leer la carpeta actual conservando la selección (y marcas).
    pub fn refresh(&mut self, select: Option<OsString>, cx: &mut Context<Self>) {
        let select = select.or_else(|| self.selected_name());
        self.navigate(self.cwd.clone(), select, Nav::Refresh, cx);
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
}

/// Aplica una tecla a un campo de una línea. Devuelve `true` si algo cambió.
pub(crate) fn apply_edit_key(
    edit: &mut LineEdit,
    event: &KeyDownEvent,
    cx: &mut Context<Explorer>,
) -> bool {
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
