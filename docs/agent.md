# Using newpub from an AI agent

newpub is fully operable by an AI agent. Everything a person can do in the app is an engine **action**, and
every question is a **query** (ARCHITECTURE.md §4); the journeys drive the program through exactly that
interface. `newpub-agent` puts that interface behind the Model Context Protocol (MCP) and a command line, and
the desktop app can host the same server so a person watches the agent work.

Three ways in:

| Way in | Start it | Use it for |
|---|---|---|
| **MCP server, headless** | `newpub-agent` (or `newpub-agent serve --file x.newspub`) | An agent that builds, edits, checks and exports publications on its own |
| **MCP server inside the app** | `newpub --agent` (optionally with a file to open) | An agent and a person working on the same open publication; the person sees every change and can take over |
| **Command line** | `newpub-agent exec / query / render / status / reference` | Scripts and shells, one call at a time |

## Connecting an MCP client

The server speaks MCP over stdio (JSON-RPC 2.0, one message per line; protocol revisions 2025-06-18,
2025-03-26 and 2024-11-05). Any MCP client can start it.

Claude Code:

```sh
claude mcp add newpub -- /path/to/newpub-agent
# or, to work in the open window:
claude mcp add newpub-live -- /path/to/newpub --agent
```

Claude Desktop and other clients that take a JSON configuration:

```json
{
  "mcpServers": {
    "newpub": { "command": "/path/to/newpub-agent", "args": [] }
  }
}
```

`newpub-agent` ships next to `newpub` in the .deb, the Linux .tar.gz, the macOS app bundle
(`newpub.app/Contents/MacOS/newpub-agent`) and the Windows setup and portable zip; from a checkout,
`cargo build --release` puts both in `target/release/`. Relative paths in tool calls resolve against the server's working directory.
`--bundled-fonts` makes the headless server use only the bundled fonts (deterministic output, as in journeys)
instead of the system's fonts.

## The tools

| Tool | What it does |
|---|---|
| `newpub_reference` | The vocabulary: every action and query with its fields. `search` by word ("link", "table", "export"), `names_only` for the bare list, or nothing for the whole reference. Also served as the resource `newpub://reference`. |
| `newpub_status` | The file, unsaved changes, page size, every page with its objects (id, kind, name, rect, text preview, chain, overflow), undo availability, and warnings (overflowing stories, missing fonts). |
| `newpub_action` | One action by name with its fields, e.g. `add_text_frame`, `insert_text`, `format_chars`, `link_frames`, `insert_picture`, `apply_para_style`, `undo`. Returns the ids it created. Every action is one undo step. |
| `newpub_actions` | Several actions in order as **one undo step**; stops at the first failure and says which. An unknown action name rejects the whole batch before anything runs. |
| `newpub_query` | One query by name, e.g. `story_text`, `frame_lines` (measured line positions), `overflow`, `page_objects`, `object`, `document`, `find`, `accessibility_check`, `missing_fonts`, `builtin_templates`. |
| `newpub_render_page` | A PNG of a page (0-based) at up to 300 dpi, returned as an image so the agent can look at its work. |
| `newpub_new` | A new publication from a built-in template (`newsletter`, `flyer`, `bulletin`, `booklet`, …) or blank with a page size. |
| `newpub_open` | Opens `.newspub` / `.npub`, imports Microsoft Publisher `.pub`, or converts a predecessor NewsPub file. |
| `newpub_save` | Saves as `.newspub` (to the opened file when no path is given). |
| `newpub_export_pdf` | PDF export: plain, PDF/X-4 or PDF/UA-1, a page subset, a saddle-stitched booklet, bleed and crop marks. Other exports (PNG, JPEG, HTML, EPUB, XPS, Pack and Go) are actions. |

Tool failures come back as results with `isError: true` and a message that names the `newpub_reference`
search to run, never as protocol errors, so the agent can read them and try again.

Conventions the agent needs: geometry is in points (72 per inch) from the page's top-left corner, y down;
any length also accepts a string with a unit (`"2in"`, `"50mm"`, `"12pt"`); pages are numbered from 0; text
positions are char indexes into a story, with `\n` between paragraphs. The MCP `instructions` field says
the same, so a client's model starts with it.

### A worked session

```text
newpub_new        {template: "newsletter"}
newpub_status     {}                                   → pages, objects, warnings
newpub_reference  {search: "text"}                     → insert_text, type_text, format_chars, …
newpub_actions    {actions: [
                    {action: "insert_text",  args: {target: 12, text: "Spring fair …"}},
                    {action: "format_chars", args: {target: 12, start: 0, end: 11, attrs: {bold: true}}}]}
newpub_query      {query: "overflow", args: {target: 12}}
newpub_action     {action: "autoflow", args: {frame: 12}}
newpub_render_page {page: 0, dpi: 96}                  → the agent looks at the page
newpub_save       {path: "spring.newspub"}
newpub_export_pdf {path: "spring.pdf", standard: "pdf_ua1"}
```

## Working in the open window

`newpub --agent` starts the app with the MCP server on its stdin/stdout. The window shows the publication
the agent is working on; every change appears at once and the status bar says what the agent did ("Agent:
insert_text"). The person can select, type, undo (the agent's batches are single undo steps) and save as
usual; the agent sees those changes in its next `newpub_status`. An agent change ends any text editing the
person was doing, and the current page and selection stay valid when the agent deletes things.

The app's own diagnostics go to stderr, so stdout stays a clean protocol stream.

## Command line

```sh
newpub-agent status --file spring.newspub
newpub-agent query  --file spring.newspub '{"q": "story_text", "target": 12}'
newpub-agent exec   --file spring.newspub --save \
    '{"cmd": "insert_text", "target": 12, "text": "Hello"}' \
    export_pdf '{"path": "spring.pdf"}'
newpub-agent render --file spring.newspub --page 0 --dpi 150 --out page-1.png
newpub-agent reference link
```

`exec` takes actions as JSON objects with a `cmd` field, or as a name followed by its fields; `--save`
writes back to `--file`, `--save=other.newspub` elsewhere. Exit status 1 on the first failure.

## Keeping the reference honest

`docs/agent-reference.md` is generated from the Rust sources by `tools/agent/reference.py` and embedded in
the binary. CI fails when it is out of date, so run the script after changing `Command`, `SessionAction` or
`Query`. The journeys J-AG-001 (headless) and UI-AG-001 (the app as host) cover the tools; the crate's
`tests/stdio.rs` drives the real binary over a pipe.
