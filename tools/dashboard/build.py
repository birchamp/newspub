#!/usr/bin/env python3
"""Generate the newpub-rs progress dashboard as one static page.

Standard library only. Reads PARITY.md, PROGRESS.md, status/dispatch.json and
journey results.json files (plus job-status.json next to them), then writes
OUTDIR/index.html and copies every linked artifact or screenshot into
OUTDIR/runs/<os>/<relative path>.

See tools/dashboard/README.md for the CLI and inputs.
"""
import argparse
import datetime as dt
import html
import json
import os
import posixpath
import re
import shutil
import sys
from urllib.parse import quote

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OS_ORDER = ["linux", "macos", "windows"]
ITEM_RE = re.compile(r"^[A-Z]{2}-\d{2}$")
AREA_RE = re.compile(r"^##\s+([A-Z]{2}):\s*(.+?)\s*$")
BULLET_RE = re.compile(r"^[-*]\s+(.*)$")
BLOCKER_RE = re.compile(r"^\[([^\]]+)\]\s*(.*)$")
OS_SAFE_RE = re.compile(r"^[a-z0-9_.-]{1,32}$")
SHA_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
# Worst-first ordering used when aggregating one journey across OSes.
STATUS_PRIORITY = ["error", "fail", "needs-approval", "other", "pass"]
KNOWN_STATUSES = {"pass", "fail", "needs-approval", "error"}
STATUS_CLASS = {"pass": "pass", "fail": "fail", "needs-approval": "appr",
                "error": "err", "not-run": "notrun", "partial": "partial",
                "success": "pass", "failure": "fail", "skipped": "notrun"}


def warn(msg):
    print("warning: " + msg, file=sys.stderr)


def esc(value):
    return html.escape("" if value is None else str(value), quote=True)


def pct(n, d):
    return "%.1f%%" % (100.0 * n / d) if d else "0.0%"


def load_json(path, required=False):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except FileNotFoundError:
        if required:
            raise
        return None
    except (OSError, ValueError) as exc:
        warn("could not read %s: %s" % (path, exc))
        return None


def norm_os(value):
    name = str(value or "unknown").strip().lower()
    return name if OS_SAFE_RE.match(name) else "unknown"


def safe_rel(rel):
    """Return a normalised relative path, or None if it could escape its folder."""
    if not isinstance(rel, str) or not rel.strip():
        return None
    r = rel.replace("\\", "/")
    if r.startswith("/") or re.match(r"^[A-Za-z]:", r):
        return None
    n = posixpath.normpath(r)
    parts = n.split("/")
    if n in (".", "") or any(p in ("", ".", "..") for p in parts):
        return None
    return parts


# ---------------------------------------------------------------- inputs

def parse_parity(path):
    areas, order, items = {}, [], []
    area = None
    with open(path, encoding="utf-8") as f:
        for raw in f:
            s = raw.strip()
            m = AREA_RE.match(s)
            if m:
                area = m.group(1)
                if area not in areas:
                    areas[area] = m.group(2)
                    order.append(area)
                continue
            if s.startswith("## "):
                area = None
                continue
            if not s.startswith("|"):
                continue
            cells = [c.strip() for c in s.strip("|").split("|")]
            if len(cells) < 5 or not ITEM_RE.match(cells[0]):
                continue
            iid, prio, feature, journeys, done = cells[:5]
            code = iid[:2]
            if code not in areas:
                areas[code] = code
                order.append(code)
            items.append({
                "id": iid,
                "area": code,
                "prio": prio,
                "feature": feature,
                "journeys": [j.strip() for j in journeys.split(",") if j.strip()],
                "done": done.lower() == "[x]",
            })
    return {"areas": areas, "order": order, "items": items}


