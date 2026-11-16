//! Tamaño real de una carpeta (suma recursiva), cancelable.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Suma el tamaño de todo lo que hay bajo `path`. No sigue enlaces simbólicos y se salta lo que no
/// puede leer. Devuelve `None` si `cancel` se activó antes de terminar.
pub fn dir_size(path: &Path, cancel: &AtomicBool) -> Option<u64> {
    let mut total = 0u64;
    let mut pending = vec![path.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(items) = fs::read_dir(&dir) else {
            continue;
        };
        for item in items.flatten() {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let Ok(meta) = item.metadata() else {
                continue;
            };
            if meta.is_dir() {
                pending.push(item.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn sums_recursively_without_following_links() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("x"), vec![0u8; 100]).unwrap();
        fs::write(root.join("a/y"), vec![0u8; 50]).unwrap();
        fs::write(root.join("a/b/z"), vec![0u8; 7]).unwrap();
        symlink(root, root.join("a/lazo")).unwrap(); // enlace a la raíz: sin bucle
        let size = dir_size(root, &AtomicBool::new(false)).unwrap();
        assert!((157..157 + 64).contains(&size), "tamaño inesperado: {size}"); // + el enlace
    }

    #[test]
    fn cancelling_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("f"), "x").unwrap();
        assert_eq!(dir_size(tmp.path(), &AtomicBool::new(true)), None);
    }
}
