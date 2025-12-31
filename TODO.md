# TODO

## HTML Parser

- [ ] **Raw text element handling** - The parser incorrectly parses content inside `<style>`, `<script>`, `<textarea>`, and `<title>` tags as HTML instead of raw text. Per the HTML spec, these elements should collect all text until their closing tag without interpreting it as markup. This causes `simple.html` to render incorrectly because the CSS content `body {` is interpreted as an HTML `<body>` tag.

## Future Work

- [ ] CSS style application to rendered content
- [ ] Browser history (back/forward navigation)
- [ ] HTTP/HTTPS URL loading
- [ ] Tab support
