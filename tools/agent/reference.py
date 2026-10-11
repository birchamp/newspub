#!/usr/bin/env python3
"""Writes docs/agent-reference.md: every engine action (document commands and session actions) and every
query, with their fields, read from the Rust sources, plus a Types appendix with the JSON forms of the
enums and structs those fields use. Standard library only.

    python3 tools/agent/reference.py          # rewrite docs/agent-reference.md
    python3 tools/agent/reference.py --check  # exit 1 when the file is out of date (CI)

The agent crate embeds the generated file (newpub_agent::REFERENCE), so run this after changing
`Command`, `SessionAction` or `Query`.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "docs", "agent-reference.md")
SOURCES = [
    ("Document commands", os.path.join(ROOT, "crates", "core", "src", "command.rs"), "Command",
     "Change the publication. Every command is one undo step (typing coalesces). Names are the `cmd` tag."),
    ("Session actions", os.path.join(ROOT, "crates", "engine", "src", "action.rs"), "SessionAction",
     "Session-level actions: files, undo, export, autoflow, merge and the like. Names are the `cmd` tag."),
    ("Queries", os.path.join(ROOT, "crates", "engine", "src", "action.rs"), "Query",
     "Read-only questions, answered as JSON. Names are the `q` tag."),
]
# Crates searched for the `pub struct` and `pub enum` definitions that action and query fields refer to.
CRATES = ("core", "engine", "io-pdf", "layout")

# A variant line: `Name,` / `Name(Payload),` / `Name {` (fields follow) / `Name {},` / `Name { a: T, b: U },`.
VARIANT_RE = re.compile(
    r"^    ([A-Z]\w*)\s*(?:(\{)\s*(.*?)\s*(\})?\s*,?|\(([^)]*)\)\s*,|,)\s*(//.*)?$")
# Independent count of the variants in an enum body, used by `self_check` to catch VARIANT_RE blind spots.
COUNT_RE = re.compile(r"^    [A-Z]\w*\s*[{(,]")
FIELD_RE = re.compile(r"^ {4,8}(?:pub )?(\w+):\s*(.+?),?\s*$")
STRUCT_RE = re.compile(r"^pub struct (\w+)\s*\{")
ENUM_RE = re.compile(r"^pub enum (\w+)\s*\{")
SERDE_KV_RE = re.compile(r'(\w+)\s*=\s*"([^"]*)"')

PRIMITIVES = {"String", "str", "bool", "char", "f32", "f64", "u8", "u16", "u32", "u64", "usize",
              "i8", "i16", "i32", "i64", "isize"}
WRAPPERS = {"Option", "Vec", "Box", "BTreeMap", "HashMap", "BTreeSet", "HashSet"}
# Explained once in the intro instead of the Types appendix.
DESCRIBED = {"Rect", "Insets", "Id", "Length"}
# Hand-written notes for types whose custom `Deserialize` accepts more than the derived form shows.
NOTES = {
    "Color": "Also accepts a string: `\"#rrggbb\"`, `\"#rrggbbaa\"`, `\"cmyk(c,m,y,k)\"` with 0–100 components, "
             "or `black`, `white`, `red`, `green`, `blue`, `none`/`transparent`; and the short scheme form "
             "`{\"scheme\": \"accent1\"}` (optional `\"a\"`). In the object forms `a` and `tint` may be omitted "
             "and default to 1.",
}


def snake(name):
    out = re.sub(r"(?<=[a-z0-9])([A-Z])", r"_\1", name)
    out = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", out)
    return out.lower()


def serde_rename(name, rule):
    """`name` under `#[serde(rename_all = rule)]`, as serde spells it."""
    if rule in (None, "PascalCase"):
        return name
    snake_ = "".join(("_" if i and c.isupper() else "") + c.lower() for i, c in enumerate(name))
    return {
        "lowercase": name.lower(),
        "UPPERCASE": name.upper(),
        "camelCase": name[:1].lower() + name[1:],
        "snake_case": snake_,
        "SCREAMING_SNAKE_CASE": snake_.upper(),
        "kebab-case": snake_.replace("_", "-"),
        "SCREAMING-KEBAB-CASE": snake_.upper().replace("_", "-"),
    }[rule]


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read().splitlines()


def block(lines, start):
    """Lines of the item that opens at `start` up to its closing brace at column 0."""
    body = []
    for line in lines[start + 1:]:
        if line.startswith("}"):
            break
        body.append(line)
    return body


def item_attrs(lines, start):
    """(doc lines, serde attributes) written above the item at `start`."""
    doc, serde = [], {}
    i = start - 1
    while i >= 0 and (lines[i].startswith("#[") or lines[i].startswith("///")):
        s = lines[i]
        if s.startswith("///"):
            doc.insert(0, s[3:].strip())
        elif s.startswith("#[serde("):
            inner = s[len("#[serde("):s.rindex(")")]
            serde.update(SERDE_KV_RE.findall(inner))
            for flag in ("untagged", "default"):
                if re.search(r"\b%s\b(?!\s*=)" % flag, inner):
                    serde[flag] = True
        i -= 1
    return doc, serde


def parse_fields(body):
    """[(name, type, optional, doc)] of a struct body or a struct-variant body (8-space indent)."""
    fields, doc, optional = [], [], False
    for line in body:
        s = line.strip()
        if s.startswith("///"):
            doc.append(s[3:].strip())
        elif s.startswith("#[serde(") and "default" in s:
            optional = True
        elif s.startswith("#[") or s.startswith("//"):
            continue
        else:
            m = FIELD_RE.match(line)
            if m:
                ty = m.group(2)
                optional = optional or ty.startswith("Option<")
                fields.append((m.group(1), ty, optional, " ".join(doc)))
            doc, optional = [], False
    return fields


def split_top(s):
    """Splits `a: T, b: Vec<U, V>` on the commas outside brackets."""
    parts, depth, cur = [], 0, ""
    for c in s:
        if c in "<[(":
            depth += 1
        elif c in ">])":
            depth -= 1
        if c == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += c
    parts.append(cur)
    return [p.strip() for p in parts if p.strip()]


def parse_inline_fields(text):
    """[(name, type, optional, doc)] of a single-line struct body such as `r: u8, g: u8`."""
    fields = []
    for part in split_top(text):
        name, ty = part.split(":", 1)
        ty = ty.strip()
        fields.append((name.strip().removeprefix("pub "), ty, ty.startswith("Option<"), ""))
    return fields


def parse_structs(lines):
    """name → {"fields", "doc", "default"} for every `pub struct` in `lines`."""
    structs = {}
    for i, line in enumerate(lines):
        m = STRUCT_RE.match(line)
        if m:
            doc, serde = item_attrs(lines, i)
            structs[m.group(1)] = {"fields": parse_fields(block(lines, i)), "doc": doc,
                                   "default": bool(serde.get("default"))}
    return structs


def parse_enum(lines, name):
    """{"doc", "serde", "variants": [(variant, doc, fields, tuple_type, serde_name)]} of `pub enum name`.

    `serde_name` is the JSON spelling of the variant after `rename_all` and `rename`."""
    start = next(i for i, l in enumerate(lines) if ENUM_RE.match(l) and ENUM_RE.match(l).group(1) == name)
    edoc, eserde = item_attrs(lines, start)
    body = block(lines, start)
    variants, doc, vserde, i = [], [], {}, 0
    while i < len(body):
        line = body[i]
        s = line.strip()
        if s.startswith("///"):
            doc.append(s[3:].strip())
        elif s.startswith("#[serde("):
            vserde.update(SERDE_KV_RE.findall(s))
        elif s.startswith("#[") or s.startswith("//") or not s:
            pass
        else:
            m = VARIANT_RE.match(line)
            if m:
                vname = m.group(1)
                jname = vserde.get("rename") or serde_rename(vname, eserde.get("rename_all"))
                if m.group(2) == "{" and m.group(4) is None:
                    vbody = []
                    i += 1
                    while i < len(body) and not body[i].startswith("    }"):
                        vbody.append(body[i])
                        i += 1
                    variants.append((vname, " ".join(doc), parse_fields(vbody), None, jname))
                elif m.group(2) == "{":
                    variants.append((vname, " ".join(doc), parse_inline_fields(m.group(3)), None, jname))
                elif m.group(5) is not None:
                    variants.append((vname, " ".join(doc), [], m.group(5).strip(), jname))
                else:
                    variants.append((vname, " ".join(doc), [], None, jname))
            doc, vserde = [], {}
        i += 1
    return {"doc": edoc, "serde": eserde, "variants": variants}


def count_variants(lines, name):
    """Variants of `pub enum name` counted with COUNT_RE, independently of VARIANT_RE."""
    start = next(i for i, l in enumerate(lines) if ENUM_RE.match(l) and ENUM_RE.match(l).group(1) == name)
    return sum(1 for l in block(lines, start) if COUNT_RE.match(l))


def source_files():
    for crate in CRATES:
        src = os.path.join(ROOT, "crates", crate, "src")
        for name in sorted(os.listdir(src)):
            if name.endswith(".rs"):
                yield os.path.join(src, name)


def all_structs():
    """Every `pub struct` in the searched crates, for payloads such as `SetupPatch` and nested types."""
    structs = {}
    for path in source_files():
        structs.update(parse_structs(read(path)))
    return structs


def all_enums():
    """Every `pub enum` in the searched crates, by name."""
    enums = {}
    for path in source_files():
        lines = read(path)
        for line in lines:
            m = ENUM_RE.match(line)
            if m:
                enums[m.group(1)] = parse_enum(lines, m.group(1))
    return enums


def strip_path(ty):
    """`crate::field::Field` → `Field`, `newpub_core::Length` → `Length`."""
    return re.sub(r"\b(?:\w+::)+", "", ty)


def bare_type(ty):
    """`Option<ObjectPatch>` → `ObjectPatch`, `Vec<crate::Id>` → `Id`."""
    ty = strip_path(ty)
    m = re.match(r"^(?:Option|Vec|Box)<(.+)>$", ty)
    return bare_type(m.group(1)) if m else ty


def type_names(ty):
    """Named types inside `ty`, however nested: `BTreeMap<String, Option<Color>>` → {"Color"}."""
    return {t for t in re.findall(r"[A-Za-z_]\w*", strip_path(ty))
            if t not in PRIMITIVES and t not in WRAPPERS and t[:1].isupper()}


class Types:
    """The enums and structs reachable from action and query fields that the Types appendix documents."""

    def __init__(self, structs, enums):
        self.structs, self.enums, self.listed = structs, enums, set()

    def mark(self, name):
        if name in DESCRIBED or name in self.listed or (name not in self.structs and name not in self.enums):
            return
        self.listed.add(name)
        if name in self.structs:
            for _n, ty, _o, _d in self.structs[name]["fields"]:
                self.mark_all(ty)
        else:
            for _v, _doc, fields, tuple_ty, _j in self.enums[name]["variants"]:
                for _n, ty, _o, _d in fields:
                    self.mark_all(ty)
                if tuple_ty:
                    self.mark_all(tuple_ty)

    def mark_all(self, ty):
        for name in sorted(type_names(ty)):
            self.mark(name)

    def inline(self, ty):
        """The struct that a field of type `ty` expands inline (one level), or None."""
        name = bare_type(ty)
        return name if name in self.structs and name not in DESCRIBED else None

    def field_use(self, ty):
        """Records the types a field uses: an inline-expanded struct's own fields, everything else by name."""
        inner = self.inline(ty)
        if inner:
            for _n, ity, _o, _d in self.structs[inner]["fields"]:
                self.mark_all(ity)
        else:
            self.mark_all(ty)

    def ref(self, ty):
        """` (see Types)` when `ty` names something the appendix documents."""
        return " (see Types)" if type_names(ty) & self.listed else ""


