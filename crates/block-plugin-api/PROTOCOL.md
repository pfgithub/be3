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
the screen's own logical coordinates. A pinch or trackpad zoom is its own
event, carrying the factor the view is asked to grow by; a wheel turned with
the zoom modifier held stays a wheel event, which the receiver reads as a zoom
of its own. Consecutive zoom gestures coalesce by multiplying their factors.

A screen's input goes out at the start of the host frame that received it,
before the host lays anything out, and it is routed against where the screen
sat and what covered it the last time the screen was drawn. The host has that
much of its own frame left to spend, so a plugin answering promptly is drawn
into the frame that delivered the input rather than the one after it. A screen
the host did not draw last frame is sent nothing.

Opening an instance carries the account and workspace its block client speaks
for, and the id of the app client the host itself is: a per-installation
identity a block that stores one setting per client - the settings block -
resolves against, which the instance's own tunnelled client cannot supply
because the host opens a fresh one for every runtime.

Client messages tunnel the block protocol between an editor instance's own
block client and the host's server connection. Their payloads are opaque JSON
of any length: the host forwards them in both directions without interpreting
them, and the server treats each instance as a separate client of the one
connection.

An editor whose block cannot be made until something has been filled in is
opened as a creation dialog instead of on a block: it has a client of its own
but no block, and is given a frame with no chrome bands. It reports whether the dialog
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

An editor instance may report the area its text input occupies and where its
caret sits inside it, in the region's own logical coordinates, which is what an
operating system's input method needs to place its candidate window. Only the
host can tell the window that, and it maps the area through the placement it
last drew the region at. The host sends the composition back the other way, as
input events of their own alongside the text it already delivers: an input
method turning on, the text being composed, the text it settled on, and its
turning off again.

Presence belongs to whoever is looking, and what is looking is the host: a
plugin's block client is carried over the host's connection but is a client of
its own to the server, so anything it published would be attributed to someone
else. The host therefore tells an editor instance whether its block is being
viewed and hands over the presence the block currently carries - every entry's
client, kind and opaque value - whenever any of that changes, and publishes on
the instance's behalf whatever the instance asks it to, under its own client.
An instance that wants no cursor of its own ignores both. The host also asks an
instance to reveal one client's cursor, when the user picks that person out of
the list of people viewing the block, which only the instance can do because
only it knows where in its own content that cursor is; the client it names is
one of the clients the presence it was handed came from.

An editor instance may be asked to replace one of the blocks it references with
another - a copy the host made of a block being edited in two places at once -
which the host cannot do itself for a block type whose references live inside
its own content. The request names the block to replace and the one to put in
its place, and is answered exactly once with whether the instance made the
change. An instance that answers that it did not leaves the host to make the
replacement through the block's own structural edit.

An editor instance may ask the host to open another block in its own tab.
The host decides whether to honour the request; it is not answered, and a
request for a block the host cannot open is discarded.

An editor instance may ask the host to choose a file for it, which only the
host can do on every platform the app runs on. The request carries the filter
the picker offers and is answered exactly once, with the file the host read,
the picker the user closed, or why the file could not be read. Requests are
identified per instance and may be outstanding together.

An instance the host resizes - one embedded where the user drags its corner,
which the manifest's resize mode allows - is told the size it was given, in
its own points. Only an editor whose block records its size answers by
writing it; the rest ignore it and keep reporting the size they want.

Files the user drags onto a region reach the instance the way a dragged block
does: while they are only hovering the instance is told where they are and
nothing else, and on the drop it is told the same position along with each
file's name and the bytes the host read. Only the host can read a file on
every platform, so a plugin never sees a path.

An editor instance may ask the host for the image on the clipboard, which
only the host can read on any platform. A paste reaches the instance as a
paste event of its own rather than as ordinary typed text, so an editor that
takes images can tell one from the other; the request is answered exactly
once, with the image the host read, with nothing when the clipboard holds no
image, or with why it could not be read.

