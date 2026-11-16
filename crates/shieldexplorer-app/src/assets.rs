//! Fuentes incluidas en el binario.

use std::borrow::Cow;

use gpui::App;

pub const MONO_FONT: &str = "JetBrains Mono";

const FONTS: [&[u8]; 3] = [
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Medium.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf"),
];

/// Registra las fuentes sin copiarlas: el texto las lee directamente del binario.
pub fn load_fonts(cx: &App) -> anyhow::Result<()> {
    cx.text_system()
        .add_fonts(FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect())
}
