use super::Pdf;

#[test]
fn pdf_rejects_non_pdf_data() {
    assert!(Pdf::new("not-a.pdf", b"not a pdf".to_vec()).is_err());
}
