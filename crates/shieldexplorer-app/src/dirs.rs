//! Carpetas estándar de XDG para la configuración, el estado y la papelera.

use std::path::PathBuf;

/// `$VAR` si es una ruta absoluta; si no, `~/<fallback>`.
fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))
}

/// `~/.config/shieldexplorer`
pub fn config_dir() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config").map(|d| d.join("shieldexplorer"))
}

/// `~/.local/state/shieldexplorer`: marcas y carpetas visitadas.
pub fn state_dir() -> Option<PathBuf> {
    xdg("XDG_STATE_HOME", ".local/state").map(|d| d.join("shieldexplorer"))
}

/// La papelera del usuario según freedesktop: `~/.local/share/Trash`.
pub fn trash_root() -> Option<PathBuf> {
    xdg("XDG_DATA_HOME", ".local/share").map(|d| d.join("Trash"))
}
