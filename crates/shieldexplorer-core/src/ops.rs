//! Operaciones sobre archivos: copiar, mover, papelera, renombrar, crear y deshacer.
//!
//! Nada sobrescribe: si el destino existe se elige un nombre libre (`nombre copia.ext`). Los
//! borrados van a la papelera de freedesktop (`~/.local/share/Trash`), así se pueden deshacer.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("{path}: {source}")]
    Io { path: String, source: io::Error },
    #[error("ya existe: {0}")]
    Exists(String),
    #[error("{0}")]
    Invalid(&'static str),
}

fn io_err(path: &Path, source: io::Error) -> OpError {
    OpError::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Un elemento enviado a la papelera, con lo necesario para restaurarlo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trashed {
    pub original: PathBuf,
    pub stored: PathBuf,
    pub info: PathBuf,
}

/// Cómo deshacer lo último que se hizo.
#[derive(Clone, Debug)]
pub enum Undo {
    /// Se crearon o copiaron estos elementos: deshacer los manda a la papelera.
    Created(Vec<PathBuf>),
    Renamed {
        from: PathBuf,
        to: PathBuf,
    },
    Moved(Vec<(PathBuf, PathBuf)>),
    Trashed(Vec<Trashed>),
}

// --- Nombres ---------------------------------------------------------------------------------

/// Primer nombre libre en `dir`: `nombre`, `nombre copia`, `nombre copia 2`… (antes de la extensión).
pub fn unique_path(dir: &Path, name: &OsStr) -> PathBuf {
    let first = dir.join(name);
    if fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .unwrap_or(name)
        .to_string_lossy()
        .into_owned();
    // Un nombre como `.bashrc` no tiene extensión: `file_stem` lo devuelve entero.
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 1u32.. {
        let label = if n == 1 {
            " copia".to_owned()
        } else {
            format!(" copia {n}")
        };
        let candidate = dir.join(format!("{stem}{label}{ext}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    first // inalcanzable en la práctica: u32 entero de nombres ocupados
}

/// Valida un nombre escrito por el usuario: relativo, sin `..` y sin componentes vacíos.
fn clean_relative(name: &str) -> Result<PathBuf, OpError> {
    let path = Path::new(name.trim());
    if path.as_os_str().is_empty() {
        return Err(OpError::Invalid("el nombre está vacío"));
    }
    let ok = path.components().all(|c| matches!(c, Component::Normal(_)));
    if !ok {
        return Err(OpError::Invalid(
            "el nombre no puede ser absoluto ni contener . o ..",
        ));
    }
    Ok(path.to_path_buf())
}

// --- Copiar y mover ---------------------------------------------------------------------------

/// Copia un archivo, enlace o carpeta (recursivo). Los enlaces se copian como enlaces.
pub fn copy_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.is_symlink() {
        symlink(fs::read_link(src)?, dst)
    } else if meta.is_dir() {
        fs::create_dir(dst)?;
        fs::set_permissions(dst, meta.permissions())?;
        for item in fs::read_dir(src)? {
            let item = item?;
            copy_recursive(&item.path(), &dst.join(item.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}

fn remove_recursive(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Mueve con `rename` y, si cruza de sistema de archivos, copia y borra el original.
fn move_path(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            copy_recursive(src, dst)?;
            remove_recursive(src)
        }
        other => other,
    }
}

fn check_not_into_itself(src: &Path, dest_dir: &Path) -> Result<(), OpError> {
    if dest_dir.starts_with(src) {
        return Err(OpError::Invalid(
            "no se puede copiar o mover una carpeta dentro de sí misma",
        ));
    }
    Ok(())
}

/// Copia `sources` dentro de `dest_dir`. Devuelve las rutas creadas.
pub fn paste_copy(sources: &[PathBuf], dest_dir: &Path) -> Result<Vec<PathBuf>, OpError> {
    let mut created = Vec::new();
    for src in sources {
        check_not_into_itself(src, dest_dir)?;
        let name = src.file_name().ok_or(OpError::Invalid("ruta sin nombre"))?;
        let dst = unique_path(dest_dir, name);
        copy_recursive(src, &dst).map_err(|e| io_err(src, e))?;
        created.push(dst);
    }
    Ok(created)
}

/// Mueve `sources` a `dest_dir`. Lo que ya está allí se deja como está. Devuelve `(origen, destino)`.
pub fn paste_move(
    sources: &[PathBuf],
    dest_dir: &Path,
) -> Result<Vec<(PathBuf, PathBuf)>, OpError> {
    let mut moved = Vec::new();
    for src in sources {
        if src.parent() == Some(dest_dir) {
            continue;
        }
        check_not_into_itself(src, dest_dir)?;
        let name = src.file_name().ok_or(OpError::Invalid("ruta sin nombre"))?;
        let dst = unique_path(dest_dir, name);
        move_path(src, &dst).map_err(|e| io_err(src, e))?;
        moved.push((src.clone(), dst));
    }
    Ok(moved)
}

// --- Renombrar y crear ------------------------------------------------------------------------

pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf, OpError> {
    let new_name = new_name.trim();
    if new_name.is_empty() || new_name.contains('/') || new_name == "." || new_name == ".." {
        return Err(OpError::Invalid("nombre no válido"));
    }
    let target = path.with_file_name(new_name);
    if target == path {
        return Ok(target);
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(OpError::Exists(target.display().to_string()));
    }
    fs::rename(path, &target).map_err(|e| io_err(path, e))?;
    Ok(target)
}

/// Crea un archivo vacío (y las carpetas intermedias si el nombre las lleva).
pub fn create_file(dir: &Path, name: &str) -> Result<PathBuf, OpError> {
    let target = dir.join(clean_relative(name)?);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                OpError::Exists(target.display().to_string())
            } else {
                io_err(&target, e)
            }
        })?;
    Ok(target)
}

/// Crea una carpeta (con las intermedias, como `mkdir -p`). Falla si ya existe.
pub fn create_dir(dir: &Path, name: &str) -> Result<PathBuf, OpError> {
    let target = dir.join(clean_relative(name)?);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(OpError::Exists(target.display().to_string()));
    }
    fs::create_dir_all(&target).map_err(|e| io_err(&target, e))?;
    Ok(target)
}

// --- Papelera (freedesktop.org Trash spec, papelera del directorio personal) -----------------

/// Codifica una ruta como pide `.trashinfo`: todo salvo letras, cifras, `-_.~` y `/`.
fn percent_encode(path: &Path) -> String {
    let mut out = String::new();
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn percent_decode(text: &str) -> PathBuf {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).and_then(|&c| hex(c)),
            bytes.get(i + 2).and_then(|&c| hex(c)),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (b, ..) => {
                out.push(b);
                i += 1;
            }
        }
    }
    PathBuf::from(OsString::from(OsStr::from_bytes(&out)))
}

