use beui_macros::component;

use crate::geometry::Vec2;
use crate::node::NodeId;
use crate::reactive::{
    component_size, create_memo, provide_context, use_context, Memo, ReadSignal, Render,
};

#[derive(Clone)]
pub struct ContainerSize(pub ReadSignal<Vec2>);

#[component]
pub fn Container(#[prop(children)] content: Render<ReadSignal<Vec2>>) -> NodeId {
    let size = component_size();
    provide_context(ContainerSize(size.clone()));
    content.call(size)
}

pub fn container_size() -> Option<ReadSignal<Vec2>> {
    use_context::<ContainerSize>().map(|ContainerSize(size)| size)
}

pub fn narrower_than(width: f32) -> Memo<bool> {
    let size = container_size();
    create_memo(move || {
        size.as_ref().is_some_and(|size| {
            let measured = size.get().x;
            measured > 0.0 && measured < width
        })
    })
}
