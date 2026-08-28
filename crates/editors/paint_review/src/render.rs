use block_editor_plugin::egui;
use paint_snapshot::Snapshot;

pub struct Rendered {
    pub texture: egui::TextureHandle,
    pub size: egui::Vec2,
    pub description: String,
}

pub fn describe(snapshot: &Snapshot) -> String {
    let [width, height] = snapshot.size;
    format!(
        "{width}x{height}, {} draw calls, {} textures",
        snapshot.primitives.len(),
        snapshot.textures.len()
    )
}

pub fn change(approved: &[u8], current: &[u8]) -> Result<String, String> {
    let (approved, current) = (Snapshot::decode(approved)?, Snapshot::decode(current)?);
    Ok(paint_snapshot::difference(&approved, &current).map_or_else(
        || "the painting is the same".to_owned(),
        |difference| difference.description,
    ))
}

pub fn render(context: &egui::Context, name: &str, data: &[u8]) -> Result<Rendered, String> {
    let snapshot = Snapshot::decode(data)?;
    let image = paint_snapshot::render(&snapshot)?;
    let size = egui::vec2(image.width() as f32, image.height() as f32);
    let pixels = egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    );
    Ok(Rendered {
        texture: context.load_texture(name, pixels, egui::TextureOptions::LINEAR),
        size,
        description: describe(&snapshot),
    })
}
