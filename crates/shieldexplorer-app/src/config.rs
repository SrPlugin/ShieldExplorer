//! Configuración: `~/.config/shieldexplorer/config`, un archivo de texto con líneas `clave = valor`.
//!
//! ```text
//! mode = vim                       # vim | explorer
//! show_hidden = true
//! preview = true
//! theme = system                   # system | light | dark
//! columns = perms, modified, git   # las que se ven además del tamaño y el nombre
//! bind vim ctrl-x = cut            # atajos propios: bind <vim|explorer|all> <teclas> = <acción>
//! color dark accent = #ff8800      # colores propios: color <dark|light> <nombre> = #rrggbb
//! ```
//!
//! Las líneas que no se entienden se ignoran y se avisa de ellas al arrancar, nunca impiden abrir.

use std::path::PathBuf;

use crate::dirs;

/// Cómo se maneja el explorador con el teclado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyMode {
    /// Atajos de un explorador normal: flechas, Enter, Alt+flechas, escribir para saltar.
    #[default]
    Explorer,
    /// Atajos estilo vim: `hjkl`, contadores, `/` para buscar.
    Vim,
}

impl KeyMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Explorer => Self::Vim,
            Self::Vim => Self::Explorer,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Explorer => "explorer",
            Self::Vim => "vim",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

/// Columnas opcionales del listado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Columns {
    pub perms: bool,
    pub modified: bool,
    pub git: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Self {
            perms: true,
            modified: true,
            git: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindScope {
    All,
    Vim,
    Explorer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserBind {
    pub scope: BindScope,
    pub keys: String,
    pub action: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub mode: KeyMode,
    pub show_hidden: Option<bool>,
    pub preview: Option<bool>,
    pub theme: ThemeChoice,
    pub columns: Columns,
    pub binds: Vec<UserBind>,
    /// `(esquema oscuro?, nombre, 0xRRGGBB)`
    pub colors: Vec<(bool, String, u32)>,
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn parse_hex(value: &str) -> Option<u32> {
    let hex = value.strip_prefix('#')?;
    (hex.len() == 6)
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}

/// Interpreta el texto del archivo. Devuelve la configuración y un aviso por cada línea ignorada.
pub fn parse(text: &str) -> (Config, Vec<String>) {
    let mut config = Config::default();
    let mut warnings = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        // Un `#` dentro de un color (`#ff8800`) no es un comentario.
        let line = if raw.trim_start().starts_with("color") {
            raw.trim()
        } else {
            line
        };
        if line.is_empty() {
            continue;
        }
        let Some((left, value)) = line.split_once('=') else {
            warnings.push(format!("línea {}: falta '='", n + 1));
            continue;
        };
        // Se recorta antes de cortar el comentario: `" #fafafa"` no es un comentario vacío.
        let value = value.trim();
        let (left, value) = (
            left.trim(),
            value.split(" #").next().unwrap_or(value).trim(),
        );
        let ok = match left.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["mode"] => match value {
                "vim" | "explorer" => {
                    config.mode = if value == "vim" {
                        KeyMode::Vim
                    } else {
                        KeyMode::Explorer
                    };
                    true
                }
                _ => false,
            },
            ["show_hidden"] => parse_bool(value).is_some_and(|b| {
                config.show_hidden = Some(b);
                true
            }),
            ["preview"] => parse_bool(value).is_some_and(|b| {
                config.preview = Some(b);
                true
            }),
            ["theme"] => {
                let choice = match value {
                    "system" => Some(ThemeChoice::System),
                    "light" => Some(ThemeChoice::Light),
                    "dark" => Some(ThemeChoice::Dark),
                    _ => None,
                };
                choice.is_some_and(|c| {
                    config.theme = c;
                    true
                })
            }
            ["columns"] => {
                let names: Vec<&str> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                let known = names
                    .iter()
                    .all(|n| matches!(*n, "perms" | "modified" | "git"));
                if known {
                    config.columns = Columns {
                        perms: names.contains(&"perms"),
                        modified: names.contains(&"modified"),
                        git: names.contains(&"git"),
                    };
                }
                known
            }
            ["bind", scope, keys @ ..] if !keys.is_empty() && !value.is_empty() => {
                let scope = match *scope {
                    "all" => Some(BindScope::All),
                    "vim" => Some(BindScope::Vim),
                    "explorer" => Some(BindScope::Explorer),
                    _ => None,
                };
                scope.is_some_and(|scope| {
                    config.binds.push(UserBind {
                        scope,
                        keys: keys.join(" "),
                        action: value.to_owned(),
                    });
                    true
                })
            }
            ["color", scheme, name] => match (*scheme, parse_hex(value)) {
                ("dark" | "light", Some(rgb)) => {
                    config
                        .colors
                        .push((*scheme == "dark", (*name).to_owned(), rgb));
                    true
                }
                _ => false,
            },
            _ => false,
        };
        if !ok {
            warnings.push(format!("línea {}: no se entiende «{}»", n + 1, raw.trim()));
        }
    }
    (config, warnings)
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("config"))
}

