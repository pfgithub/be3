# Block plugin protocol

Every wire message is a four-byte, network-order payload length followed by a
bincode payload. A frame is bounded only by that length, so a message carrying
a whole block or an imported file needs no chunking. Receivers reject
truncated or trailing data, unknown message variants, and values exceeding the
published string, collection, or opaque-descriptor limits. Implementations
must also bound pending messages to `MAX_QUEUED_MESSAGES` and requests to
`REQUEST_TIMEOUT_MILLISECONDS`.

The accepted handshake carries the theme the host is drawn in, which is what
a plugin's own interface follows, so an editor looks the same as the app it is
embedded in wherever it runs.

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
defined by the session layer; it may not reorder retained messages. A request
whose message is coalesced away is no longer waited on, since nothing will
answer a message that never went out. A timeout, malformed frame, invalid
ordering, or unsupported capability is reported with a structured error
before disconnect when the transport remains usable.

A message belongs either to the session - the handshake, acknowledgements,
errors, ping and shutdown - or to what the host is showing. The session owns
exactly the first kind and rejects anything else, so a receiver routes on that
distinction rather than on a list of the messages it happens to expect. What
is left is delivered to the host in the order it arrived, whatever class each
message belongs to: a block a plugin wrote and the editor message that
depends on it cannot swap places on the way.

Input events carry pointer, wheel, key, text, focus and zoom-gesture input in
the region's own logical coordinates. A pinch or trackpad zoom is its own
event, carrying the factor the view is asked to grow by; a wheel turned with
the zoom modifier held stays a wheel event, which the receiver reads as a zoom
of its own. Consecutive zoom gestures coalesce by multiplying their factors.

A region's input goes out at the start of the host frame that received it,
before the host lays anything out, and it is routed against where the region
sat and what covered it the last time the region was drawn. The host has that
much of its own frame left to spend, so a plugin answering promptly is drawn
into the frame that delivered the input rather than the one after it. A region
the host did not draw last frame is sent nothing.

Client messages tunnel the block protocol between an editor instance's own
block client and the host's server connection. Their payloads are opaque JSON
of any length: the host forwards them in both directions without interpreting
them, and the server treats each instance as a separate client of the one
connection.

An editor whose block cannot be made until something has been filled in is
opened as a creation dialog instead of on a block: it has a client of its own
but no block, and draws in its main region only. It reports whether the dialog
has been filled in, which is what lets the user accept it. On acceptance the
host asks the instance to commit, and the instance creates the block through
its own client and answers with the block's id, or with why it could not be
created; the host then opens the block it was given.

An editor whose block type generates dynamic artifacts is also opened on an
artifact: an instance with a client of its own, the artifact's block and the
opaque settings the host stores on it. The instance answers with the block the
artifact was generated from and a short description of what the settings
produce, or with why they cannot be read, and repeats that answer whenever the
settings change. The host sends the settings again whenever it changes them
itself; the instance's settings region edits its own copy and reports every
edit, which the host stores on the artifact only once the user applies it. A
regeneration request carries the settings to rebuild from and is answered
exactly once, after the instance has written the artifact through its own
client, with success or with why it failed.

An editor instance may ask for the cursor shown over one of its regions,
which only the host can put on the window. The request names what the cursor
means rather than any one toolkit's spelling of it, is sent only when the
cursor changes, and is honoured only while the pointer is over that region.

An editor instance may ask the host to open another block in its own tab.
The host decides whether to honour the request; it is not answered, and a
request for a block the host cannot open is discarded.

An editor instance may ask the host to choose a file for it, which only the
host can do on every platform the app runs on. The request carries the filter
the picker offers and is answered exactly once, with the file the host read,
the picker the user closed, or why the file could not be read. Requests are
identified per instance and may be outstanding together.

An editor instance may ask the host to fetch a URL for it, which is the only
network a plugin has: a plugin runs with no sockets of its own, and its block
client already reaches the server through the host's connection. The request
carries the URL and is answered exactly once, with the body the host read or
with why it could not be read. Requests are identified per instance and may be
outstanding together. The host refuses a URL that is not https, or whose host
name is not one the plugin's manifest names, and reports the refusal as an
ordinary failure rather than as a protocol error.

