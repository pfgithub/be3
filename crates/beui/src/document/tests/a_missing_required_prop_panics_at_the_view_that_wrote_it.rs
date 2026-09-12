use super::*;
use crate::reactive::view;

#[test]
fn a_missing_required_prop_panics_at_the_view_that_wrote_it() {
    let location = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = location.clone();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let reported = info
            .location()
            .map(|location| (location.file().to_string(), location.line()));
        *sink.lock().expect("the panic hook lock is healthy") = reported;
    }));

    let expected = line!() + 2;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build(|| view! { <ButtonFace /> })
    }));

    std::panic::set_hook(previous);
    assert!(result.is_err());

    let (file, line) = location
        .lock()
        .expect("the panic hook lock is healthy")
        .clone()
        .expect("the panic reported a location");
    assert!(
        file.ends_with("a_missing_required_prop_panics_at_the_view_that_wrote_it.rs"),
        "{file}"
    );
    assert_eq!(line, expected);
}
