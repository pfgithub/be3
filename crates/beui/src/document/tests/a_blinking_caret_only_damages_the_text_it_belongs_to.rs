use super::*;
use crate::reactive::view;

const PANEL_HEIGHT: f32 = 200.0;

#[test]
fn a_blinking_caret_only_damages_the_text_it_belongs_to() {
    let (panel, text) = (NodeRef::new(), NodeRef::new());
    let mut document = build({
        let (panel, text) = (panel.clone(), text.clone());
        move || {
            view! {
                <Column spacing=0.0>
                    <Frame @node_ref=&panel height={PANEL_HEIGHT} color=Color32::WHITE radius=0 />
                    <Text
                        @node_ref=&text
                        string="hello"
                        font_size=14.0
                        color=Color32::WHITE
                        caret=Some(0)
                    />
                </Column>
            }
        }
    });
    let (panel, text) = (panel.get(), text.get());
    let (_, paints) = counted(&mut document, panel);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = paints.get();

    harness.document.next_paint = Some(Instant::now());
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("the caret blinks on its deadline");

    assert_eq!(
        paints.get(),
        settled,
        "a deadline repaint must not reach past the element that asked for it"
    );
    assert!(damage.intersects(harness.rect(text)));
    assert!(!damage.intersects(harness.rect(panel)));
}
