use crate::sys;
use serde_json::Value;
use std::ffi::CStr;

#[test]
fn struct_layouts_match_the_library() {
    let json = unsafe { CStr::from_ptr(sys::ghostty_type_json()) };
    let types: Value = serde_json::from_str(json.to_str().unwrap()).unwrap();

    assert_size::<sys::ColorRgb>(&types, "GhosttyColorRgb");
    assert_size::<sys::Buffer>(&types, "GhosttyBuffer");
    assert_size::<sys::TerminalOptions>(&types, "GhosttyTerminalOptions");
    assert_size::<sys::ScrollViewport>(&types, "GhosttyTerminalScrollViewport");
    assert_size::<sys::StyleColor>(&types, "GhosttyStyleColor");
    assert_size::<sys::Style>(&types, "GhosttyStyle");
    assert_size::<sys::RenderStateColors>(&types, "GhosttyRenderStateColors");

    let style = sys::Style::default();
    let base = std::ptr::from_ref(&style).addr();
    assert_offset(
        &types,
        "GhosttyStyle",
        "size",
        std::ptr::from_ref(&style.size).addr() - base,
    );
    assert_offset(
        &types,
        "GhosttyStyle",
        "bg_color",
        std::ptr::from_ref(&style.bg_color).addr() - base,
    );
    assert_offset(
        &types,
        "GhosttyStyle",
        "inverse",
        std::ptr::from_ref(&style.inverse).addr() - base,
    );
    assert_offset(
        &types,
        "GhosttyStyle",
        "underline",
        std::ptr::from_ref(&style.underline).addr() - base,
    );

    let colors = sys::RenderStateColors::default();
    let base = std::ptr::from_ref(&colors).addr();
    assert_offset(
        &types,
        "GhosttyRenderStateColors",
        "cursor_has_value",
        std::ptr::from_ref(&colors.cursor_has_value).addr() - base,
    );
    assert_offset(
        &types,
        "GhosttyRenderStateColors",
        "palette",
        std::ptr::from_ref(&colors.palette).addr() - base,
    );
}

fn assert_size<T>(types: &Value, name: &str) {
    let described = types[name]["size"]
        .as_u64()
        .expect("the library describes the struct");
    assert_eq!(described as usize, size_of::<T>(), "size of {name}");
}

fn assert_offset(types: &Value, name: &str, field: &str, offset: usize) {
    let described = types[name]["fields"][field]["offset"]
        .as_u64()
        .unwrap_or_else(|| panic!("{name} has no field {field}"));
    assert_eq!(described as usize, offset, "offset of {name}.{field}");
}
