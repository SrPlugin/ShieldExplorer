//! Estado de git de una carpeta, leído con el propio `git` (sin shell y sin tomar el bloqueo del
//! índice, así no estorba a otras herramientas).

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Marca de un archivo o carpeta. El orden es de menor a mayor gravedad: una carpeta muestra la
/// más grave de las que contiene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GitMark {
    Untracked,
    Renamed,
    Deleted,
    Added,
    Modified,
    Conflict,
}

impl GitMark {
    pub fn symbol(self) -> char {
        match self {
            Self::Untracked => '?',
            Self::Renamed => 'R',
            Self::Deleted => 'D',
            Self::Added => 'A',
            Self::Modified => 'M',
            Self::Conflict => 'U',
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Clone, Debug, Default)]
pub struct GitInfo {
    pub branch: Branch,
    /// Marca de cada entrada directa de la carpeta que tenga cambios.
    pub marks: HashMap<OsString, GitMark>,
}

impl GitInfo {
    pub fn dirty(&self) -> bool {
        !self.marks.is_empty()
    }
}

fn classify(x: u8, y: u8) -> Option<GitMark> {
    Some(match (x, y) {
        (b'?', b'?') => GitMark::Untracked,
        (b'!', b'!') | (b' ', b' ') => return None,
        (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => GitMark::Conflict,
        (b'M' | b'T', _) | (_, b'M' | b'T') => GitMark::Modified,
        (b'A', _) => GitMark::Added,
        (b'R' | b'C', _) | (_, b'R' | b'C') => GitMark::Renamed,
        (b'D', _) | (_, b'D') => GitMark::Deleted,
        _ => return None,
    })
}

fn parse_branch(line: &str) -> Branch {
    let rest = line.strip_prefix("## ").unwrap_or(line);
    if rest.starts_with("HEAD (no branch)") {
        return Branch {
            name: "(detached)".into(),
            ..Branch::default()
        };
    }
    let rest = rest.strip_prefix("No commits yet on ").unwrap_or(rest);
    let name = rest
        .split("...")
        .next()
        .unwrap_or(rest)
        .split(' ')
        .next()
        .unwrap_or(rest);
    let count = |word: &str| {
        rest.split(word)
            .nth(1)
            .and_then(|t| t.split([',', ']']).next())
            .and_then(|n| n.trim().parse().ok())
    };
    Branch {
        name: name.to_owned(),
        ahead: count("ahead ").unwrap_or(0),
        behind: count("behind ").unwrap_or(0),
    }
}

/// Interpreta la salida de `git status --porcelain=v1 -z -b`. `prefix` es la ruta de la carpeta
/// respecto a la raíz del repositorio (`src/`, o vacío en la raíz).
pub fn parse_status(output: &[u8], prefix: &str) -> GitInfo {
    let mut info = GitInfo::default();
    let mut records = output.split(|&b| b == 0).filter(|r| !r.is_empty());
    while let Some(rec) = records.next() {
        if let Some(line) = rec.strip_prefix(b"## ") {
            info.branch = parse_branch(&format!("## {}", String::from_utf8_lossy(line)));
            continue;
        }
        if rec.len() < 4 {
            continue;
        }
        let (x, y, path) = (rec[0], rec[1], &rec[3..]);
        if x == b'R' || x == b'C' {
            records.next(); // el siguiente registro es la ruta de origen
        }
        let Some(mark) = classify(x, y) else {
            continue;
        };
        let Some(inside) = path.strip_prefix(prefix.as_bytes()) else {
            continue;
        };
        let first = inside.split(|&b| b == b'/').next().unwrap_or(inside);
        if first.is_empty() {
            continue;
        }
        let name = OsString::from_vec(first.to_vec());
        info.marks
            .entry(name)
            .and_modify(|m| *m = (*m).max(mark))
            .or_insert(mark);
    }
    info
}

fn git(cwd: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// Estado de `cwd`, o `None` si no es un repositorio o no hay `git`. Bloquea: llamarlo en segundo plano.
pub fn read(cwd: &Path) -> Option<GitInfo> {
    let prefix = git(cwd, &["rev-parse", "--show-prefix"])?;
    let prefix = String::from_utf8_lossy(&prefix)
        .trim_end_matches('\n')
        .to_owned();
    let status = git(
        cwd,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "-b",
            "--untracked-files=normal",
        ],
    )?;
    Some(parse_status(&status, &prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"## main...origin/main [ahead 2, behind 1]\0 M src/a.rs\0A  src/new.rs\0?? docs/x\0R  b.rs\0old.rs\0?? top.txt\0UU conflict.rs\0";

    fn mark(info: &GitInfo, name: &str) -> Option<GitMark> {
        info.marks.get(std::ffi::OsStr::new(name)).copied()
    }

    #[test]
    fn parses_branch_and_counts() {
        let info = parse_status(SAMPLE, "");
        assert_eq!(
            info.branch,
            Branch {
                name: "main".into(),
                ahead: 2,
                behind: 1
            }
        );
        assert_eq!(parse_branch("## HEAD (no branch)").name, "(detached)");
        assert_eq!(parse_branch("## No commits yet on dev").name, "dev");
        assert_eq!(parse_branch("## feat/x").name, "feat/x");
    }

    #[test]
    fn marks_entries_at_the_repo_root() {
        let info = parse_status(SAMPLE, "");
        assert_eq!(mark(&info, "src"), Some(GitMark::Modified)); // la más grave entre M y A
        assert_eq!(mark(&info, "docs"), Some(GitMark::Untracked));
        assert_eq!(mark(&info, "b.rs"), Some(GitMark::Renamed));
        assert_eq!(mark(&info, "old.rs"), None); // el origen del renombrado no cuenta
        assert_eq!(mark(&info, "top.txt"), Some(GitMark::Untracked));
        assert_eq!(mark(&info, "conflict.rs"), Some(GitMark::Conflict));
        assert!(info.dirty());
    }

    #[test]
    fn marks_relative_to_a_subfolder() {
        let info = parse_status(SAMPLE, "src/");
        assert_eq!(mark(&info, "a.rs"), Some(GitMark::Modified));
        assert_eq!(mark(&info, "new.rs"), Some(GitMark::Added));
        assert_eq!(info.marks.len(), 2);
    }

    #[test]
    fn clean_repo_has_no_marks() {
        let info = parse_status(b"## main\0", "");
        assert!(!info.dirty());
        assert_eq!(info.branch.name, "main");
    }
}
