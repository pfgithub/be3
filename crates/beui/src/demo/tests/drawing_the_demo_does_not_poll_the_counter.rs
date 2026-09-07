use super::*;

#[test]
fn drawing_the_demo_does_not_poll_the_counter() {
    let counter = Rc::new(CountingCounter::default());
    let mut demo = Demo::new(counter.clone());
    assert_eq!(counter.reads.get(), 1);

    let context = Context::new();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    for _ in 0..3 {
        context.run(RawInput::default(), |context| demo.show(context, rect));
    }
    assert_eq!(counter.reads.get(), 1);

    demo.set_value(42);
    context.run(RawInput::default(), |context| demo.show(context, rect));
    assert_eq!(
        demo.document().node_detail(demo.value_node()).unwrap(),
        "\"42\""
    );
    assert_eq!(counter.reads.get(), 1);
}
