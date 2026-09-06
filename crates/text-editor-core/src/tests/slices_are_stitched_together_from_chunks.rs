use std::borrow::Cow;

use super::*;

#[test]
fn slices_are_stitched_together_from_chunks() {
    let document = ChunkedDocument::new(b"chunked document", 3);

    assert!(matches!(document.slice(3..6), Cow::Borrowed(b"nke")));
    assert_eq!(document.slice(0..16).as_ref(), b"chunked document");
    assert_eq!(document.slice(4..12).as_ref(), b"ked docu");
    assert_eq!(document.slice(12..64).as_ref(), b"ment");
    assert_eq!(document.slice(64..96).as_ref(), b"");

    let anchor = document.anchor(8).expect("the eighth byte has an anchor");
    assert_eq!(document.anchor_index(anchor), Some(8));
    assert_eq!(document.slice_from_anchor(anchor, 8).as_ref(), b"document");
}

struct ChunkedDocument {
    bytes: Vec<u8>,
    anchors: Vec<Anchor>,
    chunk: usize,
}

impl ChunkedDocument {
    fn new(bytes: impl AsRef<[u8]>, chunk: usize) -> Self {
        let bytes = bytes.as_ref().to_vec();
        let anchors = bytes.iter().map(|_| Anchor::new()).collect();
        Self {
            bytes,
            anchors,
            chunk,
        }
    }
}

impl DocumentRead for ChunkedDocument {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        let end = index.saturating_add(self.chunk).min(self.bytes.len());
        self.bytes.get(index..end).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.anchors.get(index).copied()
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.anchors
            .iter()
            .position(|candidate| *candidate == anchor)
    }

    fn language(&self) -> TextLanguage {
        TextLanguage::default()
    }

    fn indentation(&self) -> TextIndentation {
        TextIndentation::default()
    }
}
