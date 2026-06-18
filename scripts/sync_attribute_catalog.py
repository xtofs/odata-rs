#!/usr/bin/env python3
"""Render the generated tables in docs/edm-semantic-graph.md from the
machine-readable catalog docs/csdl-attribute-catalog.toml.

The catalog is the single source of truth for the structured facts. Only the
table regions delimited by

    <!-- GENERATED:<name> -->
    ... table ...
    <!-- END:<name> -->

are produced from it; all prose in the document is hand-authored and untouched.

Usage:
    python3 scripts/sync_attribute_catalog.py          # rewrite the regions
    python3 scripts/sync_attribute_catalog.py --check   # exit 1 if out of date
"""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "docs" / "csdl-attribute-catalog.toml"
DOC = ROOT / "docs" / "edm-semantic-graph.md"


def esc(text: str) -> str:
    return str(text).replace("|", "\\|")


def md_table(headers: list[str], rows: list[list[str]]) -> str:
    out = ["| " + " | ".join(headers) + " |",
           "| " + " | ".join("---" for _ in headers) + " |"]
    for row in rows:
        out.append("| " + " | ".join(esc(c) for c in row) + " |")
    return "\n".join(out)


def target_or_segment(attr: dict) -> str:
    if attr["category"] == "reference":
        return ", ".join(attr.get("reference_targets", [])) or "—"
    if attr["category"] == "path":
        return attr.get("path_segment_domain", "—")
    return "—"


def render_constraints(attr: dict) -> str:
    """Format the structured path-attribute constraints (head_kinds /
    terminal_kinds / descent_kinds) as a compact prose fragment. Empty
    string when the attribute has no structured constraints.

    Example output: `head: BindingParameter; terminal: EntitySet, Singleton; descent: ComplexType`
    """
    if attr.get("category") != "path":
        return ""
    parts: list[str] = []
    for key, label in (
        ("head_kinds", "head"),
        ("terminal_kinds", "terminal"),
        ("descent_kinds", "descent"),
    ):
        kinds = attr.get(key)
        if kinds:
            parts.append(f"{label}: {', '.join(f'`{k}`' for k in kinds)}")
    return "; ".join(parts)


def merge_notes(attr: dict) -> str:
    """Combine structured-constraint summary with free-form notes for display."""
    structured = render_constraints(attr)
    prose = attr.get("notes", "").strip()
    if structured and prose:
        return f"{structured}. {prose}"
    return structured or prose


def render_segment_domains(domains: list[dict]) -> str:
    """Render the segment-domain legend as a bullet list. Each bullet names the
    domain and inline-codes its members; resolution context goes in the notes."""
    if not domains:
        return ""
    lines = []
    for d in domains:
        members = " | ".join(f"`{m}`" for m in d.get("members", []))
        line = f"- **{d['name']}** — {members}"
        notes = d.get("notes")
        if notes:
            line += f". {notes}."
        lines.append(line)
    # The "(not yet typed: String)" pseudo-domain is documented in prose, not
    # the catalog, because it represents a resolver-implementation gap rather
    # than a CSDL-level concept.
    return "\n".join(lines)


def resolve_value_type(attr: dict, defaults: dict[str, str]) -> str:
    """For a value attribute, return its `type` (TypeScript-style) — either the
    per-attribute `type` field or the default keyed by attribute name."""
    if attr.get("category") != "value":
        return ""
    explicit = attr.get("type")
    if explicit:
        return explicit
    return defaults.get(attr["name"], "")


def blank_repeated_first_column(rows: list[list[str]]) -> list[list[str]]:
    """Markdown tables have no rowspan. To get the same visual effect we leave
    the first column blank when it matches the previous row — making per-element
    groupings scan more cleanly when the same element has many attributes."""
    out: list[list[str]] = []
    prev = None
    for row in rows:
        if not row:
            out.append(row)
            continue
        if row[0] == prev:
            out.append(["", *row[1:]])
        else:
            prev = row[0]
            out.append(row)
    return out


def render(catalog: dict) -> dict[str, str]:
    regions: dict[str, str] = {}

    regions["containment"] = md_table(
        ["Parent", "Child", "Edge label"],
        blank_repeated_first_column(
            [[c["parent"], c["child"], "has"] for c in catalog.get("containment", [])]
        ),
    )

    value_type_defaults = catalog.get("value_type_defaults", {})

    attrs = catalog.get("attribute", [])
    for category in ("reference", "path", "value"):
        rows = []
        for a in attrs:
            if a["category"] != category:
                continue
            row = [
                a["element"],
                a["name"],
                a.get("cardinality", ""),
            ]
            if category != "value":
                row.append(target_or_segment(a))
            else:
                row.append(f"`{resolve_value_type(a, value_type_defaults)}`")
            row += [merge_notes(a)]
            rows.append(row)
        headers = ["Element", "Attribute", "Card."]
        if category != "value":
            headers.append("Target / Segment")
        else:
            headers.append("Type")
        headers += ["Notes"]
        regions[f"attributes-{category}"] = md_table(
            headers, blank_repeated_first_column(rows)
        )

    regions["segment-domains"] = render_segment_domains(
        catalog.get("segment_domain", [])
    )

    return regions


def apply_regions(text: str, regions: dict[str, str]) -> str:
    for name, table in regions.items():
        pattern = re.compile(
            r"(<!-- GENERATED:" + re.escape(name) + r" -->\n).*?(\n<!-- END:"
            + re.escape(name) + r" -->)",
            re.DOTALL,
        )
        if not pattern.search(text):
            raise SystemExit(f"error: missing region markers for '{name}' in {DOC}")
        text = pattern.sub(lambda m: m.group(1) + table + m.group(2), text)
    return text


def main() -> int:
    check = "--check" in sys.argv[1:]
    with CATALOG.open("rb") as fh:
        catalog = tomllib.load(fh)
    regions = render(catalog)

    original = DOC.read_text()
    updated = apply_regions(original, regions)

    if check:
        if original != updated:
            print("docs/edm-semantic-graph.md is out of sync with the catalog. "
                  "Run scripts/sync_attribute_catalog.py.", file=sys.stderr)
            return 1
        print("in sync")
        return 0

    if original != updated:
        DOC.write_text(updated)
        print("updated docs/edm-semantic-graph.md")
    else:
        print("already up to date")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