/// Envía `path` a la papelera `trash_root`. `date` es `AAAA-MM-DDThh:mm:ss` en hora local.
pub fn trash(path: &Path, trash_root: &Path, date: &str) -> Result<Trashed, OpError> {
    let original = fs::canonicalize(path.parent().unwrap_or(Path::new("/")))
        .map_err(|e| io_err(path, e))?
        .join(
            path.file_name()
                .ok_or(OpError::Invalid("ruta sin nombre"))?,
        );
    let files = trash_root.join("files");
    let infos = trash_root.join("info");
    for dir in [&files, &infos] {
        fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    }
    let name = original.file_name().unwrap_or_default();
    // Un nombre libre en `files` y en `info`: se reserva con el `.trashinfo`.
    let stored = unique_path(&files, name);
    let stored_name = stored.file_name().unwrap_or_default().to_owned();
    let info = infos.join(format!("{}.trashinfo", stored_name.to_string_lossy()));
    let body = format!(
        "[Trash Info]\nPath={}\nDeletionDate={date}\n",
        percent_encode(&original)
    );
    fs::write(&info, body).map_err(|e| io_err(&info, e))?;
    if let Err(e) = move_path(&original, &stored) {
        let _ = fs::remove_file(&info);
        return Err(io_err(&original, e));
    }
    Ok(Trashed {
        original,
        stored,
        info,
    })
}