def parse_progress(path):
    blockers, log, escalations = [], [], []
    if not os.path.isfile(path):
        return blockers, log, escalations
    section, entry = None, None
    with open(path, encoding="utf-8") as f:
        for raw in f:
            line = raw.rstrip("\n")
            s = line.strip()
            if s.startswith("## "):
                section = s[3:].strip().lower()
                entry = None
                continue
            if section and section.startswith("open blockers"):
                m = BULLET_RE.match(s)
                if m:
                    text = m.group(1).strip()
                    bm = BLOCKER_RE.match(text)
                    if bm:
                        blockers.append({"id": bm.group(1).strip(), "text": bm.group(2).strip()})
                    else:
                        blockers.append({"id": "", "text": text})
                continue
            if section == "log":
                if s.startswith("### "):
                    entry = {"heading": s[4:].strip(), "lines": []}
                    log.append(entry)
                    continue
                if entry is None:
                    if not s:
                        continue
                    entry = {"heading": "(no heading)", "lines": []}
                    log.append(entry)
                entry["lines"].append(line)
                bm = BULLET_RE.match(s)
                if bm and "ESCALATION:" in s:
                    escalations.append({"entry": entry["heading"], "text": bm.group(1).strip()})
    for entry in log:
        while entry["lines"] and not entry["lines"][-1].strip():
            entry["lines"].pop()
    return blockers, log, escalations


def load_dispatch(path):
    data = load_json(path)
    tasks = {}
    updated = None
    if isinstance(data, dict):
        updated = data.get("updated")
        for t in data.get("tasks") or []:
            if isinstance(t, dict) and t.get("item"):
                tasks.setdefault(str(t["item"]).strip(), []).append(t)
    return updated, tasks


def find_runs(dirs):
    """Return one run per OS: the results.json with the newest mtime."""
    best = {}
    for d in dirs:
        if not os.path.isdir(d):
            warn("results directory not found: %s" % d)
            continue
        for dp, _dn, fn in os.walk(d):
            if "results.json" not in fn:
                continue
            p = os.path.join(dp, "results.json")
            data = load_json(p)
            if not isinstance(data, dict):
                warn("skipping %s (not a results object)" % p)
                continue
            osname = norm_os(data.get("os"))
            mtime = os.path.getmtime(p)
            if osname not in best or mtime > best[osname]["mtime"]:
                job = load_json(os.path.join(dp, "job-status.json"))
                best[osname] = {"os": osname, "dir": dp, "mtime": mtime,
                                "data": data, "job": job if isinstance(job, dict) else {}}
    return best


def read_git_sha(root):
    """Short commit sha from .git/HEAD and its ref, without running git."""
    try:
        gitdir = os.path.join(root, ".git")
        if os.path.isfile(gitdir):
            with open(gitdir, encoding="utf-8") as f:
                first = f.readline().strip()
            if not first.startswith("gitdir:"):
                return None
            gitdir = os.path.normpath(os.path.join(root, first[7:].strip()))
        with open(os.path.join(gitdir, "HEAD"), encoding="utf-8") as f:
            head = f.read().strip()
        sha = head
        if head.startswith("ref:"):
            ref = head[4:].strip()
            refpath = os.path.join(gitdir, *ref.split("/"))
            if os.path.isfile(refpath):
                with open(refpath, encoding="utf-8") as f:
                    sha = f.read().strip()
            else:
                sha = ""
                packed = os.path.join(gitdir, "packed-refs")
                if os.path.isfile(packed):
                    with open(packed, encoding="utf-8") as f:
                        for line in f:
                            parts = line.split()
                            if len(parts) == 2 and parts[1] == ref:
                                sha = parts[0]
                                break
        if SHA_RE.match(sha):
            return sha[:7]
    except OSError:
        return None
    return None


# ---------------------------------------------------------------- derived data

class Copier:
    """Copies artifacts into OUT/runs/<os>/ and returns the links to them."""

    def __init__(self, out_dir, inline=False):
        self.out_dir = out_dir
        # inline=True: no copying; PNG screenshots become data: URIs, other files are not linked.
        self.inline = inline
        self.runs_dir = os.path.join(out_dir, "runs")
        if os.path.isdir(self.runs_dir):
            shutil.rmtree(self.runs_dir)

    def link(self, run, rel):
        parts = safe_rel(rel)
        if parts is None:
            warn("ignoring unsafe path %r in %s" % (rel, run["dir"]))
            return None
        base = os.path.realpath(run["dir"])
        src = os.path.realpath(os.path.join(base, *parts))
        if os.path.commonpath([base, src]) != base or not os.path.isfile(src):
            warn("missing artifact %s" % os.path.join(run["dir"], *parts))
            return None
        if self.inline:
            if not src.lower().endswith(".png"):
                return None
            import base64
            with open(src, "rb") as fh:
                return "data:image/png;base64," + base64.b64encode(fh.read()).decode("ascii")
        dst = os.path.join(self.runs_dir, run["os"], *parts)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(src, dst)
        return "runs/%s/%s" % (quote(run["os"]), "/".join(quote(p) for p in parts))


