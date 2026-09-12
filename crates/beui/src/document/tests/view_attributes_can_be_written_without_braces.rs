use super::*;
use crate::base::text::TextAlign;
use crate::reactive::{view, Row};

const LABEL: &str = "Unbraced";
const LABEL_SIZE: f32 = 20.0;
const BOX_WIDTH: f32 = 120.0;

#[test]
fn view_attributes_can_be_written_without_braces() {
    let box_ref = NodeRef::new();
    let label = NodeRef::new();
    let document = build({
        let box_ref = box_ref.clone();
        let label = label.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <Row spacing=0.0>
                        <Sized @node_ref=&box_ref width=BOX_WIDTH height=40.0>
                            <Fill color=Color32::from_gray(40) radius=4>
                                <Text
                                    @node_ref=&label
                                    string=LABEL
                                    font_size=LABEL_SIZE
                                    align=TextAlign::End
                                    wrap=false
                                />
                            </Fill>
                        </Sized>
                    </Row>
                </Column>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), label.get()), LABEL);
    assert_eq!(harness.rect(box_ref.get()).width(), BOX_WIDTH);
    assert_eq!(harness.rect(box_ref.get()).height(), 40.0);
}
