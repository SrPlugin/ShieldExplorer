use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

use crate::entry::Entry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub key: SortKey,
    pub ascending: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            ascending: true,
        }
    }
}

impl Sort {
    /// Clic en un encabezado: la misma columna invierte el sentido, otra empieza ascendente.
    pub fn toggled(self, key: SortKey) -> Self {
        if self.key == key {
            Self {
                key,
                ascending: !self.ascending,
            }
        } else {
            Self {
                key,
                ascending: true,
            }
        }
    }
}

/// Ordena con las carpetas primero, también en sentido descendente.
pub fn sort_entries(entries: &mut [Entry], sort: Sort) {
    entries.sort_by(|a, b| {
        let by_key = match sort.key {
            SortKey::Name => Ordering::Equal,
            SortKey::Size => a.size.cmp(&b.size),
            SortKey::Modified => a.modified.cmp(&b.modified),
        }
        .then_with(|| natural_cmp(&a.name.to_string_lossy(), &b.name.to_string_lossy()));
        let by_key = if sort.ascending {
            by_key
        } else {
            by_key.reverse()
        };
        // Desempate por los bytes del nombre para que el orden sea determinista.
        b.is_dir()
            .cmp(&a.is_dir())
            .then(by_key)
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// Orden "natural": sin distinguir mayúsculas y con los números por su valor (`f2` < `f10`).
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (na, nb) = (take_number(&mut a), take_number(&mut b));
                let order = na.len().cmp(&nb.len()).then_with(|| na.cmp(&nb));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Consume un número y lo devuelve sin ceros a la izquierda.
fn take_number(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        digits.push(c);
    }
    digits.trim_start_matches('0').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::Kind;
    use std::time::{Duration, UNIX_EPOCH};

    fn entry(name: &str, dir: bool, size: u64, secs: u64) -> Entry {
        Entry {
            name: name.into(),
            kind: if dir { Kind::Dir } else { Kind::File },
            size,
            modified: Some(UNIX_EPOCH + Duration::from_secs(secs)),
            mode: 0,
        }
    }

    fn names(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| e.name.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn natural_order_ignores_case_and_reads_numbers() {
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less);
        assert_eq!(natural_cmp("File", "file"), Ordering::Equal);
        assert_eq!(natural_cmp("a007", "a7"), Ordering::Equal);
        assert_eq!(natural_cmp("abc", "abcd"), Ordering::Less);
    }

    #[test]
    fn dirs_first_in_both_directions() {
        let mut v = vec![
            entry("b", false, 1, 1),
            entry("z", true, 0, 1),
            entry("a", false, 1, 1),
        ];
        sort_entries(&mut v, Sort::default());
        assert_eq!(names(&v), ["z", "a", "b"]);
        sort_entries(
            &mut v,
            Sort {
                key: SortKey::Name,
                ascending: false,
            },
        );
        assert_eq!(names(&v), ["z", "b", "a"]);
    }

    #[test]
    fn sorts_by_size_and_date() {
        let mut v = vec![
            entry("a", false, 30, 5),
            entry("b", false, 10, 9),
            entry("c", false, 20, 1),
        ];
        sort_entries(
            &mut v,
            Sort {
                key: SortKey::Size,
                ascending: true,
            },
        );
        assert_eq!(names(&v), ["b", "c", "a"]);
        sort_entries(
            &mut v,
            Sort {
                key: SortKey::Modified,
                ascending: false,
            },
        );
        assert_eq!(names(&v), ["b", "a", "c"]);
    }

    #[test]
    fn toggling_flips_or_resets() {
        let s = Sort::default().toggled(SortKey::Name);
        assert!(!s.ascending);
        let s = s.toggled(SortKey::Size);
        assert_eq!(
            s,
            Sort {
                key: SortKey::Size,
                ascending: true
            }
        );
    }
}