def field_line(name, ty, optional, doc, types, indent="", show_optional=True):
    tag = " *(optional)*" if optional and show_optional else ""
    return f"{indent}- `{name}`: `{ty}`{types.ref(ty)}{tag}" + (f" — {doc}" if doc else "")


def json_form(enum, variant, types):
    """How one variant of `enum` is written in JSON."""
    _vname, _doc, fields, tuple_ty, jname = variant
    serde = enum["serde"]
    obj = ", ".join(f'"{n}": {strip_path(t)}' for n, t, _o, _d in fields)
    if serde.get("untagged"):
        return f"`{strip_path(tuple_ty)}`" if tuple_ty else f"`{{{obj}}}`"
    tag = serde.get("tag")
    if tag:
        if tuple_ty:
            return f'`{{"{tag}": "{jname}", ...fields of {strip_path(tuple_ty)}}}`'
        return f'`{{"{tag}": "{jname}"' + (f", {obj}" if obj else "") + "}`"
    if tuple_ty:
        return f'`{{"{jname}": {strip_path(tuple_ty)}}}`'
    if fields:
        return f'`{{"{jname}": {{{obj}}}}}`'
    return f'`"{jname}"`'


def render_types(types):
    out = ["## Types", "",
           "Enums and structs used by the fields above, as they deserialize from JSON. Unit enum variants are",
           "strings; tagged enums are objects carrying the tag field; untagged enums accept any of their forms.",
           "Struct fields marked *optional* may be omitted.", ""]
    for name in sorted(types.listed):
        out.append(f"### `{name}`")
        if name in types.enums:
            enum = types.enums[name]
            serde = enum["serde"]
            if serde.get("untagged"):
                out.append("Enum (untagged): any one of the forms below.")
            elif serde.get("tag"):
                out.append(f"Enum, tagged by `\"{serde['tag']}\"`.")
            else:
                out.append("Enum.")
            out += enum["doc"]
            for variant in enum["variants"]:
                _vname, doc, fields, _tuple_ty, _jname = variant
                out.append(f"- {json_form(enum, variant, types)}" + (f" — {doc}" if doc else ""))
                for fname, ty, optional, fdoc in fields:
                    # The JSON form already names every field; a sub-bullet only when it adds something.
                    if optional or fdoc or types.ref(ty):
                        out.append(field_line(fname, strip_path(ty), optional, fdoc, types, indent="  "))
        else:
            st = types.structs[name]
            out.append("Struct" + (" (every field optional)." if st["default"] else "."))
            out += st["doc"]
            for fname, ty, optional, fdoc in st["fields"]:
                out.append(field_line(fname, strip_path(ty), optional and not st["default"], fdoc, types))
        if name in NOTES:
            out.append(NOTES[name])
        out.append("")
    return out


