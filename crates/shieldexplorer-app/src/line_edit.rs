//! Campo de texto de una línea (ruta y búsqueda). Sin dependencia de GPUI para poder probarlo.
//!
//! ponytail: mueve el cursor por carácter (no por grafema), sin selección ni IME. Subir a
//! `unicode-segmentation` si hiciera falta.

#[derive(Debug)]
pub struct LineEdit {
    text: String,
    /// Posición del cursor en bytes, siempre en el límite de un carácter.
    cursor: usize,
}

impl LineEdit {
    pub fn new(text: String) -> Self {
        let cursor = text.len();
        Self { text, cursor }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Inserta en el cursor, ignorando caracteres de control (saltos de línea, tabuladores…).
    pub fn insert(&mut self, s: &str) {
        let clean: String = s.chars().filter(|c| !c.is_control()).collect();
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }

    pub fn backspace(&mut self) {
        if let Some((i, _)) = self.text[..self.cursor].char_indices().next_back() {
            self.text.remove(i);
            self.cursor = i;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some((i, _)) = self.text[..self.cursor].char_indices().next_back() {
            self.cursor = i;
        }
    }

    pub fn right(&mut self) {
        if let Some(c) = self.text[self.cursor..].chars().next() {
            self.cursor += c.len_utf8();
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_at_the_cursor() {
        let mut e = LineEdit::new("~/ab".into());
        e.left();
        e.insert("X");
        assert_eq!((e.text(), e.cursor()), ("~/aXb", 4));
        e.backspace();
        e.delete();
        assert_eq!(e.text(), "~/a");
        e.home();
        e.delete();
        assert_eq!(e.text(), "/a");
    }

    #[test]
    fn handles_multibyte_characters() {
        let mut e = LineEdit::new("ñá".into());
        e.left();
        assert_eq!(e.cursor(), 2);
        e.backspace();
        assert_eq!(e.text(), "á");
        e.end();
        e.right();
        e.insert("é\n\t");
        assert_eq!(e.text(), "áé");
    }

    #[test]
    fn edge_moves_do_not_panic() {
        let mut e = LineEdit::new(String::new());
        e.left();
        e.right();
        e.backspace();
        e.delete();
        assert_eq!(e.cursor(), 0);
    }
}
