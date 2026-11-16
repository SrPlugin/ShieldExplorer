//! Abrir archivos y terminales, y ejecutar comandos de shell con su salida en un panel.

use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use gpui::{Context, Window};
use shieldexplorer_core::{
    Running, ShellEvent, open_default, open_editor, open_terminal, spawn_shell,
};

use super::Explorer;
use crate::keys as k;

/// Líneas de salida que se conservan; las más viejas se descartan.
const MAX_OUTPUT_LINES: usize = 2000;
/// Cada cuánto se recoge la salida de un comando en marcha.
const POLL: Duration = Duration::from_millis(40);

/// Salida del último comando `:!`.
pub struct Output {
    pub command: String,
    pub lines: VecDeque<String>,
    /// `Some(código)` cuando terminó (`None` dentro si lo mató una señal).
    pub finished: Option<Option<i32>>,
    running: Option<Running>,
}

/// Entre comillas simples para un shell: `it's` → `'it'\''s'`.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

impl Explorer {
    pub fn open_default_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match open_default(path) {
            Ok(()) => self.show_notice(format!("abriendo {name}…"), false, cx),
            Err(error) => self.show_notice(format!("no se pudo abrir {name}: {error}"), true, cx),
        }
    }

    pub fn on_open_default(&mut self, _: &k::OpenDefault, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        if let Some(path) = self.selected_path().filter(|_| self.error.is_none()) {
            self.open_default_path(&path, cx);
        }
    }

    pub fn open_editor(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.selected_path().filter(|_| self.error.is_none()) else {
            return;
        };
        match open_editor(&path, &self.cwd) {
            Ok(()) => self.show_notice("abriendo el editor…", false, cx),
            Err(error) => {
                self.show_notice(format!("no se pudo abrir el editor: {error}"), true, cx)
            }
        }
    }

    pub fn on_open_editor(&mut self, _: &k::OpenEditor, _: &mut Window, cx: &mut Context<Self>) {
        self.count = None;
        self.open_editor(cx);
    }

    pub fn open_terminal(&mut self, cx: &mut Context<Self>) {
        match open_terminal(&self.cwd) {
            Ok(()) => self.show_notice("abriendo la terminal…", false, cx),
            Err(error) => {
                self.show_notice(format!("no se pudo abrir la terminal: {error}"), true, cx)
            }
        }
    }

    pub fn on_open_terminal(
        &mut self,
        _: &k::OpenTerminal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.count = None;
        self.open_terminal(cx);
    }

    // --- Comandos de shell -----------------------------------------------------------------

    /// Ejecuta `command` en la carpeta actual. `%s` se sustituye por el nombre seleccionado, ya
    /// entre comillas.
    pub fn run_shell(&mut self, command: String, cx: &mut Context<Self>) {
        self.close_output();
        let selected = self
            .selected_name()
            .map(|n| shell_quote(&n.to_string_lossy()))
            .unwrap_or_default();
        let expanded = command.replace("%s", &selected);
        let running = match spawn_shell(&expanded, &self.cwd) {
            Ok(running) => running,
            Err(error) => {
                self.show_notice(format!("no se pudo ejecutar: {error}"), true, cx);
                return;
            }
        };
        self.output = Some(Output {
            command,
            lines: VecDeque::new(),
            finished: None,
            running: Some(running),
        });
        self.tasks.shell = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let done = this
                    .update(cx, |this, cx| this.poll_output(cx))
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Recoge la salida pendiente. Devuelve `true` cuando el comando ya terminó.
    fn poll_output(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(output) = self.output.as_mut() else {
            return true;
        };
        let mut changed = false;
        let mut finished = false;
        while let Some(event) = output.running.as_ref().and_then(|r| r.try_recv().ok()) {
            changed = true;
            match event {
                ShellEvent::Line(line) => {
                    output.lines.push_back(line);
                    if output.lines.len() > MAX_OUTPUT_LINES {
                        output.lines.pop_front();
                    }
                }
                ShellEvent::Done(code) => {
                    output.finished = Some(code);
                    output.running = None;
                    finished = true;
                    break;
                }
            }
        }
        if changed {
            cx.notify();
        }
        if finished {
            self.refresh(None, cx); // el comando pudo cambiar archivos
        }
        finished
    }

    /// Cierra el panel de salida; si el comando seguía en marcha, lo cancela.
    pub fn close_output(&mut self) {
        if let Some(output) = self.output.take()
            && let Some(running) = output.running
        {
            running.cancel();
        }
        self.tasks.shell = None;
    }

    pub fn on_cancel_command(
        &mut self,
        _: &k::CancelCommand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.count = None;
        if let Some(running) = self.output.as_ref().and_then(|o| o.running.as_ref()) {
            running.cancel();
            self.show_notice("cancelando el comando…", false, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shell_quote;

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("$(rm -rf ~)"), "'$(rm -rf ~)'");
    }
}
