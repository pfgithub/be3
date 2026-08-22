# Adding a game

A deterministic game is a crate of its own that compiles to a single
WebAssembly module. The app never links a game: it finds the modules staged
beside it, runs them through the `wasmi` interpreter in
`crates/tabletop_games/host`, and asks each one what a player currently sees.
That is the same on desktop, Android and the browser, so a game is written
and built once.

Everything about games lives under `crates/tabletop_games`, split by which
side of the WebAssembly boundary it runs on:

- `api` (`game-api`) is the contract, and is compiled into every game
  module. It holds the types the two sides exchange, the `GameHelper` a game
  is written against, the `game!` macro that declares a module's exports,
  and the build script helper each game calls. It has to build for
  wasm32-unknown-unknown, so it depends on nothing that needs a host.
- `host` (`game-host`) is the other side: it embeds `wasmi`, loads a module,
  and calls it. Only the app depends on it. It re-exports the `game-api`
  types it passes across, so a caller needs the one crate.
- `rules/<game>` is one game each - the only place a game's actual rules
  live.

## 1. Write the crate

- `crates/tabletop_games/rules/foo/Cargo.toml` — copy `tic_tac_toe`'s. The
  package name is the module's file name and the game's id, so it is written
  with underscores. It needs `crate-type = ["cdylib", "rlib"]`, `game-api`
  as a dependency and a build dependency, and `game-host` as a dev
  dependency so the tests can run the module.
- `crates/tabletop_games/rules/foo/build.rs` —
  `fn main() { game_api::build::wasm(); }`. This compiles the crate to
  wasm32-unknown-unknown and points `GAME_WASM` at the module for the tests.
- `crates/tabletop_games/rules/foo/src/lib.rs` — the game, ending in
  `game_api::game!("Foo", foo);`. The first argument is the display name the
  app shows and stores on the blocks made from it; the second is the game
  function.
- `crates/tabletop_games/rules/foo/rulebook.md` — the rules in English,
  written the way a rulebook explains a game: who plays, what the pieces or
  cards are, how a turn goes, how it ends. Write this one first, because the
  code is meant to read like it.
- Root `Cargo.toml` — add `crates/tabletop_games/rules/foo` to the members.

Anything a game depends on has to build for wasm32-unknown-unknown with no
host to call into: no clock, no filesystem, no randomness. Whatever a game
would want randomness for, it seeds from the action log instead, the way
`crazy_8s` seeds its shuffle from the ids of the players who joined - two
clients replaying the same log must reach the same screen.

## 2. Write the game

A game is one straight-line function over its action log:

```rust
fn foo(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    loop {
        helper.action(
            |player| describe(player),
            |player, action| {
                if action("Do the thing") {
                    // the log records this move for this player
                }
            },
        )?;
    }
}
```

`helper.action` blocks on a player until the log supplies their move, so the
game reads like a normal loop rather than a replay pass. Each legal move is
offered by calling `action(label)`; when it answers `true`, that move is the
one the log records next for that actor, so the state updates happen right
there, inline. A move is identified by its position among the `action` calls
reached for its actor, never by its label, so nothing a client sends can name
a move it was not offered. When the log runs out, `action` returns the screen
for the viewing player, which `?` propagates out.

Some of what a rulebook says in one sentence is a paragraph of Rust, so
`GameHelper` says those the short way too:

- `helper.gather(2)?` fills a table before play begins - anyone may join,
  and once two have, any of them may start - and answers with the players in
  the order they joined.
- `helper.turn(whose, yours, theirs, |choose| ...)` waits on the one player
  whose turn it is, who is offered the moves `choose` names; everyone else
  is only told they are waiting.
- `helper.game_over(|player| ...)` ends the game: every viewer is told how
  it ended and nobody is offered anything.

`game_api::cards` and `game_api::table` are the same idea for card games:
a deck of the usual 52, and a `Table` that deals it, holds each player's
hand, the draw pile and the discard pile, draws (reshuffling the discard
pile back under the face-up card when the draw pile runs out), counts
passes and passes the turn to the left. Its shuffle is seeded from the ids
of the players it deals to, so it needs no randomness of its own.

These all live in `game-api` rather than in a game, so put anything new of
the same sort there too: whenever a rulebook sentence is plain and the Rust
for it is not, the sentence belongs in the shared crate and the game keeps
only its own rules. `crazy_8s` is written that way - one `lib.rs` that
follows its `rulebook.md` section by section - so what is left in it is
what makes Crazy 8s that game: eights are wild, a played eight calls a
suit, and drawing gets you one card you may play.

The game keeps no state between calls: the log is replayed from the top every
time, in a fresh instance, and the interpreter cuts a module off that never
returns.

## 3. Test it

Tests live in the game crate (`src/tests.rs` plus one file per test under
`src/tests/`) and drive the module rather than the Rust source:

```rust
fn show(actions: &[GameAction], player: Uuid) -> GameScreen {
    static GAME: OnceLock<Game> = OnceLock::new();
    GAME.get_or_init(|| Game::load(include_bytes!(env!("GAME_WASM"))).unwrap())
        .show(actions, player)
        .unwrap()
}
```

so what is tested is the artifact that ships. Pure helpers (a deck, a win
check) are still tested directly. A guest panic has nowhere to print, so it
reaches the test as a trap rather than a message.

## 4. Nothing else

`./scripts/build` finds every crate under `crates/tabletop_games/rules` for
each target, stages the modules in `games/` beside the app with an index the
browser and Android read in place of listing a directory, and the app
compiles what it finds at startup. The block stores the id of the module it plays and the name
that module gave itself, so a client without the module still names the
block, and a game that is not installed is reported in its place.
