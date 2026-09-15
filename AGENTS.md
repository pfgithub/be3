BE3 project

You are inside of an ubuntu VM. You may install / remove programs as needed. If the disk runs out of space, you may free up space.

Guides:
- guides/adding_a_block.md
- guides/adding_a_game.md
- guides/adding_a_plugin_editor.md
- guides/beui.md
- guides/beui_keyboard.md
- guides/pan_and_zoom.md
- guides/reactive.md
- guides/testing_a_gui.md

Do not:
- When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data. The project is still early, and it is fine to ask the user to delete all their data. The crash handler in block-app will offer this automatically.
- Do not use unicode symbols for icons, either use an icon library or no icon at all.
- Do not edit README.md. If it is out of date, you may say so in your handoff message.
- Don't use worktrees. If using subagents, run them sequentially rather than in parallel.

Verification:
- `./scripts/check`: Use this for fast compile feedback. It prepares non-Cargo prerequisites and checks the complete workspace with the feature unification the project expects. Prefer this over `cargo build` or `cargo check` directly.
- `./scripts/verify`: This is the primary full check and is required before committing. Run it after coherent changes and use a 10 minute timeout in the tool call arguments so it doesn't convert itself to a background task.
  - This will run all project tests and clippy lints
  - A plugin's own tests are not part of the workspace run: they are compiled to wasm and run through the plugin host by `scripts/internal/test-plugins.sh`, which verify calls for you. Run that script on its own for faster feedback on one editor (`scripts/internal/test-plugins.sh -p checklist`); it takes `cargo nextest run`'s arguments.
  - It will autofix formatting, clippy fixable rules, and it will autofix to enforce project-specific rules: It will delete all code comments & doc comments, it will structure test folders & files to the project's one test per file standard, it will automatically move+rename mod.rs files to be in the parent folder named after the folder instead, and it will format the bodies of `view!` macro calls (rustfmt cannot, because the body is not Rust syntax).
  - Use manual Cargo commands only for a narrow diagnostic after a supported script has exposed a failure. A package by itself may not be a valid build unit. Always finish with `./scripts/verify`.
- `PATH="/home/ubuntu/.local/android-build/gradle-8.11.1/bin:$PATH" ./scripts/build --target android --android-sdk /home/ubuntu/Android/Sdk`: run this for changes that affect features specific to Android.
- `./scripts/build --target web`: run this for changes that affect features specific to web
- `./scripts/run --smoke`: run this for changes that could affect native startup or runtime integration. It performs a bounded automated launch in a virtual display with isolated data; it does not require GUI interaction.
- You may run non-GUI binaries, examples, and headless test harnesses when they exercise changed behavior.
- Do not perform manual GUI verification, interact with the GUI app, or use the browser tool. Use headless tests from `guides/testing_a_gui.md` for GUI behavior.

Do:
- Use commit message format `type: message`. Include Co-Authored-By: (model name).
- When done, create a pull request on github for the change.
- You may push a change even if it still needs GUI verification or other verification that you are unable to perform.
- In your handoff message, mention any small issues you encoutered or small things you noticed that could make the code / application better.
- If you don't need tests in your search results, consider `grep --exclude-dir="tests"`
- Claude: If the harness says it will notify you when something finishes, for example a background command, you need to stop outputting / end the turn and it will wake you up when the results are ready.
