use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A qué apunta un enlace simbólico.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    Dir,
    Other,
    Broken,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
    Symlink(LinkTarget),
}

/// Categoría usada solo para colorear el nombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Dir,
    Image,
    Code,
    Doc,
    Media,
    Archive,
    Other,
    Broken,
}

/// Una entrada de carpeta. El nombre es `OsString`: en Linux no tiene por qué ser UTF-8.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: OsString,
    pub kind: Kind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// Bits de `st_mode`, incluido el tipo de archivo.
    pub mode: u32,
    /// Destino del enlace simbólico, tal como está escrito en el enlace.
    pub link: Option<PathBuf>,
}

impl Entry {
    /// Si se puede entrar en ella: una carpeta o un enlace a una carpeta.
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, Kind::Dir | Kind::Symlink(LinkTarget::Dir))
    }

    pub fn is_hidden(&self) -> bool {
        self.name.as_encoded_bytes().first() == Some(&b'.')
    }

    pub fn class(&self) -> Class {
        if self.kind == Kind::Symlink(LinkTarget::Broken) {
            return Class::Broken;
        }
        if self.is_dir() {
            return Class::Dir;
        }
        let ext = Path::new(&self.name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" | "avif" => {
                Class::Image
            }
            "rs" | "toml" | "md" | "py" | "js" | "ts" | "tsx" | "jsx" | "json" | "yaml" | "yml"
            | "c" | "h" | "cpp" | "hpp" | "go" | "java" | "sh" | "html" | "css" | "sql"
            | "lock" => Class::Code,
            "pdf" | "txt" | "doc" | "docx" | "odt" | "rtf" | "xls" | "xlsx" | "ods" | "ppt"
            | "pptx" | "csv" => Class::Doc,
            "mp3" | "flac" | "wav" | "ogg" | "m4a" | "mp4" | "mkv" | "webm" | "avi" | "mov" => {
                Class::Media
            }
            "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "zst" | "7z" | "rar" | "iso" | "deb"
            | "rpm" => Class::Archive,
            _ => Class::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> Entry {
        Entry {
            name: name.into(),
            kind: Kind::File,
            size: 0,
            modified: None,
            mode: 0o100644,
            link: None,
        }
    }

    #[test]
    fn classifies_by_extension() {
        assert_eq!(file("a.PNG").class(), Class::Image);
        assert_eq!(file("lib.rs").class(), Class::Code);
        assert_eq!(file("x.tar.gz").class(), Class::Archive);
        assert_eq!(file("noext").class(), Class::Other);
    }

    #[test]
    fn broken_links_win_over_everything() {
        let mut e = file("foo.png");
        e.kind = Kind::Symlink(LinkTarget::Broken);
        assert_eq!(e.class(), Class::Broken);
        assert!(!e.is_dir());
    }

    #[test]
    fn detects_hidden() {
        assert!(file(".bashrc").is_hidden());
        assert!(!file("bashrc").is_hidden());
    }
}