/// Lee la configuración. Un archivo ausente o ilegible deja los valores por defecto.
pub fn load() -> (Config, Vec<String>) {
    config_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(|| (Config::default(), Vec::new()), |text| parse(&text))
}

/// Cambia (o añade) una sola clave del archivo sin tocar el resto de lo que haya escrito.
fn set_key(text: &str, key: &str, value: &str) -> String {
    let mut done = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|line| {
            let is_key = line.split_once('=').is_some_and(|(k, _)| k.trim() == key);
            if is_key && !done {
                done = true;
                format!("{key} = {value}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !done {
        out.push(format!("{key} = {value}"));
    }
    out.join("\n") + "\n"
}

fn save_key(key: &str, value: &str) {
    let Some(path) = config_path() else {
        return;
    };
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, set_key(&current, key, value)));
    if let Err(error) = result {
        eprintln!("shieldexplorer: no se pudo guardar la configuración: {error}");
    }
}

pub fn save_mode(mode: KeyMode) {
    save_key("mode", mode.as_str());
}

pub fn save_show_hidden(show: bool) {
    save_key("show_hidden", &show.to_string());
}

pub fn save_preview(on: bool) {
    save_key("preview", &on.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_setting() {
        let text = "mode = vim\nshow_hidden = false\npreview = on\ntheme = dark\ncolumns = perms, git\n\
                    bind vim ctrl-x = cut\nbind all g x = disks\ncolor dark accent = #ff8800\n# comentario\n\n";
        let (c, warnings) = parse(text);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(c.mode, KeyMode::Vim);
        assert_eq!((c.show_hidden, c.preview), (Some(false), Some(true)));
        assert_eq!(c.theme, ThemeChoice::Dark);
        assert_eq!(
            c.columns,
            Columns {
                perms: true,
                modified: false,
                git: true
            }
        );
        assert_eq!(
            c.binds[0],
            UserBind {
                scope: BindScope::Vim,
                keys: "ctrl-x".into(),
                action: "cut".into()
            }
        );
        assert_eq!(c.binds[1].keys, "g x");
        assert_eq!(c.colors, [(true, "accent".to_owned(), 0xff8800)]);
    }

    #[test]
    fn defaults_when_empty_and_warns_on_garbage() {
        assert_eq!(parse("").0, Config::default());
        let (c, w) = parse(
            "mode = raro\nsin igual\ncolumns = nope\ncolor dark accent = rojo\nbind vim = x\n",
        );
        assert_eq!(c, Config::default());
        assert_eq!(w.len(), 5);
    }

    #[test]
    fn comments_after_values_are_ignored() {
        let (c, w) = parse("mode = vim # mi modo\ncolor light bg = #fafafa\n");
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.mode, KeyMode::Vim);
        assert_eq!(c.colors, [(false, "bg".to_owned(), 0xfafafa)]);
    }

    #[test]
    fn set_key_keeps_the_rest_of_the_file() {
        let original = "# mi config\nmode = explorer\nbind vim ctrl-x = cut\n";
        let changed = set_key(original, "mode", "vim");
        assert_eq!(changed, "# mi config\nmode = vim\nbind vim ctrl-x = cut\n");
        assert_eq!(set_key("", "preview", "true"), "preview = true\n");
        assert!(set_key(&changed, "preview", "false").ends_with("preview = false\n"));
        assert_eq!(KeyMode::Vim.toggled(), KeyMode::Explorer);
    }
}
