//! Configuración mínima: el modo de teclado se recuerda entre sesiones.
//!
//! Se guarda en `$XDG_CONFIG_HOME/shieldexplorer/config` (o `~/.config/...`) como líneas
//! `clave=valor`. Un archivo ausente o ilegible simplemente deja el valor por defecto.

use std::path::PathBuf;

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

fn parse(content: &str) -> KeyMode {
    content
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "mode")
        .map_or(KeyMode::default(), |(_, value)| match value.trim() {
            "vim" => KeyMode::Vim,
            _ => KeyMode::Explorer,
        })
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("shieldexplorer").join("config"))
}

pub fn load_mode() -> KeyMode {
    config_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or(KeyMode::default(), |content| parse(&content))
}

pub fn save_mode(mode: KeyMode) {
    let Some(path) = config_path() else {
        return;
    };
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, format!("mode={}\n", mode.as_str())));
    if let Err(error) = result {
        eprintln!("shieldexplorer: no se pudo guardar la configuración: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_saved_mode() {
        assert_eq!(parse("mode=vim\n"), KeyMode::Vim);
        assert_eq!(parse("otra=1\n mode = explorer \n"), KeyMode::Explorer);
        assert_eq!(parse(""), KeyMode::Explorer);
        assert_eq!(parse("mode=raro"), KeyMode::Explorer);
    }

    #[test]
    fn round_trips_through_text() {
        for mode in [KeyMode::Explorer, KeyMode::Vim] {
            assert_eq!(parse(&format!("mode={}\n", mode.as_str())), mode);
        }
        assert_eq!(KeyMode::Vim.toggled(), KeyMode::Explorer);
    }
}
