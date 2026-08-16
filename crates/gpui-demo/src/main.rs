use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, IntoElement, ParentElement,
    Render, Styled, Window, WindowBounds, WindowOptions,
};

struct LoginScreen;

impl Render for LoginScreen {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x101828))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(400.))
                    .p(px(32.))
                    .bg(rgb(0xffffff))
                    .rounded(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(20.))
                    .child(
                        div()
                            .text_xl()
                            .text_color(rgb(0x101828))
                            .child("Welcome back"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x667085))
                            .child("Sign in to continue to GPUI Demo"),
                    )
                    .child(field("Email address", "you@example.com"))
                    .child(field("Password", "Enter your password"))
                    .child(
                        div()
                            .h(px(44.))
                            .bg(rgb(0x155eef))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0xffffff))
                            .child("Sign in"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x667085))
                            .child("Demo only: signing in does not send a request."),
                    ),
            )
    }
}

fn field(label: &'static str, placeholder: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_sm().text_color(rgb(0x344054)).child(label))
        .child(
            div()
                .h(px(44.))
                .px(px(12.))
                .border_1()
                .border_color(rgb(0xd0d5dd))
                .rounded(px(6.))
                .flex()
                .items_center()
                .text_color(rgb(0x98a2b3))
                .child(placeholder),
        )
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(650.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| LoginScreen),
        )
        .unwrap();
    });
}
