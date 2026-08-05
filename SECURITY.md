# Security

## Reporting

Use GitHub's private reporting: **[Report a
vulnerability](https://github.com/waanvar/samong/security/advisories/new)**. It
reaches the maintainer without the details being public first.

If that is unavailable to you, email **waanvar@gmail.com** with `samong security`
in the subject.

This is a one-person project, so please do not expect a same-day reply. You will
get an acknowledgement, and if the report is valid, a fix and a release that says
what was wrong.

## What is in scope

Samong binds `127.0.0.1` and stores notes as files in a folder you chose. There is
no server of ours, no account, and no telemetry, so most of the usual surface does
not exist. What remains, and is worth reporting:

- **Path escape.** A note key, a `scope.include` entry, or an API path that reads
  or writes outside the vault root.
- **A write reaching a read-only note.** Reference notes from `scope.include` —
  including a vault installed from someone else — must refuse every write path
  (CLI, HTTP, MCP).
- **The local HTTP server reachable from off the machine**, or accepting a
  cross-origin request that mutates notes.
- **`samong vault install` / `update` running something it should not**, or
  accepting content signed by a key other than the one pinned at install.
- **An MCP tool doing more than it says** — for example returning notes from a
  vault the caller did not name.
- **The published artefacts not matching the source they claim**: a release
  archive, the `.mcpb` bundle, or the crates.io package containing something the
  tagged commit does not build.

## What is already known, and not a vulnerability

These are documented trade-offs, not oversights. A report about them is welcome as
a discussion, but it will not be treated as a security issue.

- **The binaries are not code-signed.** macOS Gatekeeper refuses them and Windows
  SmartScreen warns. Certificates cost money this project does not have;
  `cargo install` avoids the problem entirely. See the README.
- **The search index holds a full copy of every note's body**, and titles of
  deleted notes can survive in `graph.redb` until their pages are reused.
  Anyone with your `.brain/` directory effectively has your notes. `samong pack`
  exists precisely because a vault must be publishable without it.
- **Anyone who can reach `127.0.0.1` on your machine can read and write your
  notes.** That is every local process; the server has no authentication, by
  design, because it is a local tool rather than a shared service.
- **An installed vault's contents are not sandboxed.** Its notes are Markdown that
  Samong indexes and renders; it is data, not code. But a vault you install is a
  git repository from someone else — `samong vault verify` tells you who signed it
  and whether it changed.

## Supported versions

The latest release only. There are no long-term support branches; fixes go out as
a new patch version.
