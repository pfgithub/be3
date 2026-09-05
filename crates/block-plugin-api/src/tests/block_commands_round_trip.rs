use super::*;

use crate::{BlockCommand, BlockLocation};

#[test]
fn block_commands_round_trip() {
    for command in [
        BlockCommand::Share,
        BlockCommand::Rename,
        BlockCommand::Unlink { container: [5; 16] },
        BlockCommand::Delete {
            block_type: [2; 16],
            source: BlockLocation::Root,
            is_reference: false,
        },
        BlockCommand::Move {
            block_type: [2; 16],
            source: BlockLocation::Block([4; 16]),
            destination: [3; 16],
            is_reference: true,
        },
        BlockCommand::Place {
            block_type: [2; 16],
            parent: [3; 16],
            linked: true,
        },
        BlockCommand::Delete {
            block_type: [2; 16],
            source: BlockLocation::Orphaned,
            is_reference: true,
        },
    ] {
        let message = Message::Editor(EditorMessage::BlockCommand {
            instance: EditorInstanceId(6),
            block_id: [1; 16],
            command,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
    let message = Message::Editor(EditorMessage::DragBlock {
        instance: EditorInstanceId(6),
        block_id: [1; 16],
        block_type: [2; 16],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
