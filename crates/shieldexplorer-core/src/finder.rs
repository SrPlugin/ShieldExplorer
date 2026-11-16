//! Buscador difuso: indexa los archivos de un proyecto respetando `.gitignore` y los ordena por
//! parecido con lo escrito.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Más archivos que esto no se indexan: protege la memoria en carpetas gigantes.
pub const MAX_FILES: usize = 300_000;

/// Raíz del proyecto: el ancestro más cercano con `.git`, o la propia carpeta.
pub fn project_root(start: &Path) -> PathBuf {
    start
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(start)
        .to_path_buf()
}

/// Lista los archivos bajo `root` (rutas relativas), sin los ignorados por git ni `.git`.
/// Se detiene si `cancel` pasa a `true`.
pub fn index_files(root: &Path, cancel: &AtomicBool, limit: usize) -> Vec<String> {
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut files = Vec::new();
    for entry in walker.flatten() {
        if cancel.load(Ordering::Relaxed) || files.len() >= limit {
            break;
        }
        if entry.file_type().is_some_and(|t| t.is_file())
            && let Ok(rel) = entry.path().strip_prefix(root)
        {
            files.push(rel.to_string_lossy().into_owned());
        }
    }
    files
}

/// Puntuación de `candidate` para `query`, o `None` si no contiene las letras en orden.
///
/// Como fzf: sin mayúsculas en la consulta no distingue mayúsculas; premia las letras seguidas,
/// las que empiezan una palabra o están en el nombre del archivo, y los candidatos cortos.
/// ponytail: alineación voraz con ajuste hacia atrás (fzf v1), no óptima como la de fzf v2.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i64> {
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() {
        return Some(0);
    }
    let case_sensitive = q.iter().any(|c| c.is_uppercase());
    let norm = |c: char| {
        if case_sensitive {
            c
        } else {
            c.to_lowercase().next().unwrap_or(c)
        }
    };
    let c: Vec<char> = candidate.chars().collect();

    // Hacia delante: ¿están todas las letras en orden y dónde acaba la primera coincidencia?
    let mut qi = 0;
    let mut end = None;
    for (i, &ch) in c.iter().enumerate() {
        if norm(ch) == norm(q[qi]) {
            qi += 1;
            if qi == q.len() {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    // Hacia atrás: acorta la ventana a la coincidencia más compacta que termina ahí.
    let mut qi = q.len();
    let mut start = end;
    for i in (0..=end).rev() {
        if norm(c[i]) == norm(q[qi - 1]) {
            qi -= 1;
            if qi == 0 {
                start = i;
                break;
            }
        }
    }

    let mut score = 0i64;
    let mut qi = 0;
    let mut prev: Option<usize> = None;
    for i in start..=end {
        if qi < q.len() && norm(c[i]) == norm(q[qi]) {
            let mut s = 16;
            let boundary = i == 0
                || matches!(c[i - 1], '/' | '_' | '-' | '.' | ' ')
                || (c[i - 1].is_lowercase() && c[i].is_uppercase());
            if boundary {
                s += 12;
            }
            match prev {
                Some(p) if p + 1 == i => s += 24,
                Some(p) => s -= 2 + (i - p - 1).min(6) as i64,
                None => {}
            }
            if q[qi] == c[i] {
                s += 2;
            }
            score += s;
            prev = Some(i);
            qi += 1;
        }
    }
    let name_start = c.iter().rposition(|&x| x == '/').map_or(0, |p| p + 1);
    if start >= name_start {
        score += 30;
    }
    Some(score - (c.len() as i64) / 4)
}

/// Índices de los mejores `limit` candidatos para `query`, del más al menos parecido.
/// Con muchos candidatos reparte el trabajo entre los núcleos del procesador.
pub fn rank(candidates: &[String], query: &str, limit: usize) -> Vec<usize> {
    if query.is_empty() {
        return (0..candidates.len().min(limit)).collect();
    }
    let score_chunk = |offset: usize, chunk: &[String]| -> Vec<(i64, usize)> {
        chunk
            .iter()
            .enumerate()
            .filter_map(|(i, cand)| fuzzy_score(query, cand).map(|s| (s, offset + i)))
            .collect()
    };
    let threads = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8);
    let mut scored: Vec<(i64, usize)> = if candidates.len() < 20_000 || threads == 1 {
        score_chunk(0, candidates)
    } else {
        let size = candidates.len().div_ceil(threads);
        thread::scope(|scope| {
            let handles: Vec<_> = candidates
                .chunks(size)
                .enumerate()
                .map(|(n, chunk)| scope.spawn(move || score_chunk(n * size, chunk)))
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        })
    };
    let best_first = |a: &(i64, usize), b: &(i64, usize)| b.0.cmp(&a.0).then(a.1.cmp(&b.1));
    if scored.len() > limit {
        scored.select_nth_unstable_by(limit, best_first);
        scored.truncate(limit);
    }
    scored.sort_unstable_by(best_first);
    scored.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn requires_letters_in_order() {
        assert_eq!(fuzzy_score("xyz", "abc"), None);
        assert_eq!(fuzzy_score("ba", "ab"), None);
        assert!(fuzzy_score("abc", "a_b_c").is_some());
        assert_eq!(fuzzy_score("", "lo que sea"), Some(0));
    }

    #[test]
    fn prefers_consecutive_and_filename_matches() {
        assert!(fuzzy_score("abc", "abc.rs") > fuzzy_score("abc", "a_b_c.rs"));
        assert!(
            fuzzy_score("main", "src/main.rs") > fuzzy_score("main", "main_stuff/other/zzz.rs")
        );
        assert!(fuzzy_score("rs", "x.rs") > fuzzy_score("rs", "a_long_path/readme_something.txt"));
    }

    #[test]
    fn smartcase() {
        assert!(fuzzy_score("readme", "README.md").is_some());
        assert!(fuzzy_score("README", "readme.md").is_none());
    }

    #[test]
    fn ranks_best_first_and_limits() {
        let cands: Vec<String> = ["docs/readme.md", "src/main.rs", "a/m/a/i/n.txt", "main.rs"]
            .map(String::from)
            .to_vec();
        let ranked = rank(&cands, "main", 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(cands[ranked[0]], "main.rs");
        assert_eq!(rank(&cands, "", 3), [0, 1, 2]);
        assert!(rank(&cands, "zzz", 5).is_empty());
    }

    #[test]
    fn ranks_many_candidates_in_parallel_like_sequentially() {
        let cands: Vec<String> = (0..30_000)
            .map(|i| format!("dir{}/file{i}.txt", i % 50))
            .collect();
        let ranked = rank(&cands, "f77", 5);
        assert!(!ranked.is_empty());
        assert!(
            ranked
                .iter()
                .all(|&i| fuzzy_score("f77", &cands[i]).is_some())
        );
    }

    #[test]
    fn indexes_respecting_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join(".gitignore"), "target/\n").unwrap();
        fs::write(root.join("target/junk.o"), "").unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join(".env"), "").unwrap();
        fs::write(root.join(".git/HEAD"), "").unwrap();

        let mut files = index_files(root, &AtomicBool::new(false), 100);
        files.sort();
        assert_eq!(files, [".env", ".gitignore", "src/main.rs"]);
        assert_eq!(project_root(&root.join("src")), root);
        assert!(index_files(root, &AtomicBool::new(true), 100).is_empty());
    }
}
