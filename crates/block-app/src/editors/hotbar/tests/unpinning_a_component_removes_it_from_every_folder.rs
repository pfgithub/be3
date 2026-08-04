use super::*;

fn component(name: &str, compiled: Uuid) -> HotbarSlot {
    HotbarSlot::Component {
        name: name.to_owned(),
        compiled,
    }
}

#[test]
fn unpinning_a_component_removes_it_from_every_folder() {
    let adder = Uuid::new_v4();
    let latch = Uuid::new_v4();
    let slots = vec![
        component("Adder", adder),
        HotbarSlot::Folder {
            name: "Memory".to_owned(),
            slots: vec![component("Latch", latch), component("Adder", adder)],
        },
    ];

    let remaining = without_component(&slots, adder);

    assert_eq!(
        remaining,
        vec![HotbarSlot::Folder {
            name: "Memory".to_owned(),
            slots: vec![component("Latch", latch)],
        }]
    );
    assert_eq!(count_slots(&remaining), 2);
}
