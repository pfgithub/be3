use std::sync::OnceLock;

use beui::{FontSource, FontSources};

const PROPORTIONAL: &[&str] = &["Ubuntu-Light"];
const MONOSPACE: &[&str] = &["Hack"];
const FALLBACK: &[&str] = &["NotoEmoji-Regular", "emoji-icon-font"];

pub fn bundled() -> FontSources {
    let bundled = definitions();
    FontSources {
        proportional: sources(bundled, PROPORTIONAL),
        monospace: sources(bundled, MONOSPACE),
        fallback: sources(bundled, FALLBACK),
    }
}

fn sources(bundled: &[(String, &'static [u8])], names: &[&str]) -> Vec<FontSource> {
    names
        .iter()
        .filter_map(|name| {
            let (_, bytes) = bundled.iter().find(|(bundled, _)| bundled == name)?;
            Some(FontSource::Memory(bytes))
        })
        .collect()
}

fn definitions() -> &'static [(String, &'static [u8])] {
    static BUNDLED: OnceLock<Vec<(String, &'static [u8])>> = OnceLock::new();
    BUNDLED.get_or_init(|| {
        egui::FontDefinitions::default()
            .font_data
            .into_iter()
            .map(|(name, data)| {
                let bytes: &'static [u8] = match &data.font {
                    std::borrow::Cow::Borrowed(bytes) => bytes,
                    std::borrow::Cow::Owned(bytes) => Box::leak(bytes.clone().into_boxed_slice()),
                };
                (name, bytes)
            })
            .collect()
    })
}