The host describes its registered block types once per plugin runtime, before
the first editor instance is opened, so an editor can name and illustrate
blocks it only holds a reference to. Each description carries the block type,
its display name, and the codepoint of the host's icon font.

The host reports a block dragged over an editor instance's region in that
region's own logical coordinates, once per frame while it hovers, once more
when it is let go, and reports that the drag has moved off the instance again.
An instance answers with whether it would take the block, which only decides
the cursor the host shows; a drop is delivered whether or not it was accepted.

An editor may be given a preview region as well as the regions it is edited
in. The host maps that region onto whatever quad it paints the block on, so
the editor fills the region and lets the host place, rotate and fade it. An
instance may also report the shape of its block, which the host holds its
preview to; it is a request in the same sense as the embedded size below.

The host owns pan and zoom. Wherever an editor's content has a view of its
own, the host works out the rectangle that content goes in and tells the
instance, in the logical coordinates of its main region; the editor draws its
content there and keeps no zoom or offset of its own. An instance moves the
view only by asking - pan by a distance, zoom by a factor about a point, or
fit - and the host answers by moving whatever viewport the instance is being
shown in, which may be a tab of its own or a canvas the block sits in. An
instance never told where its view is fills the region it is given.

An editor instance may report the size it would like to be given wherever the
host embeds it. It is a request, not a constraint: the host may embed the
instance at any size, and falls back to its own default until an instance
reports one.

An editor instance may embed other blocks' editors inside one of its regions.
Once per frame the instance publishes, for that region, the frame generation
it drew, an ordered list of the children it placed and the occluder rectangles
declared between them, all in the region's own logical coordinates. A child
carries its own identifier, the block and block type it shows, the rectangle
and clip it was placed at, the corner radius of the hole cut for it, whether
it composites below or above the instance's own pixels, whether the host
should draw it as a preview, a passive editor, an active one, or a live one,
and which part of that child's editor belongs in the rectangle: the block
itself, or the toolbar or one of the sidebars its editor draws around it.
Occluders are ordered against the children: a child's interactive area is
its rectangle minus every occluder declared after it, which is the same
subtraction that decides what the host draws over it.

A child placed below is cut out of the instance's own surface, which is
cleared transparent, so the host composites the child's editor under the
instance and anything the instance draws after the child covers it again. A
child placed above composites against the instance's pixels instead and cannot
be drawn over. The host draws the children below a region, then the region,
then the children above it, at the placements belonging to the generation it
is presenting; a placement published for a generation the host is not
presenting is stretched into the region it is showing rather than dropped.

A child placed for a part other than the block itself is that child's own
chrome: the host draws the toolbar or the sidebar that editor would be given
in a tab of its own, in the rectangle it was placed at, and reports back the
size it took as the size that child asks for, so an instance editing a block
can put the block's own chrome in its regions and lay the rest out around it.
Chrome is live from the moment it is placed, is never drawn as a preview, and
is not drawn in a preview region, where a block has no chrome at all.

The host answers with a status per child: whether the block could be opened
at all, the size and shape its editor asks for, whether its editor offers a
left or a right sidebar, whether the pointer is over it, whether it is being
given input, and why it is unavailable when it is. A block already open
above the instance is refused, so an editor cannot be nested inside itself.
The host bounds how many children of one region it will run as editors and
falls back to preview rendering for the rest.

A child drawn as a preview is a picture the instance asks the host for rather
than something the host composites over it. The instance publishes a preview
layout: one image it has allocated, with a rectangle in it for every preview
child of every one of its regions, the scale those rectangles are measured at,
and the generation of the layout. It transfers that image the way it transfers
the surface it draws its own regions on. The host draws each of those children
into the rectangle it was given, at the scale the layout declares, and tells
the instance which generation it has drawn; the instance then samples the
image where it placed the child and composites the preview itself, clipped,
scaled and blended like anything else it draws. No pixels cross the
connection: the picture is one shared image, written by the host and read by
the plugin. Until the host has drawn a generation, and for a few frames after,
it also composites that preview over the instance, so a child is never missing
while the two sides settle on a layout. A plugin whose platform cannot share
an image back publishes no layout at all, and the host composites every
preview over it as before. A layout carries at most 64 previews and no edge
longer than 4096 pixels.

