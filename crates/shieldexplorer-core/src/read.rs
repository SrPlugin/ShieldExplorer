use std::fs::{self, DirEntry};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::entry::{Entry, Kind, LinkTarget};
use crate::error::FsError;

/// Contenido de una carpeta. Las entradas que no se pudieron leer no tumban el listado: se
/// cuentan aparte para poder avisar al usuario.
#[derive(Debug)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub unreadable: usize,
}

/// Lee una carpeta. Bloquea: llamarlo fuera del hilo de la interfaz.
pub fn read_dir(path: &Path) -> Result<Listing, FsError> {
    let items = fs::read_dir(path).map_err(FsError::from_io)?;
    let mut entries = Vec::new();
    let mut unreadable = 0;
    for item in items {
        match item.and_then(|item| entry_of(&item)) {
            Ok(entry) => entries.push(entry),
            Err(_) => unreadable += 1,
        }
    }
    Ok(Listing {
        entries,
        unreadable,
    })
}

fn entry_of(item: &DirEntry) -> io::Result<Entry> {
    // No sigue enlaces simbólicos: describe la entrada tal cual está en la carpeta.
    let meta = item.metadata()?;
    let kind = if meta.is_symlink() {
        Kind::Symlink(match fs::metadata(item.path()) {
            Ok(target) if target.is_dir() => LinkTarget::Dir,
            Ok(_) => LinkTarget::Other,
            Err(_) => LinkTarget::Broken,
        })
    } else if meta.is_dir() {
        Kind::Dir
    } else {
        Kind::File
    };
    Ok(Entry {
        name: item.file_name(),
        kind,
        size: meta.len(),
        modified: meta.modified().ok(),
        mode: meta.mode(),
        link: meta
            .is_symlink()
            .then(|| fs::read_link(item.path()).ok())
            .flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn find<'a>(listing: &'a Listing, name: &str) -> &'a Entry {
        listing.entries.iter().find(|e| e.name == name).unwrap()
    }

    #[test]
    fn lists_files_dirs_and_links() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("a.txt"), b"hello").unwrap();
        fs::create_dir(root.join("sub")).unwrap();
        symlink(root.join("sub"), root.join("to_dir")).unwrap();
        symlink(root.join("missing"), root.join("dead")).unwrap();

        let listing = read_dir(root).unwrap();
        assert_eq!(listing.unreadable, 0);
        assert_eq!(listing.entries.len(), 4);
        assert_eq!(find(&listing, "a.txt").size, 5);
        assert_eq!(find(&listing, "sub").kind, Kind::Dir);
        assert_eq!(
            find(&listing, "to_dir").kind,
            Kind::Symlink(LinkTarget::Dir)
        );
        assert!(find(&listing, "to_dir").is_dir());
        assert_eq!(
            find(&listing, "dead").kind,
            Kind::Symlink(LinkTarget::Broken)
        );
    }

    #[test]
    fn empty_dir_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_dir(tmp.path()).unwrap().entries.is_empty());
    }

    #[test]
    fn keeps_non_utf8_names() {
        let tmp = tempfile::tempdir().unwrap();
        let name = OsStr::from_bytes(b"caf\xe9.txt");
        fs::write(tmp.path().join(name), b"").unwrap();
        let listing = read_dir(tmp.path()).unwrap();
        assert_eq!(listing.entries[0].name, name);
    }

    #[test]
    fn reports_missing_and_not_a_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let err = read_dir(&tmp.path().join("nope")).unwrap_err();
        assert_eq!(err.code(), "ENOENT");
        let file = tmp.path().join("f");
        fs::write(&file, b"").unwrap();
        assert_eq!(read_dir(&file).unwrap_err().code(), "ENOTDIR");
    }

    #[test]
    fn reports_permission_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let locked = tmp.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let result = read_dir(&locked);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        // Con root el sistema ignora los permisos: en ese caso no hay nada que comprobar.
        if let Err(err) = result {
            assert_eq!(err.code(), "EACCES");
        }
    }
}
