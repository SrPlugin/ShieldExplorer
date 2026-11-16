//! Acciones y atajos de teclado, según el modo (explorador normal o vim).

use gpui::{App, KeyBinding, Keystroke, actions};

use crate::config::{BindScope, UserBind};

actions!(
    shieldexplorer,
    [
        MoveDown,
        MoveUp,
        PageDown,
        PageUp,
        HalfDown,
        HalfUp,
        First,
        Last,
        ExtendDown,
        ExtendUp,
        Open,
        OpenDefault,
        OpenEditor,
        OpenTerminal,
        Parent,
        GoBack,
        GoForward,
        GoHome,
        TrashFolder,
        EditPath,
        ConfirmPath,
        CancelEdit,
        Dismiss,
        ToggleHidden,
        Reload,
        ToggleMode,
        TogglePreview,
        ComputeSizes,
        StartSearch,
        SearchNext,
        SearchPrev,
        StartCommand,
        StartSetMark,
        StartJump,
        ConfirmPrompt,
        CancelPrompt,
        Copy,
        Cut,
        Paste,
        Delete,
        RenameStart,
        NewFile,
        NewDir,
        Undo,
        VisualToggle,
        ToggleMark,
        SelectAll,
        YankPath,
        YankName,
        YankRel,
        FinderFiles,
        FinderFolders,
        Disks,
        OverlayUp,
        OverlayDown,
        OverlayConfirm,
        OverlayCancel,
        CancelCommand,
        TabNew,
        TabClose,
        TabNext,
        TabPrev,
        Quit
    ]
);

/// Contexto de la lista. Va acompañado de `vim` o `explorer` según el modo: los atajos de una
/// letra (`j`, `h`, `n`…) solo valen en `vim`, y en `explorer` las letras sirven para saltar.
pub const LIST_CONTEXT_VIM: &str = "FileList vim";
pub const LIST_CONTEXT_EXPLORER: &str = "FileList explorer";
pub const PATH_CONTEXT: &str = "PathBar";
pub const PROMPT_CONTEXT: &str = "Prompt";
pub const OVERLAY_CONTEXT: &str = "Overlay";

/// Nombre usado en el archivo de configuración → atajo con esa acción.
macro_rules! by_name {
    ($($name:literal => $action:ident),* $(,)?) => {
        fn binding(name: &str, keys: &str, context: &'static str) -> Option<KeyBinding> {
            match name {
                $($name => Some(KeyBinding::new(keys, $action, Some(context))),)*
                _ => None,
            }
        }
    };
}

by_name! {
    "move_down" => MoveDown, "move_up" => MoveUp, "page_down" => PageDown, "page_up" => PageUp,
    "half_down" => HalfDown, "half_up" => HalfUp, "first" => First, "last" => Last,
    "open" => Open, "open_default" => OpenDefault, "edit" => OpenEditor, "terminal" => OpenTerminal,
    "parent" => Parent, "back" => GoBack, "forward" => GoForward, "home" => GoHome,
    "trash_folder" => TrashFolder, "edit_path" => EditPath, "hidden" => ToggleHidden,
    "reload" => Reload, "mode" => ToggleMode, "preview" => TogglePreview, "sizes" => ComputeSizes,
    "search" => StartSearch, "search_next" => SearchNext, "search_prev" => SearchPrev,
    "command" => StartCommand, "mark" => StartSetMark, "jump" => StartJump, "copy" => Copy,
    "cut" => Cut, "paste" => Paste, "delete" => Delete, "rename" => RenameStart,
    "new_file" => NewFile, "new_dir" => NewDir, "undo" => Undo, "visual" => VisualToggle,
    "toggle_mark" => ToggleMark, "select_all" => SelectAll, "yank_path" => YankPath,
    "yank_name" => YankName, "yank_rel" => YankRel, "finder" => FinderFiles,
    "folders" => FinderFolders, "disks" => Disks, "cancel_command" => CancelCommand,
    "tab_new" => TabNew, "tab_close" => TabClose, "tab_next" => TabNext, "tab_prev" => TabPrev,
    "quit" => Quit,
}

