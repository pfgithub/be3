use super::*;

#[test]
fn performance_messages_round_trip() {
    let message = Message::Editor(EditorMessage::Performance {
        instance: EditorInstanceId(7),
        group: "PDF (document)".into(),
        measurements: vec![
            PerformanceMeasurement::Duration {
                name: "PDFium render".into(),
                nanoseconds: 12_345_678,
            },
            PerformanceMeasurement::Count {
                name: "Pixels".into(),
                count: 4_194_304,
            },
        ],
    });

    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
