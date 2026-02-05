# TODO

## HTML Parser

- [x] **Raw text element handling** - Implemented RCDATA and RAWTEXT states for `<title>`, `<textarea>`, `<style>`, `<xmp>`, `<iframe>`, `<noembed>`, and `<noframes>` elements. Content inside these elements is now correctly treated as text and not parsed as HTML markup.

- [ ] **Script element handling** - `<script>` elements require the more complex ScriptData state machine with escape sequences. Currently left unimplemented.

- [ ] **Character references** - Character references (like `&amp;`, `&#38;`, `&#x26;`) are not yet parsed. They are passed through as literal text.

## Future Work

- [ ] CSS style application to rendered content
- [ ] Browser history (back/forward navigation)
- [ ] HTTP/HTTPS URL loading
- [ ] Tab support
- [ ] **Font infrastructure refactoring** — Move `FontMetrics` trait from `koala-css` to `koala-common`, then consolidate font loading (search paths, `FontProvider`, `FontdueFontMetrics`) into `koala-common::fonts`. This unblocks both headless and GUI renderers from depending on koala-browser for font infrastructure.
