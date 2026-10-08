# Progress dashboard

`build.py` generates one static page, `dashboard/index.html`, that shows parity progress for newpub-rs:
overall and per-area status, CI per OS, the item checklist, a journey matrix, a screenshot gallery, the recent
PROGRESS.md log and the escalation log. It uses only the Python 3 standard library and writes no external links.

## Usage

    python3 tools/dashboard/build.py [--repo ROOT] [--results DIR ...] [--out OUTDIR]

- `--repo` repo root (default: the directory two levels above the script).
- `--results` one or more directories scanned recursively for `results.json` (default: `ROOT/target/journeys`).
  When several runs report the same OS, the newest `results.json` wins.
- `--out` output directory (default: `ROOT/dashboard`). Writes `OUTDIR/index.html` and copies each linked
  artifact and screenshot to `OUTDIR/runs/<os>/<relative path>`, so the page works when hosted alone.

## Inputs

- `PARITY.md`: area tables (`| ID | P | Feature | Journeys | Done |`) under `## XX: Name` headings. Required.
- `PROGRESS.md`: `## Open blockers` bullets (`- [B-001] text`) and `## Log` entries (`### heading`). Bullets
  containing `ESCALATION:` in the log feed the escalation table.
- `status/dispatch.json`: `{"tasks": [{"item", "agent", "tier", "state", ...}]}`. Optional.
- `results.json` per OS run, with optional `job-status.json` beside it (fmt, clippy, build, journeys, run_url, sha).

Item status: done if `[x]`; otherwise blocked if a dispatch task is blocked or the ID appears in a blocker;
otherwise in progress if a dispatch task is in progress; otherwise not started. "Journeys green" means every
journey listed for the item passed on every OS present in the results.