def render():
    structs, enums = all_structs(), all_enums()
    types = Types(structs, enums)
    sections = [(title, intro, parse_enum(read(path), enum)["variants"]) for title, path, enum, intro in SOURCES]
    # First pass: find every type the appendix must list, so field lines can point at it.
    for _title, _intro, variants in sections:
        for _vname, _doc, fields, tuple_ty, _jname in variants:
            if tuple_ty:
                types.field_use(tuple_ty)
            for _fname, ty, _opt, _fdoc in fields:
                types.field_use(ty)
    out = [
        "# newpub agent reference",
        "",
        "Generated by `tools/agent/reference.py` from `crates/core/src/command.rs` and `crates/engine/src/action.rs`.",
        "Do not edit by hand.",
        "",
        "Conventions: geometry is in points (1/72 in), page coordinates, origin top-left, y down. Any `Length` may",
        "be a number of points or a string with a unit (`\"8.5in\"`, `\"210mm\"`, `\"2cm\"`, `\"12pt\"`, `\"3pi\"`).",
        "A `Rect` is `{x, y, w, h}` or `[x, y, w, h]`, each a `Length`. `Insets` are `{top, bottom, left, right}`,",
        "each a `Length` defaulting to 0 (`inside`/`outside` are accepted for `left`/`right`). Ids are integers",
        "(`Id`); every object, story, page, master, style and asset has",
        "one. Pages are 0-based indexes. Text positions are char indexes into a story's text, where `\\n` separates",
        "paragraphs. Fields marked *optional* may be omitted. Actions and queries are JSON objects: an action is",
        "`{\"cmd\": \"<name>\", ...fields}` and a query is `{\"q\": \"<name>\", ...fields}`; the MCP tools",
        "`newpub_action` and `newpub_query` take the name and the fields separately. Enum and struct types named",
        "in a field are described in the Types appendix at the end.",
        "",
    ]
    for title, intro, variants in sections:
        out += [f"## {title}", "", intro, ""]
        for vname, doc, fields, tuple_ty, _jname in variants:
            out.append(f"### `{snake(vname)}`")
            if doc:
                out.append(doc)
            if tuple_ty:
                inner = types.inline(tuple_ty)
                if inner:
                    out.append(f"Fields (of `{inner}`, all optional unless noted):")
                    for fname, ty, _optional, fdoc in structs[inner]["fields"]:
                        out.append(field_line(fname, ty, False, fdoc, types))
                else:
                    out.append(f"Payload: `{tuple_ty}`{types.ref(tuple_ty)}.")
            elif fields:
                for fname, ty, optional, fdoc in fields:
                    out.append(field_line(fname, ty, optional, fdoc, types))
                    inner = types.inline(ty)
                    if inner:
                        # One level of nesting: patch and attribute structs are what agents need most.
                        for iname, ity, _opt, idoc in structs[inner]["fields"]:
                            out.append(field_line(iname, ity, False, idoc, types, indent="  "))
            else:
                out.append("No fields.")
            out.append("")
    out += render_types(types)
    return "\n".join(out).rstrip() + "\n"


def self_check():
    """Fails when VARIANT_RE misses a variant that the simpler COUNT_RE sees (or vice versa)."""
    total = 0
    for _title, path, enum, _intro in SOURCES:
        lines = read(path)
        parsed, counted = len(parse_enum(lines, enum)["variants"]), count_variants(lines, enum)
        if parsed != counted:
            print(f"reference.py: parsed {parsed} variants of {enum} but counted {counted}; VARIANT_RE "
                  "misses a form", file=sys.stderr)
            sys.exit(2)
        total += parsed
    return total


def main():
    total = self_check()
    text = render()
    if "--check" in sys.argv:
        current = open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else ""
        if current != text:
            print("docs/agent-reference.md is out of date: run python3 tools/agent/reference.py", file=sys.stderr)
            sys.exit(1)
        return
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"wrote {OUT} ({len(text)} bytes, {total} actions and queries)")


if __name__ == "__main__":
    main()
