# LikeNotepad.exe — notes for Claude

A classic-Notepad-style plain text editor for macOS: Tauri 2, Rust native shell
(`src-tauri/src/lib.rs`: menus, windows, alerts, file I/O, printing via AppKit/objc2) around
one `<textarea>` in WKWebView (`src/`). See `README.md` for features and layout,
`technical-lessons.md` for WebKit quirks already investigated (read it before touching the
selection highlight or drag-autoscroll).

The owner, Timur, writes in Russian and prefers answers in Russian. He does not want to
test things himself: verify your own changes.

## Build and run

- `npm run dev` / `npm run build` (Node for arm64 lives in `~/.local/node`, linked from
  `~/.local/bin`; the Intel Node in `/usr/local` does not run here — no Rosetta).
- Without npm: `cargo tauri build --bundles app` (tauri-cli 2.11.2 in `~/.cargo/bin`),
  `cd src-tauri && cargo run` for a debug run.
- After Rust changes: `cd src-tauri && cargo check` — keep it warning-free.
- JS syntax check without Node tooling: `new Function(src)` via `osascript -l JavaScript`.

## Verifying changes

- `tests/ui/run.sh` — end-to-end UI tests (selection highlight vs native, seams,
  drag-autoscroll, perf); `control` proves the seam detector works. Run the relevant cases
  after touching `src/app.js` selection/scroll code.
- For anything else, run the debug build and check it yourself: screenshots of the app's
  window only (`screencapture -l <windowID>`, never the whole screen — other apps hold
  private content), accessibility (AX) queries, window titles as a state channel.
- Debug builds run the script named by `LIKENOTEPAD_TEST_HOOK` before the page
  (`with_test_hook` in `lib.rs`); `window.__likeNotepad` then exposes `applyWordWrap` and
  `paintSelection`. Use a hook instead of editing `src/app.js` for experiments.
- Mouse/keyboard input through CGEvent goes to the frontmost app: check that the test
  window is in front before every input. Prefer hooks over typed keys.
- The debug app uses the real settings file
  (`~/Library/Application Support/me.polatov.notepad/config.json`): back it up before a test
  that changes settings and restore it after.
- Stop every process you started (debug app, dev server) when done.

## Shipping to Timur's Mac

After a verified change: commit, `npm run build -- --bundles app`, quit the installed app
politely (`osascript -e 'tell application id "me.polatov.notepad" to quit'`; if it stays
open, it has unsaved text — stop and ask), move the old
`/Applications/LikeNotepad.exe.app` to `~/.Trash` (never delete permanently), `ditto` the
new bundle in, `lsregister -f` it, launch it.

## GitHub

- Private repo https://github.com/polatov/likenotepad (branch `master`); `gh` 2.101 lives in
  `~/.local/bin`, logged in as `polatov`.
- Use `/usr/bin/git` for anything that talks to GitHub: the first `git` in PATH
  (`/usr/local/bin`, Intel Homebrew) cannot run on this Mac, and `gh` subcommands that shell
  out to `git` fail for the same reason. Credentials come from `gh` via the repo-local
  `credential.https://github.com.helper`; pushes need the repo-local
  `http.version=HTTP/1.1` (HTTP/2 pushes of a few MB fail with HTTP 400).
- Commits use the noreply address `1778456+polatov@users.noreply.github.com` (repo-local
  `user.email`); never commit with the personal address.

## Rules

- Conventional commits, one per feature, after self-verification; commit without asking.
- Never push, publish or create remote repositories without Timur's explicit yes.
- Product decisions (behaviour users see, what to show in menus/dialogs): ask, one
  question with options; technical choices: decide and mention.
- Menus mirror classic Windows Notepad: no extra items (macOS-added ones are switched off
  in `disable_system_menu_items`).
- Code comments and docs in English; UI strings live in `src/i18n.js` and
  `src-tauri/src/i18n.rs` (8 languages, keep them in sync).

## State (2026-09-29)

- Done: horizontal/vertical drag-autoscroll, self-painted selection highlight (no seams),
  UI tests, new app icon (source: Claude Design canvas "LikeNotepad.exe Icon"), proxy icon,
  one-time `.txt` default-app offer, About panel link to https://polatov.me/notepad/,
  title bar follows the theme.
- Released 1.0.0 (tag `v1.0.0`): pushing a `v*` tag makes `.github/workflows/build-dmg.yml`
  build an unsigned universal `.dmg` as a run artifact (no GitHub Release is published).
  To release: bump the version in `package.json`, `package-lock.json`,
  `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` (`cargo check` updates `Cargo.lock`),
  commit, tag, push the tag.
- Open: nothing planned; Apple Developer ID signing is out of scope for now.
- Project page for the blog: `docs/project-page/` (handled in a separate chat).