def build_journey_results(runs, copier):
    """results[journey_id][os] = record with status, message, links, page1."""
    results, titles = {}, {}
    for osname in sorted(runs, key=lambda o: OS_ORDER.index(o) if o in OS_ORDER else 99):
        run = runs[osname]
        for j in run["data"].get("journeys") or []:
            if not isinstance(j, dict) or not isinstance(j.get("id"), str):
                continue
            jid = j["id"]
            titles.setdefault(jid, j.get("title") or "")
            links, page1 = [], None
            for rel in (j.get("artifacts") or []) + (j.get("screenshots") or []):
                href = copier.link(run, rel)
                if href:
                    links.append((posixpath.basename(rel.replace("\\", "/")), href))
            for rel in j.get("screenshots") or []:
                if isinstance(rel, str) and posixpath.basename(rel.replace("\\", "/")) == "page-1.png":
                    page1 = copier.link(run, rel)
                    break
            message = j.get("message") or ""
            if j.get("failed_step"):
                message = "failed at %s: %s" % (j.get("failed_step"), message)
            results.setdefault(jid, {})[osname] = {
                "status": str(j.get("status") or "unknown").lower(),
                "message": str(message),
                "links": links,
                "page1": page1,
                "run": run,
            }
    return results, titles


def journey_agg(jid, present, results):
    """One status for a journey across the OSes present: worst wins, 'partial' if an OS is missing."""
    recs = results.get(jid, {})
    statuses = [recs[o]["status"] if recs[o]["status"] in KNOWN_STATUSES else "other"
                for o in present if o in recs]
    if not statuses:
        return "not-run"
    for s in STATUS_PRIORITY:
        if s in statuses:
            if s == "pass" and len(statuses) < len(present):
                return "partial"
            return s
    return "not-run"


def status_chip_class(status):
    return STATUS_CLASS.get(status, "other")


def compute_items(parity, tasks, blockers, results, present):
    blocker_texts = [b["text"] + " " + b["id"] for b in blockers]
    out = []
    for item in parity["items"]:
        iid = item["id"]
        its = tasks.get(iid, [])
        pat = re.compile(r"(?<![A-Za-z0-9-])" + re.escape(iid) + r"(?![A-Za-z0-9-])")
        blocked_by_text = any(pat.search(t) for t in blocker_texts)
        ip = [t for t in its if t.get("state") == "in_progress"]
        if item["done"]:
            status = "done"
        elif blocked_by_text or any(t.get("state") == "blocked" for t in its):
            status = "blocked"
        elif ip:
            status = "in_progress"
        else:
            status = "not_started"
        jstates = {j: journey_agg(j, present, results) for j in item["journeys"]}
        green = bool(item["journeys"]) and bool(present) and all(
            v == "pass" for v in jstates.values())
        out.append({
            **item,
            "status": status,
            "task": ip[0] if ip else None,
            "blocked_text": blocked_by_text,
            "jstates": jstates,
            "green": green,
        })
    return out


# ---------------------------------------------------------------- rendering

