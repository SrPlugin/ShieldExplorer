//! ShieldExplorer: un explorador de archivos para Linux hecho con GPUI.

mod assets;
mod keys;
mod path_edit;
mod state;
mod theme;
mod ui;

use std::ffi::OsString;
use std::path::PathBuf;

use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

use crate::keys::Quit;
use crate::state::Explorer;
use crate::theme::Theme;

/// Carpeta inicial: el argumento si es una carpeta (o la carpeta de un archivo, que queda
/// seleccionado), si no la carpeta actual.
fn start_location() -> (PathBuf, Option<OsString>) {
    let requested = std::env::args_os().nth(1).map(PathBuf::from);
    let resolved = requested.as_ref().and_then(|p| p.canonicalize().ok());
    if let (Some(path), None) = (&requested, &resolved) {
        eprintln!("shieldexplorer: no existe {}", path.display());
    }
    let path = resolved
        .or_else(|| std::env::current_dir().ok())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/"));
    if path.is_dir() {
        return (path, None);
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => (parent.to_path_buf(), Some(name.to_os_string())),
        _ => (PathBuf::from("/"), None),
    }
}

fn main() {
    let (start, select) = start_location();
    Application::new().run(move |cx: &mut App| {
        if let Err(error) = assets::load_fonts(cx) {
            eprintln!("shieldexplorer: no se pudieron cargar las fuentes: {error:#}");
        }
        cx.set_global(Theme::for_appearance(cx.window_appearance()));
        keys::bind(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.), px(700.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("ShieldExplorer".into()),
                ..Default::default()
            }),
            app_id: Some("shieldexplorer".to_owned()),
            window_min_size: Some(size(px(480.), px(320.))),
            ..Default::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            // Sigue el modo claro/oscuro del sistema mientras la app está abierta.
            window
                .observe_window_appearance(|window, cx| {
                    cx.set_global(Theme::for_appearance(window.appearance()));
                    window.refresh();
                })
                .detach();
            cx.new(|cx| Explorer::new(start, select, window, cx))
        });
        if let Err(error) = opened {
            eprintln!("shieldexplorer: no se pudo abrir la ventana: {error:#}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