The host owns input routing. Nothing is delivered to a child until the
instance publishes it as active, so an instance keeps the clicks over its
passive children; while a child is active the host stops delivering pointer
input over the child's interactive area to the instance. Keyboard and text
input go to the innermost active editor, and the cursor comes from the
innermost editor the pointer is over. Promotion to active is the user's, so
the host takes it away again when the user presses Escape or clicks outside
the child. A live child is the instance's own content rather than something
the user promoted - the whole of what the region shows - so it is given input
from the moment it is placed and the host never takes that away.

An editor instance may ask the host to choose a block for it, which is what
lets an instance acquire a child without waiting for one to be dragged in. The
request carries the name of what is being chosen, the block types that may be
chosen, empty for any, and whether the picker should open on the templates the
host offers, and is answered exactly once, with the block the user chose or
created, the picker the user closed, or why the block could not be made.
Requests are identified per instance and may be outstanding together.

An editor instance may ask the host to present it: to give its main region the
whole window and show nothing else of the app, which is what a slideshow or a
video played back full screen wants. The host owns the answer and reports the
state it settled on, since it may refuse, and takes presenting away again when
the instance is no longer the one on screen. An instance draws its main region
the same way either way; only the size it is given changes.

An editor instance may tell the host that one of the regions its manifest
declares has nothing in it at the moment - a sidebar that is only ever its
child's sidebar, when that child has none - and the host then shows that
region no more than a region the plugin never declared. Every declared region
is shown until the instance says otherwise, and the manifest still bounds
which regions an instance may ever be given.

An editor instance may report named duration and count measurements under a
named performance group. Durations are measured by the plugin and sent as
nanoseconds; the host only collects and displays the reported values. Reports
are informational, require no response, and may arrive independently of a
rendered frame.

Neither peer polls the other: a plugin runs beside the host - a process of its
own on the desktop, a worker of its own in the browser - and either side may
send at any time.

The host decides when a plugin draws. A plugin paints only when the host asks
it for a frame, and never on its own: when a message changes what it shows, or
something inside it wakes it, it says a frame is needed and waits to be asked.
It says so once and not again until it has painted, so a plugin that hears
about a pan sixty times before it can draw once paints one frame carrying all
sixty rather than one frame each. Every published frame carries how long the
plugin wants to wait before it is drawn again, absent when it wants nothing
further, and the host holds that request until the delay comes due. The host
asks at the start of its own frame, along with the input it is routing, since
input it delivers is a change the plugin will want to show. Only one request is
outstanding at a time, so a plugin that draws more slowly than the host is
asked again only once the frame it owes has arrived, and a plugin animating
without any delay is drawn once per host frame rather than as fast as its
process can run. A request left unanswered for a second is asked again. A
plugin the host is not drawing is asked for nothing.

The host shows whatever the plugin has published by the time it finishes
building its own frame. Which part of the surface each region takes is settled
then as well, against the layout that arrived with the frame being shown, so a
plugin that republishes its layout mid-frame is never drawn through the
placements of the one before it.

A surface carries the role it serves: the screens the plugin draws its regions
on, or the previews the host draws into for it. They are separate images with
separate generations, transferred by the same mechanism in the same direction,
and only the drawing runs the other way.

A surface is transferred as native graphics resources, never as pixels: the
Windows mechanism shares D3D12 textures and a fence, and the Linux one shares
the dma-buf memory of each image. A surface is a rotation of images rather
than one image, because the host goes on showing the frame it last took while
the plugin draws the next one: the descriptor declares one attachment per
image, the plugin draws into them in turn, and every published frame names the
image it was drawn into, which is the one the host samples until another frame
arrives. A plugin never draws into the image it last published, nor into the
one before that, so nothing the host is showing or has only just stopped
showing is written while it is being read. A previews surface is a single
image, since the host draws it and the plugin only reads it. A Linux surface
carries no fence, so the plugin publishes a frame only once the work writing it
has retired, and the monotonic synchronization value in its descriptor tells
the host which frame it is looking at rather than what to wait on.

Surface messages may declare at most 16 native attachments. Each declaration
records the resource type and whether ownership is borrowed or transferred.
Declaration order is the attachment order in the platform carrier. A missing,
extra, reordered, or unexpected attachment rejects the frame and closes every
received native resource. Unix carriers use ancillary file descriptors with
close-on-exec enabled. Windows carriers duplicate handles into the verified
peer process without inheritance and associate them with the immediately
following protocol frame.
