use ui::{Axis, ButtonState, Color, Component, SizeRecommendation, SizeSource, Sizing, UiWindow};

fn update_demo_button(component: &mut Component, state: ButtonState) {
    let fill = if state.pressed {
        Color::rgb(0x7a, 0xa7, 0xd8)
    } else {
        Color::rgb(0xb8, 0xd5, 0xf2)
    };
    component.set_outline_width(if state.focused { 2.0 } else { 0.0 });
    component
        .child_mut()
        .expect("demo button outline has a child")
        .set_fill_color(fill);
}

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
                                    Component::button_with_action(
                                        Component::outline(
                                            Color::BLACK,
                                            2.0,
                                            0.0,
                                            Component::fill(
                                                Color::rgb(0xb8, 0xd5, 0xf2),
                                                Component::text("Demo"),
                                            ),
                                        ),
                                        update_demo_button,
                                        || {
                                            println!("Demo button activated");
                                        },
                                    ),
                                ),
                                (
                                    Sizing::fr(1.0),
                                    Component::sized(SizeSource::Parent, SizeSource::Zero, None),
                                ),
                            ],
                        ),
                    ),
                ),
                (
                    Sizing::fr(1.0),
                    Component::fill(
                        Color::rgb(0xff, 0xff, 0xff),
                        Component::sized(SizeSource::Parent, SizeSource::Parent, None),
                    ),
                ),
            ],
        ),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut window = UiWindow::new("ui demo", 800, 600)?;
    window.run(demo_ui(), SizeRecommendation::exact(800.0, 600.0))
}
