use block_client::blocks::image::Image;
use block_editor_plugin::{FileFilter, PickedFile};

pub(crate) fn image_filter() -> FileFilter {
    FileFilter {
        name: "Images".to_owned(),
        default_file_name: "Image".to_owned(),
        extensions: Image::FILE_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        mime_types: Image::MIME_TYPES
            .iter()
            .map(|mime| (*mime).to_owned())
            .collect(),
    }
}

pub(crate) fn imported_image(file: PickedFile) -> Image {
    let PickedFile { name, data } = file;
    Image::new(name, data)
}
