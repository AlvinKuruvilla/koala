# koala-font v0: font-wide metrics

The first slice of `koala-font`, the crate that replaces `fontdue`
(see `koala-std-roadmap.md`, Track-1). It reads the handful of
OpenType values CSS needs to place text and its decorations, and
nothing else: fontdue keeps rasterizing glyphs until `koala-font` can.

Written in learning-partner mode: this document says what to read and
what to build; the parsing code is yours.

## Why now

Layout and the renderer disagree about where the baseline is, because
neither asks the font. Measured on `css/hover` (2026-10-15):

| | offset from the top of a text fragment |
|---|---|
| layout's baseline (`inline.rs`, `ASCENDER_RATIO`) | 0.9 em |
| renderer's glyph baseline (`renderer.rs` `draw_text`) | 1.0 em |
| renderer's underline (`renderer.rs`, decorations) | 0.9 em |

So underlines cut 0.1 em into the letters. The fix needs the font's
real ascent, descent and underline position, and fontdue exposes no
underline metrics at all.

## What we are building

A function from font file bytes to these values, in font units, plus
`unitsPerEm` to scale them:

| value | table | used for |
|---|---|---|
| `unitsPerEm` | `head` | scaling everything below to pixels |
| `sTypoAscender`, `sTypoDescender` | `OS/2` | CSS ascent and descent metrics |
| `sTypoLineGap` | `OS/2` | CSS line gap metric (`line-height: normal`) |
| `ascender`, `descender`, `lineGap` | `hhea` | the same, when there is no `OS/2` table |
| `underlinePosition`, `underlineThickness` | `post` | underline placement |
| `yStrikeoutPosition`, `yStrikeoutSize` | `OS/2` | `line-through` placement |

It must open both a single font (`.ttf`, `.otf`) and one face of a
collection (`.ttc`): the first font Koala tries on macOS is
`/System/Library/Fonts/Helvetica.ttc`.

`koala-font` reads through a source the caller provides and does no
I/O itself; opening the file stays with the caller, per the layering
rule in `koala-std-roadmap.md`. See Decisions below.

### Left out on purpose

- glyphs: `cmap`, `hmtx`, `glyf`/`loca`, `CFF` (the next slices)
- table checksums and `head.checksumAdjustment`
- the TTC version 2.0 signature fields
- variable-font metric variations (`MVAR`). `SFNS.ttf` is variable, so
  its metrics would come out at the default instance
- `fsSelection` bit 7 (`USE_TYPO_METRICS`): CSS asks for the typo
  metrics whether or not it is set (see CSS Inline 3 below)

## What to read

All OpenType pages are Microsoft's copy of the spec, under
`https://learn.microsoft.com/en-us/typography/opentype/spec/`. Local
copies (HTML and extracted text) are in `tmp/spec/opentype/`.

### 1. `otff`: OpenType font file

- **Data Types**: the integer types, `Offset32`, `Tag`, `FWORD`. Note
  the byte order the whole format uses.
- **Organization of an OpenType Font**, then **Table Directory**: the
  header (`sfntVersion`, `numTables`, the three search fields you can
  ignore) and the `TableRecord` layout. Note what the record's
  `offset` is measured from.
- **Font Collections**, then **TTC Header**: version 1.0's fields
  only. Note where `tableDirectoryOffsets` point and what they are
  measured from.

Skip: Filenames, Table Version Numbers beyond a skim, Calculating
Checksums, everything about DSIG.

### 2. `head`

Read the field table down to `unitsPerEm`; that and `magicNumber` (a
cheap sanity check that you found the right table) are all we use.

### 3. `hhea`

`ascender`, `descender`, `lineGap`. Read the descriptions: note the
sign convention for `descender`.

### 4. `OS/2`

- **OS/2 Table Formats**: look at how the versions relate. Which
  fields exist in every version, and does a later version move any of
  them?
- **Field descriptions** for `sTypoAscender`, `sTypoDescender`,
  `sTypoLineGap`, `yStrikeoutSize`, `yStrikeoutPosition`. The
  `sTypoAscender` description says why these, not `hhea`'s, give
  portable layout.

### 5. `post`

The header's `underlinePosition` and `underlineThickness`, and what
`underlinePosition` measures (from where, to which edge of the line).
The glyph-name data after the header is not needed.

