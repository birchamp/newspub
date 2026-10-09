# Publisher (.pub) format notes

Written in our own words from public descriptions of the format and from inspecting sample files. The main public
reference is LibreOffice's **libmspub** (MPL-2.0), read to learn the format; none of its code or comments were copied
or translated. The samples are Apache POI's Publisher test files (`journeys/fixtures/pub/`), and positions were
checked against LibreOffice's rendering of the same files.

Units: geometry is in EMU (12,700 per point), measured from the **page centre**, in every version.

## Container and versions

An OLE compound file. `Contents` starts with `E8 AC` and a u16 format number: `0x2C` for Publisher 2002 and later,
`0x22` for Publisher 98 and 2000 (and 97). The application version in `DocumentSummaryInformation` (PIDDSI_VERSION)
names the 2002+ releases. A file saved down to the 98/2000 format by a newer Publisher still carries the newer version
there, so 98 and 2000 are told apart from the shape records instead (below).

| Stream | Role |
|--------|------|
| `Contents` | Document, pages, shapes, tables, palette, fonts |
| `Quill/QuillSub/CONTENTS` | All text and its formatting (every version from 98 on) |
| `Escher/EscherStm`, `Escher/EscherDelayStm` | 2002+: Office Drawing shape records and the picture store |

## Quill CONTENTS (text)

`CHNKINK ` header, then a chain of chunk lists (at 0x18; each list has a count, a link to the next list, and 24-byte
entries: tag, id, offset, length). Chunks used:

- `TEXT`: all stories, UTF-16LE. `\r` ends a paragraph, `\v` is a line break; a story's last mark is a terminator.
- `STRS`: story lengths; `SYID`: story ids (the "text id" shapes refer to).
- `FDPC` / `FDPP`: character and paragraph runs: a count, end offsets into the stream, and offsets of records made of
  tagged blocks (bold 0x02, italic 0x03, size 0x0C in EMU, underline 0x1E, colour 0x2E or container 0x44, font
  container 0x24, script 0x0F, scale 0x20, language 0x12; paragraph align 0x04, style index 0x19, line spacing
  0x34, space before/after 0x12/0x13, indents 0x0C-0x0E, tabs 0x32).
- `STSH`: the second one holds the default styles, alternating character and paragraph styles; a paragraph's style
  index selects the pair its runs are relative to.
- `FONT`: the font table; `PL  `: text colour references (2002+); `TCD `: cell ends of a table story.

Text colours: 2002+ records index the `PL  ` table. 98/2000 records hold a reference directly: type byte `0xC0`/`0xE0`
indexes the publication palette, `0x00`/`0x80` a fixed table of 56 standard colours, `0x20`/`0x90` RGB.

## Publisher 2002 and later

- `Contents`: a trailer (offset at 0x1A) lists numbered chunks; each is a list of tagged blocks (see `blocks.rs`).
  Chunk types used: document 0x44 (page size), page 0x43 (shape list, master link), shape 0x01/0x20, group 0x30,
  table 0x10 (rows 0x66, columns 0x67, sizes 0x6D, cell chunk 0x6B) with cells 0x63, palette 0x5C, fonts 0x6C.
  Shapes give their text id (0x27), position in a linked chain (0x28) and vertical alignment (0x35).
- `EscherStm`: standard drawing records. Shapes (0xF004) carry a type, flags (flips), a property table (fill, line,
  insets, columns, rotation in 16.16 degrees, crop, picture index) and an anchor (0xF010, page-centre EMU) or a child
  anchor inside a group. The picture store points into `EscherDelayStm`.

## Publisher 98 and 2000

`Contents` holds a trailer (offset at 0x16): a u16 count, then 10-byte entries `u16 ?, u16 id, u16 parent id,
u32 offset`. A chunk's first u16 is its type: document 0x15 (page width and height as u32 at +0x14/+0x18), page 0x14,
palette 0x47 (eight colour references at +0xA0), group 0x0F, picture 0x02 with its data in a child 0x21 chunk
(`u32` length at +4, then a WMF), line 0x04, rectangle 0x05, autoshape 0x06 (kind at +0x31), ellipse 0x07, text box
0x08, table 0x0A. Other chunk types under a page are properties, not shapes.

Shape records: rotation at +4 (tenths of a degree, counter-clockwise), rectangle at +6 (four i32: xs, ys, xe, ye),
flips at +0x33 (autoshapes) or +0x41 (lines). Text boxes: text id at +0x58, inner margins at +0x46 (four u16 twips:
left, top, right, bottom). Tables: text id at +0x66, a count at +0x74 and `(end, size)` u32 pairs from +0x7E: the
column boundaries first (increasing), then the row boundaries (starting again from the top). Group members keep page
coordinates. Page ids 0x108, 0x109 (master), 0x10B, 0x10D, 0x116 and 0x119 are internal pages; a page whose records
are all zero-sized holds Publisher's default shapes, not content.

Fill and the first line sit at different offsets in the two versions: Publisher 2000 has the fill colour at +0x22,
fill type at +0x2A (2 = solid) and the first line (u8 width code, u32 colour) at +0x2C; Publisher 98 has them two
bytes earlier. Both put the further sides at +0x35, +0x3B and +0x41. The importer decides per file by looking at the
bytes before the `FE FF` marker at +0x31 in its shape records. Line width codes are quarter points, with a compressed
range above 0x81.

## Fonts

Publisher files name Windows fonts. When one is not installed, layout uses a bundled stand-in: Liberation Serif for
Times New Roman and Liberation Sans for Arial (metric-compatible, so lines break in the same places), Carlito for
Calibri, otherwise a face of the same kind (serif or sans serif). The Design Checker still lists the missing fonts.

## Not imported

Reported in the import's warnings when present: WordArt, embedded fonts, gradient and pattern fills, Publisher 97
files, shapes that belong to no page, and autoshapes with no newpub equivalent.
