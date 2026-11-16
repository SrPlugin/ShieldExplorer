use crate::entry::Entry;

/// Busca `pattern` dentro del nombre, a partir de la fila `from` (inclusive) y dando la vuelta al
/// llegar al final. Con `forward = false` recorre hacia atrás.
///
/// Como en vim ("smartcase"): sin mayúsculas en el patrón no distingue mayúsculas; con alguna, sí.
pub fn find_match(entries: &[Entry], pattern: &str, from: usize, forward: bool) -> Option<usize> {
    if pattern.is_empty() || entries.is_empty() {
        return None;
    }
    let n = entries.len();
    let from = from % n;
    let case_sensitive = pattern.chars().any(char::is_uppercase);
    let needle = if case_sensitive {
        pattern.to_owned()
    } else {
        pattern.to_lowercase()
    };
    (0..n)
        .map(|k| {
            if forward {
                (from + k) % n
            } else {
                (from + n - k) % n
            }
        })
        .find(|&i| {
            let name = entries[i].name.to_string_lossy();
            if case_sensitive {
                name.contains(&needle)
            } else {
                name.to_lowercase().contains(&needle)
            }
        })
}

/// Primera entrada cuyo nombre empieza por `prefix` (sin distinguir mayúsculas), a partir de la
/// fila `from` (inclusive) y dando la vuelta. Sirve para escribir y saltar, como en un explorador.
pub fn find_prefix(entries: &[Entry], prefix: &str, from: usize) -> Option<usize> {
    if prefix.is_empty() || entries.is_empty() {
        return None;
    }
    let n = entries.len();
    let prefix = prefix.to_lowercase();
    (0..n).map(|k| (from % n + k) % n).find(|&i| {
        entries[i]
            .name
            .to_string_lossy()
            .to_lowercase()
            .starts_with(&prefix)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::Kind;

    fn list(names: &[&str]) -> Vec<Entry> {
        names
            .iter()
            .map(|n| Entry {
                name: (*n).into(),
                kind: Kind::File,
                size: 0,
                modified: None,
                mode: 0,
            })
            .collect()
    }

    #[test]
    fn smartcase_search() {
        let v = list(&["Readme.md", "main.rs", "MAIN.txt"]);
        assert_eq!(find_match(&v, "main", 0, true), Some(1));
        assert_eq!(find_match(&v, "MAIN", 0, true), Some(2));
        assert_eq!(find_match(&v, "Main", 0, true), None);
    }

    #[test]
    fn search_wraps_in_both_directions() {
        let v = list(&["a.rs", "b.txt", "c.rs", "d.txt"]);
        assert_eq!(find_match(&v, ".rs", 3, true), Some(0));
        assert_eq!(find_match(&v, ".rs", 1, false), Some(0));
        assert_eq!(find_match(&v, ".rs", 0, false), Some(0));
        assert_eq!(find_match(&v, ".rs", 1, true), Some(2));
        assert_eq!(find_match(&v, ".rs", 9, true), Some(2)); // `from` fuera de rango se ajusta
    }

    #[test]
    fn empty_pattern_or_list_finds_nothing() {
        assert_eq!(find_match(&list(&["a"]), "", 0, true), None);
        assert_eq!(find_match(&[], "a", 0, true), None);
        assert_eq!(find_prefix(&[], "a", 0), None);
    }

    #[test]
    fn prefix_ignores_case_and_wraps() {
        let v = list(&["Cargo.toml", "docs", "Desktop", "src"]);
        assert_eq!(find_prefix(&v, "d", 0), Some(1));
        assert_eq!(find_prefix(&v, "d", 2), Some(2));
        assert_eq!(find_prefix(&v, "ca", 3), Some(0));
        assert_eq!(find_prefix(&v, "zz", 0), None);
    }
}