### 6. The CSS side: what the values are for

- CSS Inline 3 § 3.2.1 Ascent and Descent Metrics
  (`https://drafts.csswg.org/css-inline-3/#ascent-descent`):
  "It is recommended that implementations that use OpenType or
  TrueType fonts use the metrics sTypoAscender and sTypoDescender from
  the font's OS/2 table ... In the absence of these metrics, the
  "Ascent" and "Descent" metrics from the HHEA table should be used."
- CSS Inline 3 § 3.2.2 Line Gap Metrics
  (`https://drafts.csswg.org/css-inline-3/#font-line-gap`): "UAs must floor the line gap
  metric at zero."
- CSS Text Decoration 4 § 2.8.2 Using Font Metrics for Automatic
  Positioning
  (`https://drafts.csswg.org/css-text-decor-4/#line-auto-offset`): "The UA
  should use such font-based information in its choice of auto
  offset wherever appropriate".

## Questions to settle before writing code

Each has a recommendation and the strongest case against it. They are
yours to decide; the test harness gets written against whatever you
pick.

### Naming a face in a collection

Recommendation: a `u32` index, `0` for a single font, plus a way to ask
how many faces a file holds. That is what fontdue takes today
(`FontSettings::collection_index`), so swapping fontdue out later is
mechanical, and the test harness needs the count to walk every face of
every `.ttc` on the machine. Decide what index `1` means for a file that
is not a collection: an error, or ignored.

Against: an index means nothing to CSS. Font matching will want faces
by family, weight and style, which come from the `name` and `OS/2`
tables. That is a later slice, though, and it can be built on top of
indices.

### Borrowing or copying

Recommendation: both, at different levels. A face type that borrows the
file (`&'a [u8]`) and remembers where each table is, with a method that
copies the metrics out into a small `Copy` struct. The next slices
(`cmap`, `glyf`) read per-glyph data on demand and need the bytes kept;
ttf-parser and fontdue are both shaped this way. The struct keeps the
CSS side free of lifetimes.

Against: for this slice alone, a single function from bytes to the
struct is simpler, and koala-browser would hold a second borrowed
handle beside fontdue's `Font` until fontdue goes. If the face type
feels like ceremony now, the free function is defensible; the cost is
an API change at the `cmap` slice.

### What counts as an error

Recommendation: a typed error enum, since callers branch on the result
(the global guidelines prefer typed errors for that). Then sort each
failure by whether the font is still usable:

- **Error, the face is unusable**: an unrecognized `sfntVersion` or
  `ttcTag`; the data too short for the header or the table directory;
  the face index out of range; a table record pointing past the end of
  the data; `head` or `hhea` missing or too short, since nothing can be
  scaled or placed without them.
- **Absent, the caller falls back**: no `OS/2` table; no `post` table.
  So the typo, underline and strikeout values are `Option`s.
- **Short `OS/2` version 0**: the `OS/2` page warns that Apple's
  version 0 "stops at the usLastCharIndex field and does not include
  the last five fields", and that "Applications should check the table
  length for a version 0 OS/2 table before reading these fields". Find
  out which five fields those are; it decides whether the typo metrics
  are present.
- **Not checked**: checksums (left out above), and table records out
  of tag order. The spec says they "must be sorted"; a reader that
  scans them all does not need them to be.

One more boundary: `koala-font` reports what the tables contain. The
choice between `OS/2`'s typo metrics and `hhea`'s is CSS Inline 3's
rule, so it belongs in the `FontMetrics` implementation, not in
`koala-font`.

Against: a single `Option<Metrics>` with no error type is less code,
and for now the only caller just falls back to approximate metrics
either way. It would lose the reason for a failure, which is what you
want when a real font on someone's machine fails to load.

### Bounds checking

Recommendation: one small reader over the byte slice that every read
goes through, where each read returns `Option` or `Result` and nothing
indexes the slice directly. Two things make it enforced, not just a
convention:

- `#![deny(clippy::indexing_slicing)]` on the crate, so a stray
  `data[i]` fails the lint.
- `checked_add` wherever an offset and a length are combined; a table
  record's `offset + length` can overflow `u32`.