An editor instance may ask the host to play the audio in one of its blocks,
which only the host can do: a plugin has no sound device on any platform, and
an audio file is far larger than a message may carry. The request names the
block rather than its bytes, which the host already holds through its own
client. The host answers with what its player is doing - whether it is
playing, where it is, how long the audio is once it knows, and why it could
not play - and repeats that answer whenever any of it changes.

An editor instance may ask the host to fetch a URL for it, which is the only
network a plugin has: a plugin runs with no sockets of its own, and its block
client already reaches the server through the host's connection. The request
carries the URL and is answered exactly once, with the body the host read or
with why it could not be read. Requests are identified per instance and may be
outstanding together. The host refuses a URL that is not https, or whose host
name is not one the plugin's manifest names, and reports the refusal as an
ordinary failure rather than as a protocol error.

An editor instance may ask the host for a web view, which is a window of the
operating system's own laid over the app rather than anything a plugin could
draw: it opens one at a URL, says each frame where inside its own screen it
goes and whether it is shown at all, and asks it to navigate, reload or hand
the keyboard back to the app. The host maps that rectangle through the
placement it last drew the instance's screen at, hides the view while the
instance is not being drawn, and closes it with the instance. It reports back
what the page does - a navigation started or finished, a history entry pushed
or replaced, a title, a history traversal the page asked for, a window it
wanted to open, the address the view is now at - and why anything it was asked
for could not be done. A platform with no embedded browser answers the first
request with that failure and nothing else.

An editor instance may ask the host to hold the pointer still and hide it, so
an editor that looks around a scene reads motion rather than a position. Only
the host can do that: the window is its own. The request says whether the
instance wants the cursor held, the host holds it while any instance does and
lets it go as soon as none does, and an instance that closes lets go with it.
While it is held the host reports raw pointer motion to the focused instance
alongside the events it already sends, since there is no position to report.

An editor instance may ask the host to read one of the app's own files for it,
which is how a plugin reaches what was installed beside the app rather than
built into it: the games the deterministic game editor runs are wasm modules
the app ships alongside its own plugins. The request names a file and is
answered exactly once, with its bytes or with why it could not be read. The
host resolves the name against the directory it loads its plugins from - the
executable's own directory, the bundle root in the browser, the asset root on
Android - and refuses a name that is not a relative path inside it, reporting
the refusal as an ordinary failure rather than as a protocol error. A plugin has
no file system of its own on any platform, and nothing of the user's is
reachable this way.

The host describes its registered block types once per plugin runtime, before
the first editor instance is opened, so an editor can name and illustrate
blocks it only holds a reference to. Each description carries the block type,
its display name, and the codepoint of the host's icon font.

The host reports a block dragged over an editor instance's region in that
screen's own logical coordinates, once per frame while it hovers, once more
when it is let go, and reports that the drag has moved off the instance again.
An instance answers with whether it would take the block, which only decides
the cursor the host shows; a drop is delivered whether or not it was accepted.

An instance is shown through screens of three kinds: a frame, a preview, and
an artifact-settings region. A frame is the whole rectangle an editor is
edited in, and an instance has at most one; the plugin lays its own toolbar
row, sidebars and content out inside it, in one pass of one context. The host
says, per frame, whether the instance owns the chrome bands this frame, where
its content goes inside the frame, and the trail of editors it sits under. An
instance told it owns no chrome draws its content alone: that is an editor
embedded in another one, a creation dialog, or an instance presenting. An
instance told it owns the chrome and given no content rectangle fills its own
content band, which is what a tab of its own means.

A frame surface is transparent wherever its owner does not paint, so whatever
the host draws beneath shows through there. The plugin publishes, once per
frame, the content rectangle it laid out, the rectangles it painted and the
rectangles its floating layers took, all in the frame's own logical
coordinates. The host works out the view it hands a pan-and-zoom editor
against the content it was told about, delivers no pointer input over the
parts of the frame the instance did not paint, and composites the frames of
one tab bottom-up: every frame's painted area first, then every frame's
floating rectangles, so a menu opened by an editor underneath still falls over
the frame above it. The two passes are cut into disjoint rectangles, so no
pixel is blended twice.

