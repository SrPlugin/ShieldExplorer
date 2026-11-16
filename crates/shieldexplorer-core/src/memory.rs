//! Memoria entre sesiones: marcas (`ma` y `'a`) y carpetas visitadas con su frecuencia (como
//! zoxide). Se guarda como texto, una línea por dato, y un archivo dañado solo pierde esa línea.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Cuando la suma de rangos pasa de esto, se reducen todos para que los viejos se olviden.
const AGING_LIMIT: f64 = 9000.0;
/// Máximo de carpetas recordadas.
const MAX_FOLDERS: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Visits {
    rank: f64,
    last: u64,
}

#[derive(Debug, Default)]
pub struct Memory {
    marks: BTreeMap<char, PathBuf>,
    folders: HashMap<PathBuf, Visits>,
}

fn recency_factor(age_secs: u64) -> f64 {
    match age_secs {
        0..3600 => 4.0,
        3600..86_400 => 2.0,
        86_400..604_800 => 0.5,
        _ => 0.25,
    }
}

impl Memory {
    pub fn parse(text: &str) -> Self {
        let mut memory = Self::default();
        for line in text.lines() {
            let mut parts = line.splitn(2, ' ');
            match (parts.next(), parts.next()) {
                (Some("mark"), Some(rest)) => {
                    let mut p = rest.splitn(2, ' ');
                    if let (Some(key), Some(path)) = (p.next(), p.next())
                        && let Some(c) = key.chars().next().filter(|c| c.is_alphanumeric())
                    {
                        memory.marks.insert(c, PathBuf::from(path));
                    }
                }
                (Some("folder"), Some(rest)) => {
                    let mut p = rest.splitn(3, ' ');
                    if let (Some(rank), Some(last), Some(path)) = (p.next(), p.next(), p.next())
                        && let (Ok(rank), Ok(last)) = (rank.parse::<f64>(), last.parse::<u64>())
                        && rank.is_finite()
                        && rank > 0.0
                    {
                        memory
                            .folders
                            .insert(PathBuf::from(path), Visits { rank, last });
                    }
                }
                _ => {}
            }
        }
        memory
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for (c, path) in &self.marks {
            out.push_str(&format!("mark {c} {}\n", path.display()));
        }
        let mut folders: Vec<_> = self.folders.iter().collect();
        folders.sort_by(|a, b| b.1.rank.total_cmp(&a.1.rank).then(a.0.cmp(b.0)));
        for (path, v) in folders.into_iter().take(MAX_FOLDERS) {
            out.push_str(&format!(
                "folder {:.2} {} {}\n",
                v.rank,
                v.last,
                path.display()
            ));
        }
        out
    }

    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    /// Guarda escribiendo un archivo temporal y renombrándolo, para no dejar medio archivo si algo falla.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, self.serialize())?;
        fs::rename(tmp, path)
    }

    // --- Marcas ---

    pub fn set_mark(&mut self, key: char, path: PathBuf) {
        self.marks.insert(key, path);
    }

    pub fn mark(&self, key: char) -> Option<&Path> {
        self.marks.get(&key).map(PathBuf::as_path)
    }

    pub fn marks(&self) -> impl Iterator<Item = (char, &Path)> {
        self.marks.iter().map(|(c, p)| (*c, p.as_path()))
    }

    // --- Carpetas frecuentes ---

    /// Anota una visita a `path` (`now` en segundos desde la época).
    pub fn visit(&mut self, path: &Path, now: u64) {
        let v = self.folders.entry(path.to_path_buf()).or_insert(Visits {
            rank: 0.0,
            last: now,
        });
        v.rank += 1.0;
        v.last = now;
        let total: f64 = self.folders.values().map(|v| v.rank).sum();
        if total > AGING_LIMIT {
            let factor = 0.9 * AGING_LIMIT / total;
            self.folders.values_mut().for_each(|v| v.rank *= factor);
            self.folders.retain(|_, v| v.rank >= 1.0);
        }
    }

    /// Carpetas de más a menos "frecuentes" (visitas por lo reciente de la última).
    pub fn ranked(&self, now: u64) -> Vec<(PathBuf, f64)> {
        let mut list: Vec<_> = self
            .folders
            .iter()
            .map(|(p, v)| {
                (
                    p.clone(),
                    v.rank * recency_factor(now.saturating_sub(v.last)),
                )
            })
            .collect();
        list.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        list
    }

    /// La mejor carpeta para `query` (varias palabras, todas en orden en la ruta, y la última en
    /// el último componente), como `z` de zoxide. `exists` descarta las que ya no existen.
    pub fn best_match(
        &self,
        query: &str,
        now: u64,
        exists: impl Fn(&Path) -> bool,
    ) -> Option<PathBuf> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let last = terms.last()?;
        self.ranked(now).into_iter().map(|(p, _)| p).find(|path| {
            let text = path.to_string_lossy().to_lowercase();
            let mut from = 0;
            let in_order = terms.iter().all(|t| match text[from..].find(t.as_str()) {
                Some(i) => {
                    from += i + t.len();
                    true
                }
                None => false,
            });
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            in_order && name.contains(last.as_str()) && exists(path)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_text() {
        let mut m = Memory::default();
        m.set_mark('a', PathBuf::from("/home/u/con espacios"));
        m.visit(Path::new("/home/u/proyectos/app"), 1000);
        m.visit(Path::new("/home/u/proyectos/app"), 2000);
        let back = Memory::parse(&m.serialize());
        assert_eq!(back.mark('a'), Some(Path::new("/home/u/con espacios")));
        let ranked = back.ranked(2000);
        assert_eq!(ranked[0].0, Path::new("/home/u/proyectos/app"));
        assert!((ranked[0].1 - 8.0).abs() < 0.01); // 2 visitas × 4 (hace menos de una hora)
    }

    #[test]
    fn ignores_damaged_lines() {
        let m =
            Memory::parse("basura\nmark\nmark ab /x\nfolder x y /z\nfolder 1.0 5\nmark b /ok\n");
        assert_eq!(m.marks().count(), 2); // `a` con ruta /x y `b`
        assert!(m.ranked(0).is_empty());
    }

    #[test]
    fn recent_beats_old_with_equal_visits() {
        let mut m = Memory::default();
        m.visit(Path::new("/viejo"), 0);
        m.visit(Path::new("/nuevo"), 1_000_000);
        let r = m.ranked(1_000_100);
        assert_eq!(r[0].0, Path::new("/nuevo"));
    }

    #[test]
    fn aging_forgets_the_rarely_visited() {
        let mut m = Memory::default();
        m.visit(Path::new("/raro"), 0);
        for _ in 0..9100 {
            m.visit(Path::new("/frecuente"), 10);
        }
        let ranked = m.ranked(10);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].0, Path::new("/frecuente"));
    }

    #[test]
    fn jumps_like_zoxide() {
        let mut m = Memory::default();
        m.visit(Path::new("/home/u/proyectos/shield"), 100);
        m.visit(Path::new("/home/u/otros/shield"), 100);
        m.visit(Path::new("/home/u/otros/shield"), 101);
        let yes = |_: &Path| true;
        assert_eq!(
            m.best_match("shield", 200, yes),
            Some(PathBuf::from("/home/u/otros/shield"))
        );
        assert_eq!(
            m.best_match("proy shield", 200, yes),
            Some(PathBuf::from("/home/u/proyectos/shield"))
        );
        assert_eq!(m.best_match("shield proy", 200, yes), None); // el orden importa
        assert_eq!(m.best_match("", 200, yes), None);
        assert_eq!(
            m.best_match("shield", 200, |p: &Path| !p.starts_with("/home/u/otros")),
            Some(PathBuf::from("/home/u/proyectos/shield"))
        );
    }

    #[test]
    fn saves_and_loads_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sub/memory");
        let mut m = Memory::default();
        m.set_mark('z', PathBuf::from("/x"));
        m.save(&file).unwrap();
        assert_eq!(Memory::load(&file).mark('z'), Some(Path::new("/x")));
        assert!(
            Memory::load(&tmp.path().join("no-existe"))
                .marks()
                .next()
                .is_none()
        );
    }
}
