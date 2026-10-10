# newpub-rs

A desktop publishing app in Rust for macOS, Windows, and Linux, built toward feature parity with Microsoft Publisher.

![The editor: ribbon, page thumbnails, canvas and Format panel](docs/screenshots/editor-light.png)

| Start screen | Dark mode | Export As |
|---|---|---|
| ![Start screen](docs/screenshots/start-screen.png) | ![Dark mode](docs/screenshots/editor-dark.png) | ![Export As](docs/screenshots/export-as.png) |

It opens Microsoft Publisher files (`.pub`, Publisher 98 through 2010 and later) as well as its own `.npub` publications.

**To install it, see [INSTALL.md](INSTALL.md).** There is a setup program for Windows, a .dmg or .pkg for macOS, and an AppImage or .deb for Linux.

- `docs/agent.md` explains how an AI agent operates newpub: an MCP server (`newpub-agent`, or `newpub --agent` inside the window) and a command line over every action and query.
- `PARITY.md` lists the parity checklist and the journeys that prove each item.
- `ARCHITECTURE.md` covers crates, the document model, the command layer, layout, and testing.
- `PROGRESS.md` is the build log, with decisions and open blockers.
- `journeys/` holds the end-to-end journeys, which are the project's only tests.

```sh
cargo run -p newpub-app                          # the app
cargo run -p newpub-app -- --agent               # the app, driven by an AI agent over MCP (docs/agent.md)
cargo run -p newpub-agent                        # headless MCP server; `-- reference`, `exec`, `query`, `render`
cargo run -p newpub-journeys --release           # run all journeys
cargo run -p newpub-journeys --release -- --filter J-TF   # a subset
python3 tools/dashboard/build.py                 # regenerate dashboard/index.html
```

Bundled fonts (Carlito, Liberation, DejaVu) are under open licences; see `assets/fonts/licenses`. The interface uses
Inter and Phosphor icons (OFL and MIT; see `assets/ui`).