pub fn bind(cx: &mut App) {
    let list = Some("FileList");
    let vim = Some("FileList && vim");
    let explorer = Some("FileList && explorer");
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
        KeyBinding::new("tab", TogglePreview, list),
        KeyBinding::new("ctrl-shift-m", ToggleMode, list),
        KeyBinding::new("ctrl-p", FinderFiles, list),
        KeyBinding::new("ctrl-o", FinderFolders, list),
        KeyBinding::new("ctrl-shift-t", TabNew, list),
        KeyBinding::new("ctrl-w", TabClose, list),
        KeyBinding::new("ctrl-tab", TabNext, list),
        KeyBinding::new("ctrl-shift-tab", TabPrev, list),
        // Solo modo explorador.
        KeyBinding::new("ctrl-c", Copy, explorer),
        KeyBinding::new("ctrl-x", Cut, explorer),
        KeyBinding::new("ctrl-v", Paste, explorer),
        KeyBinding::new("delete", Delete, explorer),
        KeyBinding::new("f2", RenameStart, explorer),
        KeyBinding::new("ctrl-n", NewFile, explorer),
        KeyBinding::new("ctrl-shift-n", NewDir, explorer),
        KeyBinding::new("ctrl-z", Undo, explorer),
        KeyBinding::new("ctrl-a", SelectAll, explorer),
        KeyBinding::new("insert", ToggleMark, explorer),
        KeyBinding::new("shift-down", ExtendDown, explorer),
        KeyBinding::new("shift-up", ExtendUp, explorer),
        KeyBinding::new("ctrl-shift-c", YankPath, explorer),
        KeyBinding::new("ctrl-e", OpenEditor, explorer),
        KeyBinding::new("f12", OpenTerminal, explorer),
        KeyBinding::new("ctrl-d", Disks, explorer),
        KeyBinding::new("ctrl-shift-p", StartCommand, explorer),
        KeyBinding::new("ctrl-shift-s", ComputeSizes, explorer),
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
        KeyBinding::new(":", StartCommand, vim),
        KeyBinding::new("m", StartSetMark, vim),
        KeyBinding::new("'", StartJump, vim),
        KeyBinding::new("y y", Copy, vim),
        KeyBinding::new("d d", Cut, vim),
        KeyBinding::new("p", Paste, vim),
        KeyBinding::new("x", Delete, vim),
        KeyBinding::new("r", RenameStart, vim),
        KeyBinding::new("a", NewFile, vim),
        KeyBinding::new("shift-a", NewDir, vim),
        KeyBinding::new("u", Undo, vim),
        KeyBinding::new("v", VisualToggle, vim),
        KeyBinding::new("space", ToggleMark, vim),
        KeyBinding::new("y p", YankPath, vim),
        KeyBinding::new("y n", YankName, vim),
        KeyBinding::new("y r", YankRel, vim),
        KeyBinding::new("e", OpenEditor, vim),
        KeyBinding::new("t", OpenTerminal, vim),
        KeyBinding::new("o", OpenDefault, vim),
        KeyBinding::new("s", ComputeSizes, vim),
        KeyBinding::new("g d", Disks, vim),
        KeyBinding::new("g t", TrashFolder, vim),
        KeyBinding::new("g h", GoHome, vim),
        KeyBinding::new("g z", FinderFolders, vim),
        KeyBinding::new("ctrl-c", CancelCommand, vim),
        KeyBinding::new("]", TabNext, vim),
        KeyBinding::new("[", TabPrev, vim),
        // Campos de texto.
        KeyBinding::new("enter", ConfirmPath, Some(PATH_CONTEXT)),
        KeyBinding::new("escape", CancelEdit, Some(PATH_CONTEXT)),
        KeyBinding::new("enter", ConfirmPrompt, Some(PROMPT_CONTEXT)),
        KeyBinding::new("escape", CancelPrompt, Some(PROMPT_CONTEXT)),
        // Buscador y lista de discos.
        KeyBinding::new("down", OverlayDown, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-n", OverlayDown, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("up", OverlayUp, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-p", OverlayUp, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("enter", OverlayConfirm, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("escape", OverlayCancel, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}

/// Añade los atajos del usuario. Van después de los de fábrica, así que los reemplazan. Devuelve
/// un aviso por cada atajo que no se pudo usar.
pub fn bind_user(cx: &mut App, binds: &[UserBind]) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut ready = Vec::new();
    for bind in binds {
        let context = match bind.scope {
            BindScope::All => "FileList",
            BindScope::Vim => "FileList && vim",
            BindScope::Explorer => "FileList && explorer",
        };
        // `KeyBinding::new` se detiene con una tecla mal escrita: se valida antes.
        let valid = bind.keys.split(' ').all(|k| Keystroke::parse(k).is_ok());
        match (valid, binding(&bind.action, &bind.keys, context)) {
            (true, Some(binding)) => ready.push(binding),
            (false, _) => warnings.push(format!("atajo no válido: «{}»", bind.keys)),
            (_, None) => warnings.push(format!("acción desconocida: «{}»", bind.action)),
        }
    }
    cx.bind_keys(ready);
    warnings
}
