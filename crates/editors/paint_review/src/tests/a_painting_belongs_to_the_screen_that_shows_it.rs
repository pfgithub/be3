use super::*;

use block_editor_plugin::Waker;

use crate::render::Paintings;

#[test]
fn a_painting_belongs_to_the_screen_that_shows_it() {
    let data = painting(30).encode().unwrap();
    let toolbar = egui::Context::default();
    let main = egui::Context::default();
    let mut paintings = Paintings::default();

    paintings.want("counter", data);
    paintings.settle(&toolbar, &Waker::default());
    paintings.settle(&toolbar, &Waker::default());
    assert_eq!(held(&toolbar), Vec::<[usize; 2]>::new());
    assert_eq!(held(&main), Vec::<[usize; 2]>::new());

    assert_eq!(sized(&main, &mut paintings), Some([24, 16]));
    assert_eq!(held(&main), vec![[24, 16]]);
    assert_eq!(held(&toolbar), Vec::<[usize; 2]>::new());

    assert_eq!(sized(&toolbar, &mut paintings), Some([24, 16]));
    assert_eq!(held(&toolbar), vec![[24, 16]]);
}

fn sized(context: &egui::Context, paintings: &mut Paintings) -> Option<[usize; 2]> {
    let rendered = paintings
        .rendered(context, "counter", 0)
        .expect("the painting was rastered")
        .expect("the painting was rastered");
    let manager = context.tex_manager();
    let size = manager.read().meta(rendered.texture.id()).map(|it| it.size);
    size
}

fn held(context: &egui::Context) -> Vec<[usize; 2]> {
    context
        .tex_manager()
        .read()
        .allocated()
        .filter(|(_, meta)| meta.name == "paint-review")
        .map(|(_, meta)| meta.size)
        .collect()
}
