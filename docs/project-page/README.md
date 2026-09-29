# Project page draft

Draft of the project page at https://polatov.me/notepad/ (a WordPress page), linked from
the app's About panel (`PROJECT_PAGE_URL` in `src-tauri/src/lib.rs`).

- `page.html` — the page body in Russian, to paste into a WordPress “Custom HTML” block;
  the comment at its top explains where the screenshots go. `[BRACKETED]` parts are
  placeholders (download link, version, source link).
- `post.html` — draft of the 1.0 announcement post (title, slug and excerpt in its top comment).
- `notepad-light.png`, `notepad-find.png`, `notepad-font.png` — screenshots of the app
  (light theme, Russian UI) with a sample note, for the page: main window, a found match,
  the Font window. Taken from a debug build with a `LIKENOTEPAD_TEST_HOOK` script that
  fills the note; `screencapture -l` of the Font window includes its parent window.
- `app-icon-1024.png` — the app icon (copy of `notepad_icon_1024.png`) for the page.
- `cowork-brief.md`, `cowork-prompt.txt` — a brief and an opening prompt (in Russian) for a
  separate chat that works on the blog.
