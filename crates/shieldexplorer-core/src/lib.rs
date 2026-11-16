//! Lógica del Explorer sin interfaz gráfica: leer carpetas, ordenar, historial y rutas.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod command;
mod disks;
mod du;
mod entry;
mod error;
mod finder;
mod format;
mod git;
mod history;
mod launch;
mod memory;
mod ops;
mod path;
mod preview;
mod read;
mod search;
mod shell;
mod sort;

pub use command::{Command, ModeArg, YankKind, parse as parse_command};
pub use disks::{Disk, list as list_disks};
pub use du::dir_size;
pub use entry::{Class, Entry, Kind, LinkTarget};
pub use error::FsError;
pub use finder::{MAX_FILES, fuzzy_score, index_files, project_root, rank};
pub use format::{format_size, mode_string};
pub use git::{Branch, GitInfo, GitMark, read as read_git};
pub use history::History;
pub use launch::{open_default, open_editor, open_terminal, split_command};
pub use memory::Memory;
pub use ops::{
    OpError, Trashed, Undo, copy_recursive, create_dir, create_file, paste_copy, paste_move,
    read_trash_info, rename, restore, trash, undo, unique_path,
};
pub use path::{breadcrumbs, display_path, resolve};
pub use preview::{Preview, load as load_preview};
pub use read::{Listing, read_dir};
pub use search::{find_match, find_prefix};
pub use shell::{Running, ShellEvent, spawn as spawn_shell};
pub use sort::{Sort, SortKey, sort_entries};
