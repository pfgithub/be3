//! The minimal HTTP/1.1 subset the block server needs: enough to read a request
//! head, tell a websocket upgrade apart from a management request, and answer a
//! management request with JSON.

#[cfg(test)]
mod tests;

use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use crate::ServerError;

/// Largest request line and header block the server will buffer.
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// Largest management request body the server will accept.
const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_HEADERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Ok,
    BadRequest,
    Forbidden,
    NotFound,
    MethodNotAllowed,
}

impl Status {
    fn line(self) -> &'static str {
        match self {
            Self::Ok => "200 OK",
            Self::BadRequest => "400 Bad Request",
            Self::Forbidden => "403 Forbidden",
            Self::NotFound => "404 Not Found",
            Self::MethodNotAllowed => "405 Method Not Allowed",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RequestHead {
    pub method: String,
    /// The request target, including any query string.
    pub path: String,
    /// Header names are lowercased; values keep their original case.
    pub headers: Vec<(String, String)>,
}

impl RequestHead {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }

    /// Whether this is the opening request of a websocket handshake, which the
    /// server hands to the websocket implementation instead of answering itself.
    pub fn is_websocket_upgrade(&self) -> bool {
        self.method == "GET"
            && self
                .header("upgrade")
                .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
    }

    fn content_length(&self) -> Result<usize, ServerError> {
        let Some(value) = self.header("content-length") else {
            return Ok(0);
        };
        value
            .trim()
            .parse()
            .map_err(|_| ServerError::InvalidRequest(format!("invalid content-length {value}")))
    }
}

/// A request head plus every byte read while looking for its end.
#[derive(Debug)]
pub(crate) struct BufferedRequest {
    pub head: RequestHead,
    /// Everything read from the stream, the head included, so that a websocket
    /// handshake can be replayed to the websocket implementation.
    pub buffered: Vec<u8>,
    head_len: usize,
}

impl BufferedRequest {
    /// The part of the body that arrived alongside the head.
    fn body_prefix(&self) -> &[u8] {
        &self.buffered[self.head_len..]
    }
}

/// Reads until the end of the request head, leaving the body unread.
pub(crate) async fn read_head<S: AsyncRead + Unpin>(
    stream: &mut S,
) -> Result<BufferedRequest, ServerError> {
    let mut buffered = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        if let Some((head, head_len)) = parse_head(&buffered)? {
            return Ok(BufferedRequest {
                head,
                buffered,
                head_len,
            });
        }
        if buffered.len() >= MAX_HEAD_BYTES {
            return Err(ServerError::InvalidRequest(format!(
                "request head is larger than {MAX_HEAD_BYTES} bytes"
            )));
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(ServerError::InvalidRequest(
                "connection closed before the request head was complete".into(),
            ));
        }
        buffered.extend_from_slice(&chunk[..read]);
    }
}

/// Parses a complete request head, or reports `None` while more bytes are needed.
fn parse_head(buffer: &[u8]) -> Result<Option<(RequestHead, usize)>, ServerError> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut request = httparse::Request::new(&mut headers);
    let head_len = match request.parse(buffer) {
        Ok(httparse::Status::Complete(head_len)) => head_len,
        Ok(httparse::Status::Partial) => return Ok(None),
        Err(error) => {
            return Err(ServerError::InvalidRequest(format!(
                "malformed HTTP request: {error}"
            )))
        }
    };
    let head = RequestHead {
        method: request.method.unwrap_or_default().to_owned(),
        path: request.path.unwrap_or_default().to_owned(),
        headers: request
            .headers
            .iter()
            .map(|header| {
                (
                    header.name.to_ascii_lowercase(),
                    String::from_utf8_lossy(header.value).into_owned(),
                )
            })
            .collect(),
    };
    Ok(Some((head, head_len)))
}

/// Reads the request body, continuing from the bytes that arrived with the head.
pub(crate) async fn read_body<S: AsyncRead + Unpin>(
    stream: &mut S,
    request: &BufferedRequest,
) -> Result<Vec<u8>, ServerError> {
    if request.head.header("transfer-encoding").is_some() {
        return Err(ServerError::InvalidRequest(
            "chunked request bodies are not supported".into(),
        ));
    }
    let length = request.head.content_length()?;
    if length > MAX_BODY_BYTES {
        return Err(ServerError::InvalidRequest(format!(
            "request body is larger than {MAX_BODY_BYTES} bytes"
        )));
    }
    let mut body = request.body_prefix().to_vec();
    body.truncate(length);
    while body.len() < length {
        let mut chunk = vec![0; (length - body.len()).min(8 * 1024)];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(ServerError::InvalidRequest(
                "connection closed before the request body was complete".into(),
            ));
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(body)
}

pub(crate) async fn write_json_response<S: AsyncWrite + Unpin>(
    stream: &mut S,
    status: Status,
    body: &[u8],
) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        status.line(),
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await
}

/// A stream that replays already-read bytes before continuing with the real one,
/// so a handshake the server has peeked at can still be parsed from the start.
pub(crate) struct PrefixedStream<S> {
    prefix: Vec<u8>,
    offset: usize,
    stream: S,
}

impl<S> PrefixedStream<S> {
    pub fn new(prefix: Vec<u8>, stream: S) -> Self {
        Self {
            prefix,
            offset: 0,
            stream,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for PrefixedStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let remaining = &this.prefix[this.offset..];
        if remaining.is_empty() {
            return Pin::new(&mut this.stream).poll_read(context, buf);
        }
        let take = remaining.len().min(buf.remaining());
        buf.put_slice(&remaining[..take]);
        this.offset += take;
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for PrefixedStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(context, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(context)
    }
}
