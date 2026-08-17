# Block plugin protocol

Every wire message is a four-byte, network-order payload length followed by a
bincode payload. Receivers reject payloads larger than `MAX_FRAME_BYTES`,
truncated or trailing data, unknown message variants, and values exceeding the
published string, collection, or opaque-descriptor limits. Implementations
must also bound pending messages to `MAX_QUEUED_MESSAGES` and requests to
`REQUEST_TIMEOUT_MILLISECONDS`.

The initial handshake advertises an inclusive supported-version range. Peers
may communicate only after selecting one version in the intersection. A peer
rejects an absent intersection with `UnsupportedVersion` and closes the
session.

Adding an optional capability or a message that cannot be sent before its
capability is negotiated is compatible within a protocol version. Changing a
message representation, ordering requirement, validation rule, or existing
semantic requires incrementing `PROTOCOL_VERSION`. Unknown message variants
are never silently ignored.

Messages with request identifiers are answered with the same identifier by a
response, `Acknowledged`, or `Error`. Lifecycle messages and input events are
ordered as sent. Backpressure may coalesce or discard only the event classes
defined by the session layer; it may not reorder retained messages. A timeout,
malformed frame, invalid ordering, or unsupported capability is reported with
a structured error before disconnect when the transport remains usable.
