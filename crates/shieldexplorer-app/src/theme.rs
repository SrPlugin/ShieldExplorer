//! Paleta clara y oscura. Se elige según la apariencia del sistema, salvo que la configuración
//! fije una, y cada color se puede cambiar desde el archivo de configuración.

use gpui::{App, Global, Hsla, WindowAppearance, rgb};
use shieldexplorer_core::{Class, GitMark};

use crate::config::{Config, ThemeChoice};

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Hsla,
    pub bar: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub cursor: Hsla,
    pub marked: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    dir: Hsla,
    image: Hsla,
    code: Hsla,
    doc: Hsla,
    media: Hsla,
    archive: Hsla,
}

impl Global for Theme {}

/// Lo que la configuración dice sobre el tema: cuál usar y qué colores cambiar.
#[derive(Clone, Default)]
pub struct ThemeSettings {
    pub choice: ThemeChoice,
    /// `(esquema oscuro?, nombre, 0xRRGGBB)`
    pub overrides: Vec<(bool, String, u32)>,
}

impl Global for ThemeSettings {}

impl ThemeSettings {
    pub fn from_config(config: &Config) -> Self {
        Self {
            choice: config.theme,
            overrides: config.colors.clone(),
        }
    }
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

impl Theme {
    pub fn light() -> Self {
        Self {
            bg: c(0xfbfbf9),
            bar: c(0xf1f1ee),
            border: c(0xd9d9d3),
            text: c(0x24292f),
            muted: c(0x6a717b),
            faint: c(0x9aa0a8),
            cursor: c(0xdfeaf9),
            marked: c(0xfbefc6),
            accent: c(0x0969da),
            on_accent: c(0xffffff),
            danger: c(0xcf222e),
            dir: c(0x0550ae),
            image: c(0x8250df),
            code: c(0x116329),
            doc: c(0xbc4c00),
            media: c(0x9a6700),
            archive: c(0xcf222e),
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: c(0x0d1117),
            bar: c(0x161b22),
            border: c(0x252b34),
            text: c(0xc9d1d9),
            muted: c(0x8b949e),
            faint: c(0x566070),
            cursor: c(0x172338),
            marked: c(0x3a3320),
            accent: c(0x58a6ff),
            on_accent: c(0x0d1117),
            danger: c(0xff7b72),
            dir: c(0x79b8ff),
            image: c(0xd2a8ff),
            code: c(0x7ee2a8),
            doc: c(0xffa657),
            media: c(0xf2cc60),
            archive: c(0xff7b72),
        }
    }

    /// El tema que toca para la apariencia del sistema, con la elección y los colores de la
    /// configuración aplicados.
    pub fn resolve(appearance: WindowAppearance, settings: &ThemeSettings) -> Self {
        let dark = match settings.choice {
            ThemeChoice::Light => false,
            ThemeChoice::Dark => true,
            ThemeChoice::System => {
                matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                )
            }
        };
        let mut theme = if dark { Self::dark() } else { Self::light() };
        for (is_dark, name, hex) in &settings.overrides {
            if *is_dark == dark {
                theme.set(name, c(*hex));
            }
        }
        theme
    }

    /// Cambia un color por su nombre. Devuelve `false` si el nombre no existe.
    fn set(&mut self, name: &str, color: Hsla) -> bool {
        let slot = match name {
            "bg" => &mut self.bg,
            "bar" => &mut self.bar,
            "border" => &mut self.border,
            "text" => &mut self.text,
            "muted" => &mut self.muted,
            "faint" => &mut self.faint,
            "cursor" => &mut self.cursor,
            "marked" => &mut self.marked,
            "accent" => &mut self.accent,
            "on_accent" => &mut self.on_accent,
            "danger" => &mut self.danger,
            "dir" => &mut self.dir,
            "image" => &mut self.image,
            "code" => &mut self.code,
            "doc" => &mut self.doc,
            "media" => &mut self.media,
            "archive" => &mut self.archive,
            _ => return false,
        };
        *slot = color;
        true
    }

    pub fn get(cx: &App) -> Self {
        *cx.global::<Self>()
    }

    pub fn class_color(&self, class: Class) -> Hsla {
        match class {
            Class::Dir => self.dir,
            Class::Image => self.image,
            Class::Code => self.code,
            Class::Doc => self.doc,
            Class::Media => self.media,
            Class::Archive => self.archive,
            Class::Broken => self.danger,
            Class::Other => self.text,
        }
    }

    pub fn git_color(&self, mark: GitMark) -> Hsla {
        match mark {
            GitMark::Modified => self.doc,
            GitMark::Added => self.code,
            GitMark::Renamed => self.image,
            GitMark::Deleted | GitMark::Conflict => self.danger,
            GitMark::Untracked => self.muted,
        }
    }
}
