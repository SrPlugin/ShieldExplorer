//! Acciones y atajos de teclado.

use gpui::{App, KeyBinding, actions};

actions!(
    shieldexplorer,
    [
        MoveDown,
        MoveUp,
        Open,
        Parent,
        First,
        Last,
        EditPath,
        ConfirmPath,
        CancelEdit,
        GoBack,
        GoForward,
        Dismiss,
        Quit
    ]
);

/// Contexto de teclado de la lista. Los atajos de una letra (`j`, `h`, `i`…) solo valen aquí: con
/// el foco en la barra de ruta no se disparan y la letra se escribe.
pub const LIST_CONTEXT: &str = "FileList";
pub const PATH_CONTEXT: &str = "PathBar";

pub fn bind(cx: &mut App) {
    let list = Some(LIST_CONTEXT);
    let path = Some(PATH_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("j", MoveDown, list),
        KeyBinding::new("down", MoveDown, list),
        KeyBinding::new("k", MoveUp, list),
        KeyBinding::new("up", MoveUp, list),
        KeyBinding::new("l", Open, list),
        KeyBinding::new("right", Open, list),
        KeyBinding::new("enter", Open, list),
        KeyBinding::new("h", Parent, list),
        KeyBinding::new("left", Parent, list),
        KeyBinding::new("backspace", Parent, list),
        KeyBinding::new("g g", First, list),
        KeyBinding::new("home", First, list),
        KeyBinding::new("shift-g", Last, list),
        KeyBinding::new("end", Last, list),
        KeyBinding::new("i", EditPath, list),
        KeyBinding::new("alt-left", GoBack, list),
        KeyBinding::new("alt-right", GoForward, list),
        KeyBinding::new("escape", Dismiss, list),
        KeyBinding::new("enter", ConfirmPath, path),
        KeyBinding::new("escape", CancelEdit, path),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}