CSS = """
:root{--bg:#f7f7f4;--fg:#1c1c1a;--muted:#66665f;--card:#ffffff;--line:#dcdcd4;
--done:#2e7d4f;--prog:#2f6fb0;--block:#c0392b;--todo:#c9c9c0;
--pin-bg:#fff3d1;--pin-fg:#5c3b00;--pin-line:#d98a00;--pass:#2e7d4f;--fail:#c0392b;
--appr:#9a6200;--err:#8e2c8e;--notrun:#7a7a72;--partial:#9a6200;--other:#555;--chip-bg:#efefe9;}
@media (prefers-color-scheme: dark){
:root:not([data-theme="light"]){color-scheme:dark;--bg:#141413;--fg:#ecebe6;--muted:#a3a39a;--card:#1d1d1b;--line:#3a3a36;
--done:#4cae71;--prog:#6aa6e0;--block:#ff6b5b;--todo:#55554f;
--pin-bg:#3b2d10;--pin-fg:#ffe2a6;--pin-line:#f0b429;--pass:#5fbf84;--fail:#ff6b5b;
--appr:#f0b429;--err:#e08be0;--notrun:#8d8d85;--partial:#f0b429;--other:#c8c8c0;--chip-bg:#262624;}}
:root[data-theme="dark"]{color-scheme:dark;--bg:#141413;--fg:#ecebe6;--muted:#a3a39a;--card:#1d1d1b;--line:#3a3a36;
--done:#4cae71;--prog:#6aa6e0;--block:#ff6b5b;--todo:#55554f;
--pin-bg:#3b2d10;--pin-fg:#ffe2a6;--pin-line:#f0b429;--pass:#5fbf84;--fail:#ff6b5b;
--appr:#f0b429;--err:#e08be0;--notrun:#8d8d85;--partial:#f0b429;--other:#c8c8c0;--chip-bg:#262624;}
*{box-sizing:border-box}
body{margin:0;padding-block:16px;padding-inline:16px;background:var(--bg);color:var(--fg);
font:15px/1.45 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;}
.wrap{max-width:1180px;margin:0 auto}
h1{font-size:1.5rem;margin:0 0 4px}
h2{font-size:1.15rem;margin:0 0 10px;border-bottom:1px solid var(--line);padding-bottom:4px}
h3{font-size:1rem;margin:0 0 6px}
section{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:14px;margin:0 0 16px}
.muted{color:var(--muted);font-size:.9rem}
.pinned{background:var(--pin-bg);color:var(--pin-fg);border:2px solid var(--pin-line)}
.pinned h2{border-color:var(--pin-line)}
.pinned ul{margin:0;padding-left:20px}
.stats{display:flex;flex-wrap:wrap;gap:12px;margin-bottom:12px}
.stat{flex:1 1 150px;border:1px solid var(--line);border-radius:8px;padding:10px}
.stat b{display:block;font-size:1.5rem}
.area-row{display:grid;grid-template-columns:minmax(0,1fr);gap:4px;margin:10px 0}
.area-label{font-size:.9rem}
.bar{display:flex;height:14px;border-radius:7px;overflow:hidden;background:var(--todo)}
.seg{display:block;height:100%}
.seg.done{background:var(--done)}.seg.in_progress{background:var(--prog)}
.seg.blocked{background:var(--block)}.seg.not_started{background:var(--todo)}
.legend{display:flex;flex-wrap:wrap;gap:12px;font-size:.85rem;color:var(--muted);margin-top:6px}
.key{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:4px;vertical-align:middle}
.scroll{overflow-x:auto;-webkit-overflow-scrolling:touch;max-width:100%}
table{border-collapse:collapse;width:100%;min-width:560px}
th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);vertical-align:top}
th{font-size:.85rem;color:var(--muted);font-weight:600}
tr.area th{background:var(--chip-bg);color:var(--fg);font-size:.95rem}
.chip{display:inline-block;font-size:.8rem;border:1px solid var(--line);border-radius:999px;
padding:1px 8px;margin:1px 3px 1px 0;background:var(--chip-bg);white-space:nowrap}
.chip.pass{color:var(--pass);border-color:var(--pass)}.chip.fail{color:var(--fail);border-color:var(--fail)}
.chip.appr{color:var(--appr);border-color:var(--appr)}.chip.err{color:var(--err);border-color:var(--err)}
.chip.notrun{color:var(--notrun)}.chip.partial{color:var(--partial);border-color:var(--partial)}
.chip.other{color:var(--other)}
.st{font-weight:600;white-space:nowrap}
.st.pass{color:var(--pass)}.st.fail{color:var(--fail)}.st.appr{color:var(--appr)}
.st.err{color:var(--err)}.st.notrun{color:var(--notrun);font-weight:400}.st.other{color:var(--other)}
.badge{font-size:.78rem;border-radius:6px;padding:1px 6px;white-space:nowrap}
.badge.green{background:var(--pass);color:#fff}.badge.grey{border:1px solid var(--line);color:var(--muted)}
.status-done{color:var(--done);font-weight:600}.status-in_progress{color:var(--prog);font-weight:600}
.status-blocked{color:var(--block);font-weight:600}.status-not_started{color:var(--muted)}
.links a{display:inline-block;font-size:.8rem;margin:1px 6px 1px 0;word-break:break-all}
.gallery{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:10px}
.gallery figure{margin:0;border:1px solid var(--line);border-radius:8px;padding:6px;background:var(--chip-bg)}
.gallery img{width:100%;height:auto;display:block;border-radius:4px;background:#fff}
.gallery figcaption{font-size:.8rem;color:var(--muted);margin-top:4px;word-break:break-all}
.logbody{white-space:pre-wrap;font:13px/1.45 ui-monospace,Menlo,Consolas,monospace;
background:var(--chip-bg);border-radius:6px;padding:8px;overflow-x:auto;margin:0}
.log article{border-bottom:1px solid var(--line);padding:0 0 12px;margin:0 0 12px}
.ci td.ok{color:var(--pass);font-weight:600}.ci td.bad{color:var(--fail);font-weight:600}
a{color:var(--prog)}
"""


