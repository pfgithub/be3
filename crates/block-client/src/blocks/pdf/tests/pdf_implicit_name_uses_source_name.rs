use block::Block;

use super::{sample_bytes, Pdf};

#[test]
fn pdf_implicit_name_uses_source_name() {
    let named = Pdf::new("report.pdf", sample_bytes()).unwrap();
    let unnamed = Pdf::new("  ", sample_bytes()).unwrap();

    assert_eq!(named.implicit_name(), Some("report.pdf".to_owned()));
    assert_eq!(unnamed.implicit_name(), None);
}
