use super::*;

#[test]
fn non_clone_values_can_be_read_and_updated() {
    struct Value(usize);
    let (read, write) = create_signal(Value(1));
    let copied_handle = read;
    let copied_writer = write;
    assert_eq!(
        copied_writer.update(|value| {
            value.0 += 1;
            value.0
        }),
        2
    );
    assert_eq!(copied_handle.with(|value| value.0), 2);
    write.set_unconditionally(Value(3));
    assert_eq!(read.with_untracked(|value| value.0), 3);
}
