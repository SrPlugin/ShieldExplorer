//! Abrir archivos con el programa por defecto, con `$EDITOR` y abrir una terminal.
//!
//! Los programas se lanzan sin shell (los argumentos van tal cual, sin interpretar) y separados de
//! este proceso: siguen vivos aunque se cierre el explorador.

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Terminales conocidas y lo que hay que ponerles antes del comando a ejecutar.
const TERMINALS: &[(&str, &[&str])] = &[
    ("x-terminal-emulator", &["-e"]),
    ("gnome-terminal", &["--"]),
    ("kgx", &["--"]),
    ("konsole", &["-e"]),
    ("alacritty", &["-e"]),
    ("kitty", &[]),
    ("wezterm", &["start", "--"]),
    ("foot", &[]),
    ("xfce4-terminal", &["-x"]),
    ("tilix", &["-e"]),
    ("xterm", &["-e"]),
];

/// Editores con ventana propia: no necesitan terminal.
const GUI_EDITORS: &[&str] = &[
    "code",
    "codium",
    "zed",
    "zeditor",
    "subl",
    "gedit",
    "kate",
    "gnome-text-editor",
    "xed",
    "mousepad",
    "pluma",
    "gvim",
    "idea",
];

/// Parte una línea en palabras respetando comillas simples y dobles, sin pasar por un shell.
pub fn split_command(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, c) => current.push(c),
        }
    }
    if started || !current.is_empty() {
        words.push(current);
    }
    words
}

/// Ruta de un programa si existe y es ejecutable (busca en `PATH` si no lleva `/`).
pub fn which(program: &str) -> Option<PathBuf> {
    let executable = |p: &Path| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if program.contains('/') {
        return executable(Path::new(program)).then(|| PathBuf::from(program));
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|p| executable(p))
}

fn terminal_args(program: &str) -> &'static [&'static str] {
    let name = Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program);
    TERMINALS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(&["-e"], |(_, args)| args)
}

/// La terminal a usar: `$TERMINAL` si existe y, si no, la primera conocida que esté instalada.
fn find_terminal() -> Option<String> {
    if let Some(t) = std::env::var("TERMINAL")
        .ok()
        .filter(|t| which(t).is_some())
    {
        return Some(t);
    }
    TERMINALS
        .iter()
        .map(|(n, _)| *n)
        .find(|n| which(n).is_some())
        .map(str::to_owned)
}

fn not_found(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("no se encontró {what}"))
}

/// Lanza el programa separado de este proceso, sin entrada ni salida, y recoge su fin en un hilo
/// para no dejar procesos zombi.
fn spawn_detached(command: &mut Command) -> io::Result<()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Abre con el programa que el sistema asocie (`xdg-open`).
pub fn open_default(path: &Path) -> io::Result<()> {
    let program = which("xdg-open").ok_or_else(|| not_found("xdg-open"))?;
    spawn_detached(Command::new(program).arg(path))
}

/// Abre en `$VISUAL` o `$EDITOR`. Si el editor es de terminal, lo abre dentro de una.
pub fn open_editor(path: &Path, cwd: &Path) -> io::Result<()> {
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|e| !e.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|e| !e.trim().is_empty())
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "define $EDITOR o $VISUAL"))?;
    let words = split_command(&editor);
    let program = words.first().ok_or_else(|| not_found("el editor"))?;
    let base = Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program);
    if GUI_EDITORS.contains(&base) {
        return spawn_detached(
            Command::new(program)
                .args(&words[1..])
                .arg(path)
                .current_dir(cwd),
        );
    }
    let terminal = find_terminal().ok_or_else(|| not_found("una terminal"))?;
    spawn_detached(
        Command::new(&terminal)
            .args(terminal_args(&terminal))
            .args(&words)
            .arg(path)
            .current_dir(cwd),
    )
}

/// Abre una terminal en `dir`.
pub fn open_terminal(dir: &Path) -> io::Result<()> {
    let terminal = find_terminal().ok_or_else(|| not_found("una terminal"))?;
    spawn_detached(Command::new(terminal).current_dir(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_with_quotes() {
        assert_eq!(split_command("vim -u NONE"), ["vim", "-u", "NONE"]);
        assert_eq!(split_command("  code   --wait "), ["code", "--wait"]);
        assert_eq!(split_command(r#"ed "a b" 'c d'"#), ["ed", "a b", "c d"]);
        assert_eq!(split_command(r#"x "q\"r""#), ["x", "q\"r"]);
        assert_eq!(split_command(r#"a "" b"#), ["a", "", "b"]);
        assert!(split_command("   ").is_empty());
    }

    #[test]
    fn knows_terminal_flags() {
        assert_eq!(terminal_args("gnome-terminal"), ["--"]);
        assert_eq!(terminal_args("/usr/bin/kitty"), [] as [&str; 0]);
        assert_eq!(terminal_args("rara"), ["-e"]);
    }

    #[test]
    fn finds_programs_in_path() {
        assert!(which("sh").is_some());
        assert!(which("programa-que-no-existe-xyz").is_none());
        assert!(which("/bin/sh").is_some());
        assert!(which("/etc/hostname").is_none()); // existe pero no es ejecutable
    }
}
