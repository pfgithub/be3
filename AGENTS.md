BE3 project

You are inside of an ubuntu VM. You may install / remove programs as needed. If the disk runs out of space, you may free up space.

Guides:
- guides/adding_a_block.md
- guides/adding_a_game.md
- guides/adding_a_plugin_editor.md
- guides/testing_a_gui.md

Do not:
- When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data. The project is still early, and it is fine to ask the user to delete all their data. The crash handler in block-app will offer this automatically.
- Do not use unicode symbols for icons, either use an icon library or no icon at all.
- Do not edit README.md. If it is out of date, you may say so in your handoff message.

Verification:
- ~5 minutes: `./scripts/verify`: always run this one before committing. This will run clippy --fix, cargo fmt, as well as enforcing project-specific rules: It will remove all code comments and format folder structure and tests.
- `PATH="/home/ubuntu/.local/android-build/gradle-8.11.1/bin:$PATH" ./scripts/build --target android --android-sdk /home/ubuntu/Android/Sdk`: run this for changes that affect features specific to Android.
- `./scripts/build --target web`: run this for changes that affect features specific to web
- Do not perform any manual GUI verification. Do not run the GUI app or use the browser tool.

Do:
- Use commit message format `type: message`. Add Co-Authored-By: (model name).
- After running verification, commit and push changes in git.
- You may push a change even if it still needs GUI verification or other verification that you are unable to perform.
