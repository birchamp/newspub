# Publisher (.pub) format notes

Findings from one sample file (`journeys/fixtures/tika-sample.pub`, a Publisher 2000-era file) and public format
descriptions, written in our own words. No third-party parser code was copied. Everything below is observed on that
single file, so treat the constants as hypotheses until more samples are tested.

## Container

An OLE compound file (cfb crate). Streams in the sample:

| Stream | Role |
|--------|------|
| `Contents` | Main object database. Starts with `E8 AC` then a u16 format number (`0x2C` in the sample). Not decoded beyond the header. |
| `Quill/QuillSub/CONTENTS` | Text and text formatting ("Quill" text engine). |
| `Escher/EscherStm` | Office Drawing records: shapes and their positions. |
| `Escher/EscherDelayStm` | Delayed blobs (pictures); empty in the sample. |
| `\x01CompObj`, `\x05SummaryInformation`, `\x05DocumentSummaryInformation`, `Envelope`, `\x03Internal` | Standard OLE metadata. |

Detection rule used: `Contents` plus either `Quill/QuillSub/CONTENTS` or `Escher/EscherStm`.
Version names: `0x2A` -> "Publisher 98", `0x2C` -> "Publisher 2000" (the `0x2A` case is from public notes and
unverified; other values are reported as "unknown version").

## Quill CONTENTS

- Starts with the ASCII tag `CHNKINK ` and a 24-byte header; the chunk directory begins at byte 32.
- Directory entries are 24 bytes: `u16 0x18`, 4-byte tag, 2 zero bytes, `u32 1`, 4-byte tag repeated, `u32 offset`,
  `u32 length`. Offsets are from the start of the stream. Tags seen: TEXT, STSH, FDPP, FDPC, SYID, SGP, INK, BTEP, BTEC,
  FONT, STRS, MCLD, PL.
- TEXT: UTF-16LE. `\r` (U+000D) ends a paragraph; the last paragraph mark is a terminator, not an extra empty paragraph.
  The sample has one text run covering all stories, so story boundaries live in another chunk (probably STRS/PL/MCLD,
  not decoded).
- FONT: `u32 chunk size`, `u32 count`, 12 bytes not understood, `count` x `u32` offsets (relative to the start of the
  offset table, which is at byte 20 of the chunk), then entries `u16 length, UTF-16LE name, u32 id`. The first font is
  the default (Times New Roman in the sample).
- STSH/FDPP/FDPC (style sheet and formatting runs) are not decoded, so only the default font is applied.

## Escher (Office Drawing records)

Standard record header `u16 ver/instance, u16 type, u32 length`, container records have version 15. Top-level
containers are separated by a 4-byte field that is not part of any record, so the walker skips 4 bytes whenever a
header is not valid.

Shapes are `0xF004` containers. A text box has shape type 202 (the instance of the `0xF00A` record). Its position is in
a `0xF010` record of 28 bytes: a u32 `0x1C` followed by four `(u16 tag, i32)` pairs with tags `0x2001..0x2004` =
left, top, right, bottom in EMU (914400 per inch, 12700 per point), measured from the **page centre**. In the sample
this gives a box at x=65..536, y=37..94 on a US Letter page, which fits a 0.5 in margin layout, so the centre-origin
reading is likely right.

`0xF011` (client data) and `0xF00D` (client textbox) carry the link between a shape and its text; their meaning is not
decoded. The sample has two text boxes and one text run, so the importer puts the whole story in the bounding box of
all text boxes and says so in a warning.

## Not decoded

- Page size (assumed US Letter, else A4, whichever holds all boxes).
- Text-to-box mapping, story boundaries, multiple pages.
- Character and paragraph formatting runs, colours, pictures (BLIPs in EscherDelayStm), shape fills and borders.
