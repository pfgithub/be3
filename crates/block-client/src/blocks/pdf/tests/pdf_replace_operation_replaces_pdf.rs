use block::Block;

use super::{sample_bytes, Pdf, PdfOperation};

#[test]
fn pdf_replace_operation_replaces_pdf() {
    let mut pdf = Pdf::new("before.pdf", sample_bytes()).unwrap();
    let replacement = Pdf::new("after.pdf", sample_bytes()).unwrap();

    Pdf::apply_operation(
        &mut pdf,
        &PdfOperation::Replace {
            pdf: replacement.clone(),
        },
    );

    assert_eq!(pdf, replacement);
}
