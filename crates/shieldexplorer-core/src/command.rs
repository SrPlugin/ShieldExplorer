//! Línea de comandos `:` del modo vim: convierte lo escrito en una orden.

use crate::sort::SortKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YankKind {
    /// Ruta absoluta.
    Path,
    /// Solo el nombre.
    Name,
    /// Relativa a la raíz del proyecto (o al directorio personal).
    Relative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeArg {
    Vim,
    Explorer,
    Toggle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Cd(String),
    Mkdir(String),
    Touch(String),
    Rename(String),
    /// Mandar a la papelera lo seleccionado.
    Delete,
    /// Ir a la carpeta de la papelera.
    TrashFolder,
    Open,
    Edit,
    Terminal,
    Hidden,
    Sort {
        key: SortKey,
        ascending: bool,
    },
    Mode(ModeArg),
    Quit,
    Mark(char),
    Jump(char),
    /// Saltar a la carpeta frecuente que mejor encaje.
    Z(String),
    /// Ejecutar en un shell dentro de la carpeta actual.
    Shell(String),
    Yank(YankKind),
    /// Calcular el tamaño real de las carpetas.
    Du,
    Undo,
    Reload,
    Disks,
    TabNew,
    TabClose,
    Preview,
}

fn single_char(arg: &str, what: &str) -> Result<char, String> {
    let mut chars = arg.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_alphanumeric() => Ok(c),
        _ => Err(format!("{what} necesita una letra o un número")),
    }
}

fn need(arg: &str, name: &str) -> Result<String, String> {
    if arg.is_empty() {
        Err(format!(":{name} necesita un argumento"))
    } else {
        Ok(arg.to_owned())
    }
}

pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim().trim_start_matches(':').trim();
    if let Some(shell) = input.strip_prefix('!') {
        let shell = shell.trim();
        return if shell.is_empty() {
            Err(":! necesita un comando".into())
        } else {
            Ok(Command::Shell(shell.into()))
        };
    }
    let (name, arg) = input
        .split_once(char::is_whitespace)
        .map_or((input, ""), |(n, a)| (n, a.trim()));
    Ok(match name {
        "" => return Err("escribe un comando".into()),
        "q" | "quit" | "exit" => Command::Quit,
        "cd" => Command::Cd(if arg.is_empty() {
            "~".into()
        } else {
            arg.into()
        }),
        "mkdir" | "md" => Command::Mkdir(need(arg, name)?),
        "touch" | "new" => Command::Touch(need(arg, name)?),
        "rename" | "rn" | "mv" => Command::Rename(need(arg, name)?),
        "rm" | "delete" | "del" => Command::Delete,
        "trash" => Command::TrashFolder,
        "open" => Command::Open,
        "edit" | "e" => Command::Edit,
        "term" | "terminal" => Command::Terminal,
        "hidden" => Command::Hidden,
        "sort" => {
            let mut words = arg.split_whitespace();
            let key = match words.next() {
                Some("name" | "nombre") => SortKey::Name,
                Some("size" | "tamaño" | "tamano") => SortKey::Size,
                Some("date" | "fecha" | "time" | "modified") => SortKey::Modified,
                _ => return Err(":sort name|size|date [r]".into()),
            };
            Command::Sort {
                key,
                ascending: !matches!(words.next(), Some("r" | "desc" | "reverse")),
            }
        }
        "mode" => Command::Mode(match arg {
            "vim" => ModeArg::Vim,
            "explorer" | "normal" => ModeArg::Explorer,
            "" | "toggle" => ModeArg::Toggle,
            _ => return Err(":mode vim|explorer".into()),
        }),
        "mark" | "m" => Command::Mark(single_char(arg, ":mark")?),
        "jump" | "'" => Command::Jump(single_char(arg, ":jump")?),
        "z" => Command::Z(need(arg, name)?),
        "yank" | "y" => Command::Yank(match arg {
            "path" | "" => YankKind::Path,
            "name" => YankKind::Name,
            "rel" | "relative" => YankKind::Relative,
            _ => return Err(":yank path|name|rel".into()),
        }),
        "du" => Command::Du,
        "undo" | "u" => Command::Undo,
        "reload" | "e!" => Command::Reload,
        "disks" | "df" | "discos" => Command::Disks,
        "tabnew" | "tab" => Command::TabNew,
        "tabclose" | "tabc" => Command::TabClose,
        "preview" | "pv" => Command::Preview,
        other => return Err(format!("comando desconocido: {other}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_commands_and_aliases() {
        assert_eq!(parse(":q"), Ok(Command::Quit));
        assert_eq!(
            parse("  cd ~/proyectos  "),
            Ok(Command::Cd("~/proyectos".into()))
        );
        assert_eq!(parse("cd"), Ok(Command::Cd("~".into())));
        assert_eq!(
            parse("md nueva carpeta"),
            Ok(Command::Mkdir("nueva carpeta".into()))
        );
        assert_eq!(parse("mv otro.txt"), Ok(Command::Rename("otro.txt".into())));
        assert_eq!(parse("rm"), Ok(Command::Delete));
        assert_eq!(parse("trash"), Ok(Command::TrashFolder));
        assert_eq!(parse("discos"), Ok(Command::Disks));
    }

    #[test]
    fn parses_shell_escape_verbatim() {
        assert_eq!(
            parse(":!ls -la | wc -l"),
            Ok(Command::Shell("ls -la | wc -l".into()))
        );
        assert_eq!(parse("! echo %s"), Ok(Command::Shell("echo %s".into())));
        assert!(parse("!").is_err());
    }

    #[test]
    fn parses_sort_mode_yank_and_marks() {
        assert_eq!(
            parse("sort size"),
            Ok(Command::Sort {
                key: SortKey::Size,
                ascending: true
            })
        );
        assert_eq!(
            parse("sort date r"),
            Ok(Command::Sort {
                key: SortKey::Modified,
                ascending: false
            })
        );
        assert_eq!(parse("mode vim"), Ok(Command::Mode(ModeArg::Vim)));
        assert_eq!(parse("mode"), Ok(Command::Mode(ModeArg::Toggle)));
        assert_eq!(parse("yank rel"), Ok(Command::Yank(YankKind::Relative)));
        assert_eq!(parse("mark a"), Ok(Command::Mark('a')));
        assert_eq!(parse("z shield proy"), Ok(Command::Z("shield proy".into())));
    }

    #[test]
    fn explains_errors() {
        assert!(parse("").unwrap_err().contains("escribe"));
        assert!(parse("mkdir").unwrap_err().contains("argumento"));
        assert!(parse("mark ab").is_err());
        assert!(parse("sort color").is_err());
        assert!(parse("volar").unwrap_err().contains("desconocido"));
    }
}
