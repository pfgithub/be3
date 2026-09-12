use super::*;
use crate::base::text::TextAlign;
use crate::reactive::{view, RowBuilder};

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
                <column spacing=0.0>
                    <row spacing=0.0>
                        <sized node_ref=&box_ref width=BOX_WIDTH height=40.0>
                            <fill color=Color32::from_gray(40) radius=4>
                                <text
                                    node_ref=&label
                                    string=LABEL
                                    font_size=LABEL_SIZE
                                    align=TextAlign::End
                                    wrap=false
                                />
                            </fill>
                        </sized>
                    </row>
                </column>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), label.get()), LABEL);
    assert_eq!(harness.rect(box_ref.get()).width(), BOX_WIDTH);
    assert_eq!(harness.rect(box_ref.get()).height(), 40.0);
}
