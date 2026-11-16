//! Acciones y atajos de teclado, según el modo (explorador normal o vim).

use gpui::{App, KeyBinding, actions};

actions!(
    shieldexplorer,
    [
        MoveDown,
        MoveUp,
        PageDown,
        PageUp,
        HalfDown,
        HalfUp,
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
        ToggleHidden,
        Reload,
        ToggleMode,
        StartSearch,
        SearchNext,
        SearchPrev,
        ConfirmSearch,
        CancelSearch,
        Quit
    ]
);

/// Contexto de la lista. Va acompañado de `vim` o `explorer` según el modo: los atajos de una
/// letra (`j`, `h`, `n`…) solo valen en `vim`, y en `explorer` las letras sirven para saltar.
pub const LIST_CONTEXT_VIM: &str = "FileList vim";
pub const LIST_CONTEXT_EXPLORER: &str = "FileList explorer";
pub const PATH_CONTEXT: &str = "PathBar";
pub const SEARCH_CONTEXT: &str = "SearchBar";

pub fn bind(cx: &mut App) {
    let list = Some("FileList");
    let vim = Some("FileList && vim");
    let path = Some(PATH_CONTEXT);
    let search = Some(SEARCH_CONTEXT);
    cx.bind_keys([
        // En los dos modos.
        KeyBinding::new("down", MoveDown, list),
        KeyBinding::new("up", MoveUp, list),
        KeyBinding::new("pagedown", PageDown, list),
        KeyBinding::new("pageup", PageUp, list),
        KeyBinding::new("home", First, list),
        KeyBinding::new("end", Last, list),
        KeyBinding::new("enter", Open, list),
        KeyBinding::new("backspace", Parent, list),
        KeyBinding::new("alt-up", Parent, list),
        KeyBinding::new("alt-left", GoBack, list),
        KeyBinding::new("alt-right", GoForward, list),
        KeyBinding::new("ctrl-l", EditPath, list),
        KeyBinding::new("f4", EditPath, list),
        KeyBinding::new("ctrl-h", ToggleHidden, list),
        KeyBinding::new("f5", Reload, list),
        KeyBinding::new("escape", Dismiss, list),
        KeyBinding::new("ctrl-shift-m", ToggleMode, list),
        // Solo modo vim.
        KeyBinding::new("j", MoveDown, vim),
        KeyBinding::new("k", MoveUp, vim),
        KeyBinding::new("l", Open, vim),
        KeyBinding::new("right", Open, vim),
        KeyBinding::new("h", Parent, vim),
        KeyBinding::new("left", Parent, vim),
        KeyBinding::new("g g", First, vim),
        KeyBinding::new("shift-g", Last, vim),
        KeyBinding::new("ctrl-d", HalfDown, vim),
        KeyBinding::new("ctrl-u", HalfUp, vim),
        KeyBinding::new("i", EditPath, vim),
        KeyBinding::new(".", ToggleHidden, vim),
        KeyBinding::new("/", StartSearch, vim),
        KeyBinding::new("n", SearchNext, vim),
        KeyBinding::new("shift-n", SearchPrev, vim),
        // Campos de texto.
        KeyBinding::new("enter", ConfirmPath, path),
        KeyBinding::new("escape", CancelEdit, path),
        KeyBinding::new("enter", ConfirmSearch, search),
        KeyBinding::new("escape", CancelSearch, search),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}
