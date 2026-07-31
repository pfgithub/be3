use super::super::core::render_stops;
use super::*;

#[test]
fn has_stop() {
    let cases: &[(CursorLeftRightStop, usize, &[u8])] = &[
        (CursorLeftRightStop::Byte, 0, b"|h|e|l|l|o|"),
        (CursorLeftRightStop::Byte, 0, b"|u|\xE2|\x80|\xA6|!|"),
        (CursorLeftRightStop::Codepoint, 0, b"|u|\xE2\x80\xA6|!|"),
        (
            CursorLeftRightStop::Codepoint,
            0,
            "|H|e|\u{301}|l|l|o|".as_bytes(),
        ),
        (
            CursorLeftRightStop::UnicodeGraphemeCluster,
            0,
            "|म|नी|ष|".as_bytes(),
        ),
        (
            CursorLeftRightStop::UnicodeGraphemeCluster,
            0,
            "|H|e\u{301}|l|l|o|".as_bytes(),
        ),
        (
            CursorLeftRightStop::UnicodeGraphemeCluster,
            0,
            "|🇷🇸|🇮🇴|🇷🇸|🇮🇴|🇷🇸|🇮🇴|🇷🇸|🇮🇴|".as_bytes(),
        ),
        (
            CursorLeftRightStop::UnicodeGraphemeCluster,
            0,
            "|\u{301}|".as_bytes(),
        ),
        (
            CursorLeftRightStop::UnicodeGraphemeCluster,
            0,
            "|h|i|👨‍👩‍👧‍👧|b|y|e|".as_bytes(),
        ),
        (
            CursorLeftRightStop::UnicodeGraphemeCluster,
            4,
            b"|    |    |",
        ),
        (CursorLeftRightStop::Word, 0, b"|hello> <world|"),
        (
            CursorLeftRightStop::Word,
            0,
            b"|    <\\\\>    <}>\n    <\\\\>    <@|vertex> <fn> <vert|(|in|:> <VertexIn|)|",
        ),
        (CursorLeftRightStop::Word, 0, b"| <myfn|(|crazy|)> |"),
        (CursorLeftRightStop::Word, 0, "|He|\u{301}|llo|".as_bytes()),
        (
            CursorLeftRightStop::Line,
            0,
            b"|line one]\n<line two]\n<line three|",
        ),
    ];
    for (stop, soft_tab_width, expected) in cases {
        assert_eq!(render_stops(expected, *stop, *soft_tab_width), *expected);
    }

    let indented = b"|m|a|i|n|\n|    |i|f| | | | | | |(| | | | | | |c|o|n|d| | | | | | |)|{|\n|    |    |e|p|i|c|!|;|\n|    |e|n|d|\n|e|n|d|";
    assert_eq!(
        render_stops(indented, CursorLeftRightStop::UnicodeGraphemeCluster, 4),
        indented
    );
}
