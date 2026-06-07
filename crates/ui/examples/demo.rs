use ui::{Axis, Color, Component, SizeRecommendation, Sizing, UiWindow};

fn demo_ui() -> Component {
    Component::scrollable(
        Axis::Vertical,
        Component::list(
            Axis::Vertical,
            [
                (
                    Sizing::Intrinsic,
                    Component::fill(
                        Color::rgb(0xd8, 0xd8, 0xd8),
                        Component::list(
                            Axis::Horizontal,
                            [
                                (
                                    Sizing::Intrinsic,
                                    Component::button(Component::text("Demo")),
                                ),
                                (Sizing::fr(1.0), Component::void()),
                            ],
                        ),
                    ),
                ),
                (
                    Sizing::fr(1.0),
                    Component::fill(Color::rgb(0xff, 0xff, 0xff), Component::void()),
                ),
            ],
        ),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut window = UiWindow::new("ui demo", 800, 600)?;
    window.run(demo_ui(), SizeRecommendation::exact(800.0, 600.0))
}
