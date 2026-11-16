//! Paleta clara y oscura. Se elige según la apariencia del sistema.

use gpui::{App, Global, Hsla, WindowAppearance, rgb};
use shieldexplorer_core::Class;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Hsla,
    pub bar: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub cursor: Hsla,
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

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
        }
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
}
