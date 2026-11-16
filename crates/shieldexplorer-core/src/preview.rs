//! Vista previa de lo seleccionado: contenido de una carpeta, texto, imagen o archivo binario.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

/// Cuánto leer de un archivo para la vista previa.
pub const MAX_BYTES: usize = 64 * 1024;
pub const MAX_LINES: usize = 200;
pub const MAX_ENTRIES: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preview {
    Dir {
        entries: Vec<(String, bool)>,
        total: usize,
    },
    Text {
        lines: Vec<String>,
        truncated: bool,
    },
    Image,
    Binary {
        size: u64,
    },
    Empty,
    Error(String),
}

fn is_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tiff"
    )
}

/// Carga la vista previa de `path`. Bloquea: llamarlo en segundo plano.
pub fn load(path: &Path) -> Preview {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Preview::Error(e.to_string()),
    };
    if meta.is_dir() {
        return load_dir(path);
    }
    if is_image(path) {
        return Preview::Image;
    }
    if !meta.is_file() {
        return Preview::Binary { size: meta.len() };
    }
    let mut buf = Vec::with_capacity(MAX_BYTES.min(meta.len() as usize));
    let read = File::open(path).and_then(|f| f.take(MAX_BYTES as u64).read_to_end(&mut buf));
    if let Err(e) = read {
        return Preview::Error(e.to_string());
    }
    if buf.is_empty() {
        return Preview::Empty;
    }
    if buf.contains(&0) {
        return Preview::Binary { size: meta.len() };
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text
        .lines()
        .take(MAX_LINES + 1)
        .map(|l| l.trim_end_matches('\r').to_owned())
        .collect();
    let truncated = lines.len() > MAX_LINES || meta.len() > buf.len() as u64;
    lines.truncate(MAX_LINES);
    Preview::Text { lines, truncated }
}

fn load_dir(path: &Path) -> Preview {
    let items = match fs::read_dir(path) {
        Ok(i) => i,
        Err(e) => return Preview::Error(e.to_string()),
    };
    let mut entries: Vec<(String, bool)> = items
        .flatten()
        .map(|i| {
            let is_dir = i.file_type().is_ok_and(|t| t.is_dir())
                || (i.file_type().is_ok_and(|t| t.is_symlink()) && i.path().is_dir());
            (i.file_name().to_string_lossy().into_owned(), is_dir)
        })
        .collect();
    let total = entries.len();
    entries.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    entries.truncate(MAX_ENTRIES);
    Preview::Dir { entries, total }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_text_with_a_line_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("a.txt");
        fs::write(&f, "uno\r\ndos\n").unwrap();
        assert_eq!(
            load(&f),
            Preview::Text {
                lines: vec!["uno".into(), "dos".into()],
                truncated: false
            }
        );
        let long = (0..500).map(|i| format!("l{i}\n")).collect::<String>();
        fs::write(&f, long).unwrap();
        let Preview::Text { lines, truncated } = load(&f) else {
            panic!("no es texto")
        };
        assert_eq!((lines.len(), truncated), (MAX_LINES, true));
    }

    #[test]
    fn detects_binary_empty_and_images() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("b.dat");
        fs::write(&bin, [1u8, 0, 2]).unwrap();
        assert_eq!(load(&bin), Preview::Binary { size: 3 });
        let empty = tmp.path().join("e");
        fs::write(&empty, "").unwrap();
        assert_eq!(load(&empty), Preview::Empty);
        let img = tmp.path().join("x.PNG");
        fs::write(&img, "no importa").unwrap();
        assert_eq!(load(&img), Preview::Image);
    }

    #[test]
    fn caps_big_files_by_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("big");
        fs::write(&f, "x".repeat(MAX_BYTES * 2)).unwrap();
        let Preview::Text { lines, truncated } = load(&f) else {
            panic!("no es texto")
        };
        assert!(truncated);
        assert_eq!(lines[0].len(), MAX_BYTES);
    }

    #[test]
    fn lists_a_folder_dirs_first() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("Zeta")).unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();
        fs::write(tmp.path().join("B.txt"), "").unwrap();
        let Preview::Dir { entries, total } = load(tmp.path()) else {
            panic!("no es carpeta")
        };
        assert_eq!(total, 3);
        let names: Vec<_> = entries.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["Zeta", "a.txt", "B.txt"]);
        assert!(matches!(load(&tmp.path().join("no")), Preview::Error(_)));
    }
}
