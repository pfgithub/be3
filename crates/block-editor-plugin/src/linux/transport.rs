use std::{io, os::fd::RawFd, os::unix::net::UnixStream};

use block_plugin_api::{desktop_attachments::UnixAttachmentCarrier, Message};

pub(crate) type Attachment = RawFd;

pub(crate) fn connect(endpoint: &str) -> io::Result<Connection> {
    Ok(Connection {
        stream: UnixStream::connect(endpoint)?,
    })
}

pub(crate) struct Connection {
    stream: UnixStream,
}

impl Connection {
    pub(crate) fn reader(&mut self) -> &mut impl io::Read {
        &mut self.stream
    }

    pub(crate) fn writer(&mut self) -> &mut impl io::Write {
        &mut self.stream
    }

    pub(crate) fn split(self) -> io::Result<(Reader, Sender)> {
        let reader = self.stream.try_clone()?;
        Ok((
            Reader(UnixAttachmentCarrier::new(reader)),
            Sender(UnixAttachmentCarrier::new(self.stream)),
        ))
    }
}

pub(crate) struct Reader(UnixAttachmentCarrier);

impl Reader {
    pub(crate) fn receive(&mut self) -> Result<Message, String> {
        self.0
            .receive()
            .map(|(message, _)| message)
            .map_err(|error| error.to_string())
    }
}

pub(crate) struct Sender(UnixAttachmentCarrier);

impl Sender {
    pub(crate) fn send(
        &mut self,
        message: &Message,
        attachments: &[Attachment],
    ) -> Result<(), String> {
        self.0
            .send(message, attachments)
            .map_err(|error| error.to_string())
    }
}