A property test that feeds arbitrary bytes and asserts no panic makes
it measurable; `quickcheck` is already a workspace dev-dependency.

Against: none worth the risk. A font file is untrusted input, as a page
is.

### `no_std`

Recommendation: `#![no_std]` with no `alloc` at all. This slice copies
a dozen integers and borrows everything else, so it needs no heap, and
`no_std` turns "does no I/O" from a rule into something the compiler
checks. Revisit at the rasterizer, which needs coverage buffers.

Against: error variants cannot carry formatted strings. With a typed
enum they do not need to.

## Decisions (2026-10-15)

The recommendations above were taken, except `no_std`: the crate uses
`std`. The API follows fontdue's shape (`Font::from_bytes` plus
`FontSettings { collection_index }`), with two departures:

- **Typed errors**: `FontError`, where fontdue returns `&'static str`.
- **Lazy, ranged reads**: fontdue copies everything out when the font
  is built. Measured in release on 2026-10-15, fontdue's parse costs
  7-9 ms per font against 0.1-0.4 ms to read the whole file, so eager
  parsing is the cost to avoid. `Font` instead reads through a
  *source* and asks it only for the byte ranges it needs. For the
  metrics that is the collection header, the table directory and four
  small tables: under a kilobyte per face, wherever the bytes live.

The source is a trait the caller implements, so `koala-font` itself
still does no I/O:

```text
FontSource::read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>
```

Its signature is `std::os::unix::fs::FileExt::read_exact_at`'s, so a
`File` source is a one-line wrapper on Unix (positioned reads, no
seeking, shareable by `&`). Bytes in memory (`Arc<[u8]>`, `Vec<u8>`)
are a source too, returning `UnexpectedEof` past the end. Reading the
whole file into memory stays the simple path; asking a file for a few
ranges is the path for CSS font matching, which will ask small
questions of every system font.

```text
FontSettings { collection_index: u32 }                  // Default: 0
Font::from_source(source, settings) -> Result<Font<S>, FontError>
face_count(&source) -> Result<u32, FontError>
font.units_per_em() -> u16
font.hhea_metrics() -> VerticalMetrics { ascender, descender, line_gap }
font.typo_metrics() -> Option<VerticalMetrics>
font.underline()    -> Option<DecorationLine { position, thickness }>
font.strikeout()    -> Option<DecorationLine>
font.scale_factor(px) -> f32
```

Values are in font units. The `FontMetrics` implementation in
`koala-browser` applies CSS's choice of typo or `hhea` metrics and the
scaling.

What `Font` keeps between calls (the table directory, the decoded
metrics, the source) is yours to decide. The v0 values are tiny, so
reading them all in `from_source` is reasonable; the glyph tables in
later slices are what must stay unread until asked for.

## Testing

`ttf-parser` 0.25.1, already in the dependency tree through `fontdb`
and `rustybuzz`, reads all of these values. (fontdue uses 0.21; the
`OS/2` handling described below is the same in both.) As a dev-dependency it is the
differential oracle, as Boa is planned to be for the JS engine: for
every face of every font file on the machine, every value `koala-font`
reports must equal `ttf-parser`'s. That runs over real data rather
than a hand-made fixture. `tests/ttf_parser_oracle.rs`:

- compares each face read from memory, and again read from the file
  through a counting source, which must give the same values while
  reading at most 4 KiB per face;
- compares `OS/2` values only where `ttf-parser` keeps the table: it
  drops `OS/2` entirely when the version is above 5 or a version 0
  table is shorter than 78 bytes, where `koala-font` reports what is
  there;
- reads `ttf-parser`'s raw table values (`Face::tables()`), not its
  `underline_metrics()`, which applies variable-font adjustments that
  v0 does not.

A quickcheck property test asserts that arbitrary bytes give an error
or a font, never a panic.

## After this slice

1. Claude: `FontMetrics` (koala-css) gains ascent, descent and line
   gap; `FontdueFontMetrics` gets them from `koala-font`.
2. Layout records each text fragment's baseline; the renderer draws
   glyphs on it instead of at its own 1.0 em.
3. Decorations: underline from `post`, line-through from `OS/2`.
4. Before/after screenshots of the corpus pages, since every text
   baseline moves by up to 0.1 em.
