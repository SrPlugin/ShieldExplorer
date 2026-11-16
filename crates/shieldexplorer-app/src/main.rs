use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, div, prelude::*, px, size};

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child("explorer")
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(700.)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Empty),
        );
        if opened.is_err() {
            cx.quit();
        }
    });
}
