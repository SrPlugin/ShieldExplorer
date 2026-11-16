//! Campos de texto de una línea: la búsqueda `/`, la línea de comandos `:`, renombrar, crear,
//! las marcas y la edición de la ruta.

use std::path::Path;

use gpui::{Context, KeyDownEvent, Window};
use shieldexplorer_core::{Command, ModeArg, Sort, find_match, parse_command, resolve};

use super::{Explorer, apply_edit_key};
use crate::config::KeyMode;
use crate::keys as k;
use crate::line_edit::LineEdit;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Search,
    Command,
    Rename,
    NewFile,
    NewDir,
    /// Espera una letra y guarda ahí la carpeta actual (`m`).
    SetMark,
    /// Espera una letra y salta a esa marca (`'`).
    JumpMark,
}

impl PromptKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Search => "/",
            Self::Command => ":",
            Self::Rename => "renombrar: ",
            Self::NewFile => "archivo nuevo: ",
            Self::NewDir => "carpeta nueva: ",
            Self::SetMark => "guardar marca: ",
            Self::JumpMark => "ir a marca: ",
        }
    }

    /// Las marcas leen una sola tecla y no un texto.
    pub fn single_key(self) -> bool {
        matches!(self, Self::SetMark | Self::JumpMark)
    }
}

pub struct Prompt {
    pub kind: PromptKind,
    pub edit: LineEdit,
    /// Fila donde estaba el cursor al empezar una búsqueda, para volver si se cancela.
    origin: usize,
}

impl Explorer {
    fn open_prompt(
        &mut self,
        kind: PromptKind,
        initial: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.count = None;
        self.prompt = Some(Prompt {
            kind,
            edit: LineEdit::new(initial),
            origin: self.selected,
        });
        window.focus(&self.prompt_focus);
        cx.notify();
    }

    fn list_ready(&self) -> bool {
        self.error.is_none() && !self.entries.is_empty()
    }

    pub fn on_start_search(
        &mut self,
        _: &k::StartSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.list_ready() {
            self.open_prompt(PromptKind::Search, String::new(), window, cx);
        }
    }

    pub fn on_start_command(
        &mut self,
        _: &k::StartCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_prompt(PromptKind::Command, String::new(), window, cx);
    }

    pub fn on_start_set_mark(
        &mut self,
        _: &k::StartSetMark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_prompt(PromptKind::SetMark, String::new(), window, cx);
    }

    pub fn on_start_jump(&mut self, _: &k::StartJump, window: &mut Window, cx: &mut Context<Self>) {
        self.open_prompt(PromptKind::JumpMark, String::new(), window, cx);
    }

    pub fn on_rename_start(
        &mut self,
        _: &k::RenameStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(name) = self.selected_name().filter(|_| self.list_ready()) {
            self.open_prompt(
                PromptKind::Rename,
                name.to_string_lossy().into_owned(),
                window,
                cx,
            );
        }
    }

    pub fn on_new_file(&mut self, _: &k::NewFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_none() {
            self.open_prompt(PromptKind::NewFile, String::new(), window, cx);
        }
    }

    pub fn on_new_dir(&mut self, _: &k::NewDir, window: &mut Window, cx: &mut Context<Self>) {
        if self.error.is_none() {
            self.open_prompt(PromptKind::NewDir, String::new(), window, cx);
        }
    }

    /// Cierra el campo. Si era una búsqueda y se cancela, el cursor vuelve a donde estaba.
    pub(super) fn close_prompt(&mut self, restore: bool, cx: &mut Context<Self>) {
        if let Some(prompt) = self.prompt.take()
            && restore
            && prompt.kind == PromptKind::Search
        {
            self.select(prompt.origin, cx);
        }
        cx.notify();
    }

