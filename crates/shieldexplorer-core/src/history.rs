use std::path::{Path, PathBuf};

/// Pilas de "atrás" y "adelante" de la navegación.
///
/// Se consulta el destino con `peek_*` y solo se confirma con `went_*`/`visit` cuando la carga
/// tuvo éxito, así una carpeta ilegible no hace perder el historial.
#[derive(Debug, Default)]
pub struct History {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
}

impl History {
    pub fn peek_back(&self) -> Option<&Path> {
        self.back.last().map(PathBuf::as_path)
    }

    pub fn peek_forward(&self) -> Option<&Path> {
        self.forward.last().map(PathBuf::as_path)
    }

    /// Se llegó a un sitio nuevo desde `from`: lo que había en "adelante" deja de valer.
    pub fn visit(&mut self, from: PathBuf) {
        self.back.push(from);
        self.forward.clear();
    }

    /// Se llegó retrocediendo desde `from`.
    pub fn went_back(&mut self, from: PathBuf) {
        if self.back.pop().is_some() {
            self.forward.push(from);
        }
    }

    /// Se llegó avanzando desde `from`.
    pub fn went_forward(&mut self, from: PathBuf) {
        if self.forward.pop().is_some() {
            self.back.push(from);
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn back_and_forward_round_trip() {
        let mut h = History::default();
        h.visit(p("/a"));
        h.visit(p("/b")); // ahora en /c
        assert_eq!(h.peek_back(), Some(Path::new("/b")));
        h.went_back(p("/c")); // ahora en /b
        h.went_back(p("/b")); // ahora en /a
        assert_eq!(h.peek_back(), None);
        assert_eq!(h.peek_forward(), Some(Path::new("/b")));
        h.went_forward(p("/a")); // ahora en /b
        assert_eq!(h.peek_back(), Some(Path::new("/a")));
        assert_eq!(h.peek_forward(), Some(Path::new("/c")));
    }

    #[test]
    fn peeking_does_not_consume() {
        let mut h = History::default();
        h.visit(p("/a"));
        assert!(h.peek_back().is_some());
        assert!(h.can_go_back());
        assert!(h.peek_back().is_some());
    }

    #[test]
    fn visiting_clears_forward() {
        let mut h = History::default();
        h.visit(p("/a"));
        h.went_back(p("/b"));
        assert!(h.can_go_forward());
        h.visit(p("/a"));
        assert!(!h.can_go_forward());
    }
}
