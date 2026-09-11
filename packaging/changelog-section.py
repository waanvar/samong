"""Cut one version's section out of CHANGELOG.md, for the GitHub release body.

Every release so far has been published with an empty description. The notes
existed the whole time — written carefully, in CHANGELOG.md — but the release
workflow only ever passed `tag_name` and `files` to the upload action, so the
page a user lands on from the site, from `samong update`, or from a link in a
post said nothing about what changed.

The fix is not to write the notes a second time. A second copy is a second thing
to keep in step, and the one that gets forgotten is always the copy. The
CHANGELOG is the source; this prints the part of it that belongs to a tag.

Refuses rather than prints nothing:

  - no section for that version is an error, because a release built from a tag
    nobody wrote notes for should stop and be noticed, not ship silently blank.
  - a section that is only the `_Nothing yet._` placeholder is the same error
    wearing a heading, so it is rejected by name.

`--check` walks every released section instead, which is what CI runs: it is the
cheap way to notice that an edit to CHANGELOG.md broke the shape this script
depends on, months before the next tag would have found out.

Usage:
  changelog-section.py v0.5.0              print the section
  changelog-section.py v0.5.0 --out FILE   write it to a file
  changelog-section.py --check             every released section must be usable
"""

import argparse
import re
import sys
from pathlib import Path

CHANGELOG = Path(__file__).parent.parent / "CHANGELOG.md"
# The released headings only. `## Unreleased` is deliberately not matched: it is
# not a version and must never become a release body.
HEADING = re.compile(r"^## (\d+\.\d+\.\d+)\b")
ANY_HEADING = re.compile(r"^## ")
PLACEHOLDER = "_Nothing yet._"


def sections(text):
    """Every released version in file order, as {version: body}."""
    out, version, buf = {}, None, []
    for line in text.splitlines():
        m = HEADING.match(line)
        if m:
            if version is not None:
                out[version] = "\n".join(buf).strip("\n")
            version, buf = m.group(1), []
            continue
        if ANY_HEADING.match(line) and version is not None:
            # A non-version `##` — `## Unreleased`, or anything added later —
            # ends the section it follows.
            out[version] = "\n".join(buf).strip("\n")
            version, buf = None, []
            continue
        if version is not None:
            buf.append(line)
    if version is not None:
        out[version] = "\n".join(buf).strip("\n")
    return out


def check_body(version, body):
    if not body:
        return f"the section for {version} is empty"
    if body.strip() == PLACEHOLDER:
        return f"the section for {version} is still the {PLACEHOLDER} placeholder"
    return None


def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("version", nargs="?", help="tag or bare version, e.g. v0.5.0")
    ap.add_argument("--out", help="write the section here instead of stdout")
    ap.add_argument("--check", action="store_true", help="validate every section")
    args = ap.parse_args()

    text = CHANGELOG.read_text(encoding="utf-8")
    found = sections(text)

    if args.check:
        if not found:
            sys.exit(f"{CHANGELOG.name} has no released sections — the heading shape changed")
        problems = [p for v, b in found.items() if (p := check_body(v, b))]
        if problems:
            sys.exit("\n".join(problems))
        print(f"{len(found)} released section(s) in {CHANGELOG.name}, all usable: "
              + ", ".join(found))
        return

    if not args.version:
        ap.error("a version is required unless --check is given")

    version = args.version.lstrip("vV")
    body = found.get(version)
    if body is None:
        sys.exit(
            f"CHANGELOG.md has no `## {version}` section. Releasing a version "
            f"whose notes were never written would publish an empty page; "
            f"write the section first. Found: {', '.join(found) or 'nothing'}"
        )
    problem = check_body(version, body)
    if problem:
        sys.exit(problem)

    if args.out:
        Path(args.out).write_text(body + "\n", encoding="utf-8")
        print(f"wrote {len(body.splitlines())} line(s) of notes for {version} to {args.out}")
    else:
        print(body)


if __name__ == "__main__":
    main()
