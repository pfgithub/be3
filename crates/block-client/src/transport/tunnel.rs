use futures_channel::mpsc;
use futures_util::StreamExt;

use super::SocketMessage;

/// A connection that reaches the server through another client rather than
/// over the network. The worker's protocol logic is unchanged: it still sends
/// and receives the same JSON frames, they are just carried by whoever holds
/// the other end of the channel.
pub(crate) struct TunnelSocket {
    outgoing: mpsc::UnboundedSender<String>,
    incoming: mpsc::UnboundedReceiver<String>,
}

impl TunnelSocket {
    pub(crate) fn new(
        outgoing: mpsc::UnboundedSender<String>,
        incoming: mpsc::UnboundedReceiver<String>,
    ) -> Self {
        Self { outgoing, incoming }
    }

    pub(crate) async fn send_text(&mut self, text: String) -> Result<(), String> {
        self.outgoing
            .unbounded_send(text)
            .map_err(|_| "the block client tunnel was closed".to_owned())
    }

    pub(crate) async fn next(&mut self) -> Option<Result<SocketMessage, String>> {
        match self.incoming.next().await {
            Some(text) => Some(Ok(SocketMessage::Text(text))),
            None => Some(Ok(SocketMessage::Close)),
        }
    }
}