def render_pinned(blockers):
    if not blockers:
        body = '<p>No open blockers</p>'
    else:
        lis = "".join("<li>%s%s</li>" % (
            ("<b>[%s]</b> " % esc(b["id"])) if b["id"] else "", esc(b["text"])) for b in blockers)
        body = "<ul>%s</ul>" % lis
    return ('<section class="pinned" id="needs-input"><h2>Needs your input</h2>%s</section>' % body)


def render_summary(items, present, generated):
    total = len(items)
    done = sum(1 for i in items if i["status"] == "done")
    p02 = [i for i in items if i["prio"] in ("P0", "P1", "P2")]
    done02 = sum(1 for i in p02 if i["status"] == "done")
    counts = {k: sum(1 for i in items if i["status"] == k)
              for k in ("done", "in_progress", "blocked", "not_started")}
    green = sum(1 for i in items if i["green"])
    stats = [
        ("Overall parity", "%s" % pct(done, total), "%d of %d items done" % (done, total)),
        ("P0 to P2 parity", pct(done02, len(p02)), "%d of %d items done" % (done02, len(p02))),
        ("In progress", str(counts["in_progress"]), "items with an agent working"),
        ("Blocked", str(counts["blocked"]), "items waiting on a blocker or task"),
        ("Not started", str(counts["not_started"]), "items with no task yet"),
        ("Journeys green", "%d" % green, "items whose journeys pass on every OS run"),
    ]
    cells = "".join('<div class="stat"><span class="muted">%s</span><b>%s</b><span class="muted">%s</span></div>'
                    % (esc(a), esc(b), esc(c)) for a, b, c in stats)
    oses = ", ".join(present) if present else "none"
    return ('<section id="summary"><h2>Overall parity</h2><div class="stats">%s</div>'
            '<p class="muted">Results present for: %s. Counts from PARITY.md, PROGRESS.md and '
            'dispatch.json as of %s.</p></section>' % (cells, esc(oses), esc(generated)))


def render_area_bars(parity, items):
    rows = []
    for code in parity["order"]:
        group = [i for i in items if i["area"] == code]
        if not group:
            continue
        total = len(group)
        done = sum(1 for i in group if i["status"] == "done")
        seg = []
        for key in ("done", "in_progress", "blocked", "not_started"):
            n = sum(1 for i in group if i["status"] == key)
            if n:
                seg.append('<span class="seg %s" style="width:%.2f%%" title="%s: %d"></span>'
                           % (key, 100.0 * n / total, esc(key.replace("_", " ")), n))
        rows.append('<div class="area-row"><div class="area-label"><b>%s</b> %s &middot; %d/%d (%s)</div>'
                    '<div class="bar">%s</div></div>'
                    % (esc(code), esc(parity["areas"].get(code, code)), done, total,
                       pct(done, total), "".join(seg)))
    legend = ('<div class="legend"><span><span class="key" style="background:var(--done)"></span>done</span>'
              '<span><span class="key" style="background:var(--prog)"></span>in progress</span>'
              '<span><span class="key" style="background:var(--block)"></span>blocked</span>'
              '<span><span class="key" style="background:var(--todo)"></span>not started</span></div>')
    return '<section id="areas"><h2>Feature areas</h2>%s%s</section>' % ("".join(rows), legend)


