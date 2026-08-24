use super::*;

#[test]
fn installed_fonts_cover_mixed_language_text_when_available() {
    let Some(engine) = TextEngine::new() else {
        return;
    };
    let characters = "Hello 世界 مرحبا"
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<Vec<_>>();
    if !characters
        .iter()
        .all(|character| engine.font_index_for(*character).is_some())
    {
        return;
    }

    for character in characters {
        let font = &engine.fonts[engine.font_index_for(character).unwrap()];
        assert_ne!(
            unsafe { ft::FT_Get_Char_Index(font.face, character as ft::FT_ULong) },
            0,
            "no fallback font covers {character:?}"
        );
    }
}
