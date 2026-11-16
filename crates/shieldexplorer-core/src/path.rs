use std::path::{Component, Path, PathBuf};

/// Convierte lo que el usuario escribe en una ruta absoluta y normalizada.
///
/// Acepta `~`, `~/x`, rutas relativas a `cwd`, `.` y `..`. La normalización es léxica: `..` quita
/// el último componente sin mirar enlaces simbólicos.
pub fn resolve(input: &str, cwd: &Path, home: Option<&Path>) -> PathBuf {
    let input = input.trim();
    let joined = match (input, home) {
        ("~", Some(home)) => home.to_path_buf(),
        (_, Some(home)) if input.starts_with("~/") => home.join(&input[2..]),
        _ => cwd.join(input),
    };
    let mut out = PathBuf::from("/");
    for component in joined.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    out
}

/// Ruta para mostrar: el directorio personal se abrevia como `~`.
pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/u";

    fn r(input: &str) -> PathBuf {
        resolve(input, Path::new("/home/u/Desktop"), Some(Path::new(HOME)))
    }

    #[test]
    fn resolves_home_relative_and_dots() {
        assert_eq!(r("~"), Path::new("/home/u"));
        assert_eq!(r("~/docs"), Path::new("/home/u/docs"));
        assert_eq!(r("src"), Path::new("/home/u/Desktop/src"));
        assert_eq!(r(".."), Path::new("/home/u"));
        assert_eq!(r("./a/../b"), Path::new("/home/u/Desktop/b"));
        assert_eq!(r("/etc//ssh/"), Path::new("/etc/ssh"));
        assert_eq!(r(""), Path::new("/home/u/Desktop"));
        assert_eq!(r("/../.."), Path::new("/"));
    }

    #[test]
    fn tilde_without_home_is_relative() {
        let cwd = Path::new("/x");
        assert_eq!(resolve("~", cwd, None), Path::new("/x/~"));
    }

    #[test]
    fn displays_home_as_tilde() {
        let home = Some(Path::new(HOME));
        assert_eq!(display_path(Path::new("/home/u"), home), "~");
        assert_eq!(display_path(Path::new("/home/u/a/b"), home), "~/a/b");
        assert_eq!(display_path(Path::new("/home/ux"), home), "/home/ux");
        assert_eq!(display_path(Path::new("/etc"), None), "/etc");
    }
}