def render_ci(runs, oses):
    head = "<tr><th>OS</th><th>fmt</th><th>clippy</th><th>build</th><th>journeys</th><th>sha</th><th>run</th></tr>"
    rows = []
    for osname in oses:
        run = runs.get(osname)
        job = run["job"] if run else {}

        def cell(key):
            v = job.get(key) if job else None
            v = "unknown" if not v else str(v)
            cls = status_chip_class(v)
            css = "ok" if cls == "pass" else ("bad" if cls == "fail" else "")
            return '<td class="%s">%s</td>' % (css, esc(v))
        if run:
            data = run["data"]
            total = data.get("total")
            passed = data.get("passed")
            if not isinstance(total, int) or not isinstance(passed, int):
                jl = run["data"].get("journeys") or []
                total = len(jl)
                passed = sum(1 for j in jl if isinstance(j, dict) and j.get("status") == "pass")
            jcell = '<td class="%s">%d/%d</td>' % ("ok" if total and passed == total else "bad", passed, total)
        elif job.get("journeys"):
            jcell = "<td>%s</td>" % esc(job.get("journeys"))
        else:
            jcell = "<td>unknown</td>"
        sha = str(job.get("sha") or "")[:7] or "unknown"
        url = str(job.get("run_url") or "")
        link = ('<a href="%s" rel="noopener">run</a>' % esc(url)
                if url.startswith("https://") or url.startswith("http://") else "unknown")
        rows.append("<tr><td><b>%s</b></td>%s%s%s%s<td>%s</td><td>%s</td></tr>"
                    % (esc(osname), cell("fmt"), cell("clippy"), cell("build"), jcell,
                       esc(sha), link))
    return ('<section id="ci"><h2>CI status</h2><div class="scroll"><table class="ci">%s%s</table></div>'
            '<p class="muted">fmt, clippy, build come from job-status.json; journeys from results.json.</p>'
            '</section>' % (head, "".join(rows)))


def render_checklist(parity, items, present, results):
    head = ('<tr><th>ID</th><th>Pri</th><th>Feature</th><th>Status</th><th>Journeys</th>'
            '<th>Green</th></tr>')
    rows, current = [], None
    for item in items:
        if item["area"] != current:
            current = item["area"]
            rows.append('<tr class="area"><th colspan="6">%s &middot; %s</th></tr>'
                        % (esc(current), esc(parity["areas"].get(current, current))))
        if item["status"] == "in_progress" and item["task"]:
            t = item["task"]
            status_html = ('<span class="status-in_progress">in progress</span><br>'
                           '<span class="muted">%s &middot; %s</span>'
                           % (esc(t.get("agent") or "unknown agent"), esc(t.get("tier") or "unknown tier")))
        else:
            label = {"done": "done", "blocked": "blocked", "not_started": "not started"}[item["status"]]
            status_html = '<span class="status-%s">%s</span>' % (item["status"], label)
        chips = []
        for j in item["journeys"]:
            detail = ", ".join("%s %s" % (o, results.get(j, {}).get(o, {}).get("status", "not-run"))
                               for o in present) or "no OS results"
            chips.append('<span class="chip %s" title="%s">%s</span>'
                         % (status_chip_class(item["jstates"][j]), esc(detail), esc(j)))
        badge = ('<span class="badge green">journeys green</span>' if item["green"]
                 else '<span class="badge grey">not green</span>')
        rows.append("<tr><td><b>%s</b></td><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>"
                    % (esc(item["id"]), esc(item["prio"]), esc(item["feature"]), status_html,
                       "".join(chips), badge))
    return ('<section id="checklist"><h2>Item checklist</h2><div class="scroll"><table>%s%s</table>'
            '</div></section>' % (head, "".join(rows)))