    /// Teclas dentro del campo. `Enter` y `Esc` llegan como acciones.
    pub fn on_prompt_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        let kind = prompt.kind;
        if kind.single_key() {
            let mods = &event.keystroke.modifiers;
            let letter = event
                .keystroke
                .key_char
                .as_deref()
                .and_then(|t| t.chars().next())
                .filter(|c| c.is_alphanumeric() && !mods.control && !mods.alt);
            if let Some(letter) = letter {
                self.prompt = None;
                window.focus(&self.focus);
                self.use_mark(kind, letter, cx);
            }
            return;
        }
        if !apply_edit_key(&mut prompt.edit, event, cx) {
            return;
        }
        if kind == PromptKind::Search {
            let (text, origin) = (prompt.edit.text().to_owned(), prompt.origin);
            let target = find_match(&self.entries, &text, origin, true).unwrap_or(origin);
            self.select(target, cx);
        }
        cx.notify();
    }

    fn use_mark(&mut self, kind: PromptKind, letter: char, cx: &mut Context<Self>) {
        if kind == PromptKind::SetMark {
            self.memory.set_mark(letter, self.cwd.clone());
            self.save_memory(cx);
            self.show_notice(
                format!("marca '{letter}' guardada en {}", self.shown_cwd()),
                false,
                cx,
            );
        } else if let Some(path) = self.memory.mark(letter).map(Path::to_path_buf) {
            self.go_to(path, cx);
        } else {
            self.show_notice(format!("no hay marca '{letter}'"), true, cx);
        }
    }

    pub fn on_confirm_prompt(
        &mut self,
        _: &k::ConfirmPrompt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        window.focus(&self.focus);
        let text = prompt.edit.text().to_owned();
        match prompt.kind {
            PromptKind::Search => {
                if !text.is_empty() {
                    self.last_search = Some(text);
                }
                cx.notify();
            }
            PromptKind::Command => self.run_command(&text, window, cx),
            PromptKind::Rename if !text.trim().is_empty() => self.do_rename(text, cx),
            PromptKind::NewFile if !text.trim().is_empty() => self.do_create(text, false, cx),
            PromptKind::NewDir if !text.trim().is_empty() => self.do_create(text, true, cx),
            _ => cx.notify(),
        }
    }

    pub fn on_cancel_prompt(
        &mut self,
        _: &k::CancelPrompt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_prompt(true, cx);
        window.focus(&self.focus);
    }

    // --- Búsqueda: n / N -------------------------------------------------------------------

    pub fn on_search_next(&mut self, _: &k::SearchNext, _: &mut Window, cx: &mut Context<Self>) {
        self.repeat_search(true, cx);
    }

    pub fn on_search_prev(&mut self, _: &k::SearchPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.repeat_search(false, cx);
    }

    /// `n` / `N`: siguiente o anterior coincidencia de la última búsqueda, `count` veces.
    fn repeat_search(&mut self, forward: bool, cx: &mut Context<Self>) {
        let times = self.take_count();
        let (Some(pattern), true) = (self.last_search.clone(), self.list_ready()) else {
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

    // --- Edición de la ruta ----------------------------------------------------------------

    pub fn begin_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.editing = Some(LineEdit::new(self.shown_cwd()));
        window.focus(&self.path_focus);
        cx.notify();
    }

    pub fn on_edit_path(&mut self, _: &k::EditPath, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_edit(window, cx);
    }

    pub fn on_confirm_path(
        &mut self,
        _: &k::ConfirmPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        window.focus(&self.focus);
        let target = resolve(edit.text(), &self.cwd, self.home.as_deref());
        self.go_to(target, cx);
    }

    pub fn on_cancel_edit(
        &mut self,
        _: &k::CancelEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    // --- Línea de comandos ":" -------------------------------------------------------------

    fn run_command(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let command = match parse_command(text) {
            Ok(command) => command,
            Err(message) => {
                self.show_notice(message, true, cx);
                return;
            }
        };
        match command {
            Command::Cd(path) => {
                let target = resolve(&path, &self.cwd, self.home.as_deref());
                self.go_to(target, cx);
            }
            Command::Mkdir(name) => self.do_create(name, true, cx),
            Command::Touch(name) => self.do_create(name, false, cx),
            Command::Rename(name) => self.do_rename(name, cx),
            Command::Delete => self.delete_targets(cx),
            Command::TrashFolder => self.go_trash(cx),
            Command::Open => self.open_index(self.selected, cx),
            Command::Edit => self.open_editor(cx),
            Command::Terminal => self.open_terminal(cx),
            Command::Hidden => self.toggle_hidden(cx),
            Command::Sort { key, ascending } => self.apply_sort(Sort { key, ascending }, cx),
            Command::Mode(arg) => {
                let wanted = match arg {
                    ModeArg::Vim => KeyMode::Vim,
                    ModeArg::Explorer => KeyMode::Explorer,
                    ModeArg::Toggle => self.mode.toggled(),
                };
                if wanted != self.mode {
                    self.toggle_mode(cx);
                }
            }
            Command::Quit => cx.quit(),
            Command::Mark(c) => self.use_mark(PromptKind::SetMark, c, cx),
            Command::Jump(c) => self.use_mark(PromptKind::JumpMark, c, cx),
            Command::Z(query) => {
                match self
                    .memory
                    .best_match(&query, super::side::now_secs(), |p| p.is_dir())
                {
                    Some(path) => self.go_to(path, cx),
                    None => self.show_notice(
                        format!("ninguna carpeta visitada encaja con «{query}»"),
                        true,
                        cx,
                    ),
                }
            }
            Command::Shell(command) => self.run_shell(command, cx),
            Command::Yank(kind) => self.yank(kind, cx),
            Command::Du => self.compute_sizes(cx),
            Command::Undo => self.undo_last(cx),
            Command::Reload => self.refresh(None, cx),
            Command::Disks => self.open_disks(window, cx),
            Command::TabNew => self.new_tab(cx),
            Command::TabClose => self.close_tab(cx),
            Command::Preview => self.toggle_preview(cx),
        }
    }
}
