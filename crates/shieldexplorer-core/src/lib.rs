//! Lógica del Explorer sin interfaz gráfica: leer carpetas, ordenar, historial y rutas.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod entry;
mod error;
mod format;
mod history;
mod path;
mod read;
mod sort;

pub use entry::{Class, Entry, Kind, LinkTarget};
pub use error::FsError;
pub use format::{format_size, mode_string};
pub use history::History;
pub use path::{display_path, resolve};
pub use read::{Listing, read_dir};
pub use sort::{Sort, SortKey, sort_entries};