/// Lee un `.trashinfo` y devuelve la ruta original.
pub fn read_trash_info(info: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(info).ok()?;
    let value = text.lines().find_map(|l| l.strip_prefix("Path="))?;
    Some(percent_decode(value.trim()))
}

/// Devuelve un elemento de la papelera a su sitio (con otro nombre si el original está ocupado).
pub fn restore(item: &Trashed) -> Result<PathBuf, OpError> {
    let dir = item.original.parent().unwrap_or(Path::new("/"));
    fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    let name = item
        .original
        .file_name()
        .ok_or(OpError::Invalid("ruta sin nombre"))?;
    let target = unique_path(dir, name);
    move_path(&item.stored, &target).map_err(|e| io_err(&item.stored, e))?;
    let _ = fs::remove_file(&item.info);
    Ok(target)
}

// --- Deshacer --------------------------------------------------------------------------------

/// Deshace una operación y devuelve un resumen. Las que se revierten con la papelera usan `date`.
pub fn undo(action: &Undo, trash_root: &Path, date: &str) -> Result<String, OpError> {
    match action {
        Undo::Created(paths) => {
            for p in paths {
                trash(p, trash_root, date)?;
            }
            Ok(format!(
                "deshecho: {} elemento(s) a la papelera",
                paths.len()
            ))
        }
        Undo::Renamed { from, to } => {
            move_path(to, from).map_err(|e| io_err(to, e))?;
            Ok("deshecho: renombrado".to_owned())
        }
        Undo::Moved(pairs) => {
            for (from, to) in pairs {
                move_path(to, from).map_err(|e| io_err(to, e))?;
            }
            Ok(format!("deshecho: {} elemento(s) devueltos", pairs.len()))
        }
        Undo::Trashed(items) => {
            for item in items {
                restore(item)?;
            }
            Ok(format!("deshecho: {} elemento(s) restaurados", items.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::write(path, text).unwrap();
    }

    #[test]
    fn picks_free_names() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        write(&d.join("a.txt"), "1");
        assert_eq!(unique_path(d, OsStr::new("b.txt")), d.join("b.txt"));
        assert_eq!(unique_path(d, OsStr::new("a.txt")), d.join("a copia.txt"));
        write(&d.join("a copia.txt"), "2");
        assert_eq!(unique_path(d, OsStr::new("a.txt")), d.join("a copia 2.txt"));
        write(&d.join(".bashrc"), "");
        assert_eq!(
            unique_path(d, OsStr::new(".bashrc")),
            d.join(".bashrc copia")
        );
    }

    #[test]
    fn copies_trees_and_links() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(src.join("sub")).unwrap();
        write(&src.join("sub/f.txt"), "hola");
        symlink("sub/f.txt", src.join("link")).unwrap();
        let dest = tmp.path().join("dest");
        fs::create_dir(&dest).unwrap();

        let created = paste_copy(&[src.clone()], &dest).unwrap();
        assert_eq!(created, [dest.join("src")]);
        assert_eq!(
            fs::read_to_string(dest.join("src/sub/f.txt")).unwrap(),
            "hola"
        );
        assert_eq!(
            fs::read_link(dest.join("src/link")).unwrap(),
            Path::new("sub/f.txt")
        );
        // Copiar otra vez en el mismo sitio no pisa nada.
        assert_eq!(paste_copy(&[src], &dest).unwrap(), [dest.join("src copia")]);
    }

    #[test]
    fn refuses_copying_a_folder_into_itself() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("a");
        fs::create_dir_all(src.join("b")).unwrap();
        assert!(paste_copy(&[src.clone()], &src.join("b")).is_err());
        assert!(paste_move(&[src.clone()], &src.join("b")).is_err());
    }

    #[test]
    fn moves_and_undoes() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        write(&a.join("f"), "x");
        let moved = paste_move(&[a.join("f")], &b).unwrap();
        assert_eq!(moved, [(a.join("f"), b.join("f"))]);
        assert!(!a.join("f").exists());
        undo(&Undo::Moved(moved), tmp.path(), "").unwrap();
        assert!(a.join("f").exists() && !b.join("f").exists());
        // Mover a la carpeta donde ya está no hace nada.
        assert!(paste_move(&[a.join("f")], &a).unwrap().is_empty());
    }

    #[test]
    fn renames_with_validation() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        write(&d.join("a"), "");
        write(&d.join("b"), "");
        assert!(matches!(rename(&d.join("a"), "b"), Err(OpError::Exists(_))));
        assert!(matches!(
            rename(&d.join("a"), "x/y"),
            Err(OpError::Invalid(_))
        ));
        assert!(matches!(
            rename(&d.join("a"), ".."),
            Err(OpError::Invalid(_))
        ));
        let new = rename(&d.join("a"), "c").unwrap();
        assert!(new.exists() && !d.join("a").exists());
        undo(
            &Undo::Renamed {
                from: d.join("a"),
                to: new,
            },
            d,
            "",
        )
        .unwrap();
        assert!(d.join("a").exists());
    }

    #[test]
    fn creates_nested_and_refuses_unsafe_names() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        let f = create_file(d, "x/y/z.txt").unwrap();
        assert!(f.is_file());
        assert!(matches!(
            create_file(d, "x/y/z.txt"),
            Err(OpError::Exists(_))
        ));
        assert!(create_dir(d, "../fuera").is_err());
        assert!(create_dir(d, "/abs").is_err());
        assert!(create_dir(d, "  ").is_err());
        assert!(create_dir(d, "n1/n2").unwrap().is_dir());
        assert!(matches!(create_dir(d, "n1"), Err(OpError::Exists(_))));
    }

    #[test]
    fn trash_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let trash_root = tmp.path().join("Trash");
        let victim = tmp.path().join("año 1%.txt");
        write(&victim, "datos");

        let item = trash(&victim, &trash_root, "2026-11-16T10:00:00").unwrap();
        assert!(!victim.exists());
        assert_eq!(fs::read_to_string(&item.stored).unwrap(), "datos");
        let info = fs::read_to_string(&item.info).unwrap();
        assert!(info.contains("DeletionDate=2026-11-16T10:00:00"));
        assert!(info.contains("%C3%B1")); // la ñ va codificada
        assert_eq!(read_trash_info(&item.info).unwrap(), item.original);

        // Con el nombre original ocupado, se restaura con otro nombre.
        write(&victim, "nuevo");
        let back = restore(&item).unwrap();
        assert_eq!(back, tmp.path().join("año 1% copia.txt"));
        assert_eq!(fs::read_to_string(back).unwrap(), "datos");
        assert!(!item.info.exists());
    }

    #[test]
    fn undo_of_a_trash_restores_and_undo_of_a_copy_trashes() {
        let tmp = tempfile::tempdir().unwrap();
        let trash_root = tmp.path().join("Trash");
        let f = tmp.path().join("f");
        write(&f, "1");
        let item = trash(&f, &trash_root, "d").unwrap();
        undo(&Undo::Trashed(vec![item]), &trash_root, "d").unwrap();
        assert!(f.exists());
        undo(&Undo::Created(vec![f.clone()]), &trash_root, "d").unwrap();
        assert!(!f.exists());
    }

    #[test]
    fn percent_round_trip_keeps_odd_bytes() {
        let odd = PathBuf::from(OsStr::from_bytes(b"/a b/caf\xe9%x"));
        assert_eq!(percent_decode(&percent_encode(&odd)), odd);
    }
}