def render_matrix(parity, results, titles, oses, items):
    seen = []
    for item in parity["items"]:
        for j in item["journeys"]:
            if j not in seen:
                seen.append(j)
    for j in sorted(results):
        if j not in seen:
            seen.append(j)
    covers = {}
    for item in parity["items"]:
        for j in item["journeys"]:
            covers.setdefault(j, []).append(item["id"])
    head = "<tr><th>Journey</th><th>Items</th>%s</tr>" % "".join("<th>%s</th>" % esc(o) for o in oses)
    rows = []
    for jid in seen:
        cells = []
        for o in oses:
            rec = results.get(jid, {}).get(o)
            if rec:
                title = rec["message"] or rec["status"]
                st = rec["status"]
                links = "".join('<a href="%s">%s</a>' % (esc(h), esc(lbl)) for lbl, h in rec["links"])
                cells.append('<td class="st %s" title="%s">%s%s</td>'
                             % (status_chip_class(st), esc(title), esc(st),
                                ('<div class="links">%s</div>' % links) if links else ""))
            else:
                cells.append('<td class="st notrun">not-run</td>')
        rows.append("<tr><td><b>%s</b><br><span class=\"muted\">%s</span></td><td>%s</td>%s</tr>"
                    % (esc(jid), esc(titles.get(jid, "")), esc(", ".join(covers.get(jid, [])) or "none"),
                       "".join(cells)))
    return ('<section id="matrix"><h2>Journey matrix</h2><div class="scroll"><table>%s%s</table></div>'
            '<p class="muted">Hover a status for its failure message. Links open the artifacts and screenshots '
            'copied into runs/.</p></section>' % (head, "".join(rows)))


def render_gallery(run_order, results):
    figs = []
    for osname in run_order:
        for jid in sorted(results):
            rec = results[jid].get(osname)
            if not rec or not rec.get("page1"):
                continue
            href = rec["page1"]
            figs.append('<figure><a href="%s"><img src="%s" alt="%s page 1" loading="lazy"></a>'
                        '<figcaption>%s &middot; %s</figcaption></figure>'
                        % (esc(href), esc(href), esc(jid), esc(jid), esc(osname)))
            if len(figs) >= 24:
                break
        if len(figs) >= 24:
            break
    body = '<div class="gallery">%s</div>' % "".join(figs) if figs else '<p class="muted">No screenshots yet.</p>'
    return '<section id="gallery"><h2>Screenshot gallery</h2>%s</section>' % body


def render_log(log):
    entries = list(reversed(log[-20:]))
    if not entries:
        body = '<p class="muted">No log entries.</p>'
    else:
        parts = []
        for e in entries:
            parts.append('<article><h3>%s</h3><pre class="logbody">%s</pre></article>'
                         % (esc(e["heading"]), esc("\n".join(e["lines"]).strip("\n"))))
        body = "".join(parts)
    return ('<section id="log"><h2>Recent log (last %d, newest first)</h2><div class="log">%s</div></section>'
            % (len(entries), body))


def render_escalations(escalations):
    if not escalations:
        body = '<p>No escalations</p>'
    else:
        rows = "".join("<tr><td>%s</td><td>%s</td></tr>" % (esc(e["entry"]), esc(e["text"]))
                       for e in escalations)
        body = ('<div class="scroll"><table><tr><th>Log entry</th><th>Escalation</th></tr>%s</table></div>'
                % rows)
    return '<section id="escalations"><h2>Escalation log</h2>%s</section>' % body


