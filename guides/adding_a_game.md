# Adding a game

A deterministic game is a crate of its own that compiles to a single
WebAssembly module. The app never links a game: it finds the modules staged
beside it, runs them through the `wasmi` interpreter in
`crates/deterministic_games`, and asks each one what a player currently sees.
That is the same on desktop, Android and the browser, so a game is written
and built once.

## 1. Write the crate

- `crates/games/foo/Cargo.toml` — copy `tic_tac_toe`'s. The package name is
  the module's file name and the game's id, so it is written with
  underscores. It needs `crate-type = ["cdylib", "rlib"]`, `game-api` as a
  dependency and a build dependency, and `deterministic_games` as a dev
  dependency so the tests can run the module.
- `crates/games/foo/build.rs` — `fn main() { game_api::build::wasm(); }`.
  This compiles the crate to wasm32-unknown-unknown and points `GAME_WASM`
  at the module for the tests.
- `crates/games/foo/src/lib.rs` — the game, ending in
  `game_api::game!("Foo", foo);`. The first argument is the display name the
  app shows and stores on the blocks made from it; the second is the game
  function.
- Root `Cargo.toml` — add `crates/games/foo` to the members.

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

`./scripts/build` finds every crate under `crates/games` for each target,
stages the modules in `games/` beside the app with an index the browser and
Android read in place of listing a directory, and the app compiles what it
finds at startup. The block stores the id of the module it plays and the name
that module gave itself, so a client without the module still names the
block, and a game that is not installed is reported in its place.
