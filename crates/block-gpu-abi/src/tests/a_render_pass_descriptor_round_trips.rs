use super::*;

#[test]
fn a_render_pass_descriptor_round_trips() {
    let descriptor = RenderPassDescriptor {
        label: "screens".into(),
        encoder: 11,
        color_attachments: vec![
            None,
            Some(ColorAttachment {
                view: 4,
                resolve_target: None,
                load: ColorLoadOp::Clear(Color {
                    red: 0.1,
                    green: 0.2,
                    blue: 0.3,
                    alpha: 1.0,
                }),
                store: StoreOp::Store,
                depth_slice: None,
            }),
        ],
        depth_stencil_attachment: None,
    };
    let bytes = encode(&descriptor);
    assert_eq!(decode::<RenderPassDescriptor>(&bytes).unwrap(), descriptor);
}
