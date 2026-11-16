use std::path::PathBuf;

/// Pilas de "atrás" y "adelante" de la navegación.
#[derive(Debug, Default)]
pub struct History {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
}

impl History {
    /// Se navega a un sitio nuevo desde `from`: lo que había en "adelante" deja de valer.
    pub fn visit(&mut self, from: PathBuf) {
        self.back.push(from);
        self.forward.clear();
    }

    /// Retrocede desde `current` y devuelve el destino.
    pub fn go_back(&mut self, current: PathBuf) -> Option<PathBuf> {
        let target = self.back.pop()?;
        self.forward.push(current);
        Some(target)
    }

    /// Avanza desde `current` y devuelve el destino.
    pub fn go_forward(&mut self, current: PathBuf) -> Option<PathBuf> {
        let target = self.forward.pop()?;
        self.back.push(current);
        Some(target)
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
        h.visit(p("/b"));
        assert_eq!(h.go_back(p("/c")), Some(p("/b")));
        assert_eq!(h.go_back(p("/b")), Some(p("/a")));
        assert_eq!(h.go_back(p("/a")), None);
        assert_eq!(h.go_forward(p("/a")), Some(p("/b")));
        assert!(h.can_go_forward());
    }

    #[test]
    fn visiting_clears_forward() {
        let mut h = History::default();
        h.visit(p("/a"));
        h.go_back(p("/b"));
        assert!(h.can_go_forward());
        h.visit(p("/a"));
        assert!(!h.can_go_forward());
    }
}
