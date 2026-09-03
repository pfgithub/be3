use super::*;

#[test]
fn commands_sent_after_shutdown_are_ignored() {
    let (sender, receiver) = mpsc::unbounded();
    let shutdown = Shutdown::new();
    let commands = CommandSender {
        commands: sender,
        shutdown: shutdown.clone(),
    };
    drop(receiver);

    assert!(commands.send(WorkerCommand::PauseSending).is_err());

    shutdown.request();

    assert!(commands.send(WorkerCommand::PauseSending).is_ok());
}