An instance whose chrome is only reserved keeps the bands exactly where they
were and paints nothing in them, so selecting or leaving a child moves neither
its content nor the view it holds. The instance that draws the chrome also
draws the trail back out of it, and reports that the user asked to leave; the
host then hands the frame back to the parent.

An editor may be given a preview screen as well as the frame it is edited in.
The host maps that screen onto whatever quad it paints the block on, so the
editor fills it and lets the host place, rotate and fade it. An instance may
also report the shape of its block, which the host holds its preview to; it is
a request in the same sense as the embedded size below.

The host owns pan and zoom. Wherever an editor's content has a view of its
own, the host works out the rectangle that content goes in and tells the
instance, in the logical coordinates of its frame; the editor draws its
content there and keeps no zoom or offset of its own. An instance moves the
view only by asking - pan by a distance, zoom by a factor about a point in
those same frame coordinates, or fit - and the host answers by moving
whatever viewport the instance is being shown in, which may be a tab of its
own or a canvas the block sits in. An instance never told where its view is
fills the content it is given.

An editor instance may report the size it would like to be given wherever the
host embeds it. It is a request, not a constraint: the host may embed the
instance at any size, and falls back to its own default until an instance
reports one.

An editor instance may embed other blocks' editors inside one of its screens.
Once per frame the instance publishes, for that screen, the frame generation
it drew, an ordered list of the children it placed and the occluder rectangles
declared between them, all in the screen's own logical coordinates. A child
carries its own identifier, the block and block type it shows, the rectangle
and clip it was placed at, the corner radius of the hole cut for it, whether
it composites below or above the instance's own pixels, and whether the host
should draw it as a preview, a passive editor, an active one, or a live one.
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

A child the user has selected, or one an instance declares as the whole of
what it shows, becomes a frame owner of its own: it is handed the same frame
as the instance that placed it, takes over every chrome band, and draws its
content at the rectangle the placement gave it. Its parent is told for that
frame that it owns no chrome, and keeps its content exactly where it was, so
selecting a child moves nothing. Handover is always a replacement: a child's
chrome is never nested inside its parent's, and the framework, not the plugin,
draws the trail back out.

The host answers with a status per child: whether the block could be opened
at all, the size and shape its editor asks for, whether the pointer is over
it, whether it is being given input, and why it is unavailable when it is. A block already open
above the instance is refused, so an editor cannot be nested inside itself.
The host bounds how many children of one region it will run as editors and
falls back to preview rendering for the rest.

A child drawn as a preview is a picture the host draws rather than an editor
it runs: the instance declares the child as a preview and leaves a hole for
it, and the host composites the block's own preview into that hole, clipped
to the rectangle the child was placed at.

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

An editor instance may ask the host to present it: to give it the whole window
and show nothing else of the app, which is what a slideshow or a video played
back full screen wants. The host owns the answer and reports the state it
settled on, since it may refuse, and takes presenting away again when the
instance is no longer the one on screen. A presenting instance is a frame owner
with no chrome bands, so it draws its content the same way either way; only the
size it is given changes.

An editor instance may report named duration and count measurements under a
named performance group. Durations are measured by the plugin and sent as
nanoseconds; the host only collects and displays the reported values. Reports
are informational, require no response, and may arrive independently of a
rendered frame.

Neither peer polls the other: a plugin runs beside the host - a thread of its
own under wasmtime, a worker of its own in the browser - and either side may
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

Every screen an instance is given is packed into one surface for the whole
plugin runtime, in two dimensions rather than in a single column, since a
frame-sized slot would otherwise run past the largest texture the device
allows. Which part of the surface a screen takes is the layout the plugin
publishes and the host samples.

A plugin never learns where its pixels live. It asks its host for a render
target and draws into an opaque texture, which is the host's own under
wasmtime and the worker's offscreen canvas in the browser; the frame it
publishes names the generation of the layout it drew, and the host shows that
texture until another frame arrives. No pixels and no native graphics
resources cross the connection.
