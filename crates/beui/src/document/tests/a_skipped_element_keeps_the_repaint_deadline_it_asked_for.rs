use super::*;
use crate::reactive::view;

const PANEL_HEIGHT: f32 = 200.0;

#[test]
fn a_skipped_element_keeps_the_repaint_deadline_it_asked_for() {
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
    let (_, paints) = counted(&mut document, text);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = paints.get();

    harness
        .document_mut()
        .set_frame_color(panel, Color32::from_gray(90));
    let output = harness.frame(Vec::new());

    assert_eq!(paints.get(), settled, "the caret is outside the damage");
    assert!(
        output.repaint_after < Duration::MAX,
        "a skipped element must still ask for the repaint it was waiting on"
    );
}
