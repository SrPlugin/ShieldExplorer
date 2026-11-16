use std::io;

/// Por qué no se pudo leer una carpeta.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("No existe")]
    NotFound,
    #[error("Permiso denegado")]
    PermissionDenied,
    #[error("No es una carpeta")]
    NotADirectory,
    #[error("Error de entrada/salida: {0}")]
    Io(#[source] io::Error),
}

impl FsError {
    pub fn from_io(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::NotADirectory => Self::NotADirectory,
            _ => Self::Io(error),
        }
    }

    /// Código de errno de Linux, tal como lo mostraría una terminal.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "ENOENT",
            Self::PermissionDenied => "EACCES",
            Self::NotADirectory => "ENOTDIR",
            Self::Io(_) => "EIO",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_io_kinds_to_codes() {
        let code = |kind| FsError::from_io(io::Error::from(kind)).code();
        assert_eq!(code(io::ErrorKind::NotFound), "ENOENT");
        assert_eq!(code(io::ErrorKind::PermissionDenied), "EACCES");
        assert_eq!(code(io::ErrorKind::NotADirectory), "ENOTDIR");
        assert_eq!(code(io::ErrorKind::Other), "EIO");
    }
}