def build_page(ctx):
    sha = ctx["sha"]
    commit = ("commit <b>%s</b> &middot; " % esc(sha)) if sha else ""
    head = ('<header><h1>newpub-rs &mdash; Publisher parity</h1>'
            '<p class="muted">%sGenerated %s</p></header>' % (commit, esc(ctx["generated"])))
    parts = [
        head,
        render_pinned(ctx["blockers"]),
        render_summary(ctx["items"], ctx["present"], ctx["generated"]),
        render_area_bars(ctx["parity"], ctx["items"]),
        render_ci(ctx["runs"], ctx["oses"]),
        render_checklist(ctx["parity"], ctx["items"], ctx["present"], ctx["results"]),
        render_matrix(ctx["parity"], ctx["results"], ctx["titles"], ctx["oses"], ctx["items"]),
        render_gallery(ctx["run_order"], ctx["results"]),
        render_log(ctx["log"]),
        render_escalations(ctx["escalations"]),
    ]
    if ctx.get("artifact"):
        return ("<title>newpub-rs parity</title>\n<style>%s</style>\n<div class=\"wrap\">\n%s\n"
                "<p class=\"muted\">Generated by tools/dashboard/build.py from PARITY.md, PROGRESS.md, "
                "status/dispatch.json and journey results.</p>\n</div>\n" % (CSS, "\n".join(parts)))
    return ("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n"
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n"
            "<meta name=\"color-scheme\" content=\"light dark\">\n"
            "<title>newpub-rs parity</title>\n<style>%s</style>\n</head>\n<body>\n<div class=\"wrap\">\n%s\n"
            "<p class=\"muted\">Static page generated by tools/dashboard/build.py. No external resources.</p>\n"
            "</div>\n</body>\n</html>\n" % (CSS, "\n".join(parts)))


# ---------------------------------------------------------------- main

def main(argv=None):
    ap = argparse.ArgumentParser(description="Generate the newpub-rs progress dashboard (index.html).")
    ap.add_argument("--repo", default=DEFAULT_ROOT, help="repo root (default: two levels above this script)")
    ap.add_argument("--results", action="extend", nargs="+", metavar="DIR",
                    help="directories scanned recursively for results.json (default: ROOT/target/journeys)")
    ap.add_argument("--out", default=None, help="output directory (default: ROOT/dashboard)")
    ap.add_argument("--artifact", default=None, metavar="FILE",
                    help="also write a single self-contained, body-only page (screenshots inlined) to FILE")
    args = ap.parse_args(argv)

    root = os.path.abspath(args.repo)
    out = os.path.abspath(args.out or os.path.join(root, "dashboard"))
    result_dirs = args.results or [os.path.join(root, "target", "journeys")]

    parity_path = os.path.join(root, "PARITY.md")
    if not os.path.isfile(parity_path):
        ap.error("PARITY.md not found under %s" % root)
    parity = parse_parity(parity_path)
    blockers, log, escalations = parse_progress(os.path.join(root, "PROGRESS.md"))
    _updated, tasks = load_dispatch(os.path.join(root, "status", "dispatch.json"))

    runs = find_runs(result_dirs)
    present = sorted(runs, key=lambda o: OS_ORDER.index(o) if o in OS_ORDER else 99)
    oses = OS_ORDER + [o for o in present if o not in OS_ORDER]
    run_order = sorted(runs, key=lambda o: -runs[o]["mtime"])

    os.makedirs(out, exist_ok=True)
    copier = Copier(out)
    results, titles = build_journey_results(runs, copier)

    items = compute_items(parity, tasks, blockers, results, present)
    generated = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    ctx = {
        "parity": parity, "items": items, "blockers": blockers, "log": log,
        "escalations": escalations, "runs": runs, "present": present, "oses": oses,
        "run_order": run_order, "results": results, "titles": titles,
        "sha": read_git_sha(root), "generated": generated,
    }
    page = build_page(ctx)
    index = os.path.join(out, "index.html")
    with open(index, "w", encoding="utf-8") as f:
        f.write(page)
    if args.artifact:
        results_i, _ = build_journey_results(runs, Copier(out, inline=True))
        actx = dict(ctx, results=results_i, artifact=True)
        with open(args.artifact, "w", encoding="utf-8") as f:
            f.write(build_page(actx))
        print("wrote %s" % args.artifact)

    done = sum(1 for i in items if i["status"] == "done")
    p02 = [i for i in items if i["prio"] in ("P0", "P1", "P2")]
    done02 = sum(1 for i in p02 if i["status"] == "done")
    print("wrote %s" % index)
    print("items: %d, done: %d (%s); P0-P2 done: %d/%d (%s)"
          % (len(items), done, pct(done, len(items)), done02, len(p02), pct(done02, len(p02))))
    print("os with results: %s" % (", ".join(present) or "none"))
    print("blockers: %d, log entries: %d, escalations: %d" % (len(blockers), len(log), len(escalations)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
