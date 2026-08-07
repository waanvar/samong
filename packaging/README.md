# Packaging

What turns the binaries into something a person can double-click. Assembled by
the release workflow; kept here as scripts rather than inline YAML so the logic
can be read, reviewed and run by hand.

| Platform | What ships | How it is opened |
|---|---|---|
| Windows | `Open Samong.exe` (a copy of `samong-app.exe`) | Double-click. Built for the windows subsystem, so no console window appears. |
| macOS | `Samong.app` | Double-click. See the note on Gatekeeper below. |
| Linux | `samong-app` + `samong.desktop` | Copy the `.desktop` file to `~/.local/share/applications/` once `samong-app` is on `PATH`. |

## The unsigned-binary problem, stated plainly

Neither the macOS bundle nor the Windows executable is code-signed, because
signing certificates cost money on both platforms (an Apple Developer
membership; an OV/EV certificate for Windows).

The consequence is not cosmetic:

- **macOS** refuses to open `Samong.app` on first launch — "cannot be opened
  because it is from an unidentified developer". The user has to right-click →
  **Open**, then confirm. That is one extra step on the very first run, and it
  arrives at the worst possible moment: before they have seen anything.
- **Windows** SmartScreen may warn on `Open Samong.exe` until the file has been
  downloaded enough times to earn reputation. "More info" → "Run anyway".

### The fix is not money, which took a while to establish

The obvious conclusion is "buy a certificate", and it is wrong — or at least it
buys much less than it looks like.

A certificate replaces **"Unknown publisher"** with a name. It does not remove the
warning. Microsoft changed SmartScreen in 2024 so that even an EV certificate no
longer grants reputation on sight; both OV and EV now have to accumulate it from
real downloads. Worse, reputation is bound to the certificate's thumbprint, so it
resets to zero on renewal — and since 27 February 2026 a code-signing certificate
may be valid for at most 459 days. A project at this scale would spend money to be
back at the same dialog every fifteen months.

**The actual fix is to stop arriving through a browser.** Scoop, Homebrew, winget
and `cargo install` all fetch and verify the archive themselves, so no download
reputation is ever weighed and macOS never attaches `com.apple.quarantine`. That
path exists today and is free; the README lists it first for this reason.

Worth revisiting only if enough people install by double-click to make the
"Unknown publisher" line itself the thing costing installs.

## Icons

Shipped, and generated rather than drawn by hand — `packaging/icons/make-icons.py`
derives every size from `assets/icon/samong-icon.svg`.

| Platform | Where the icon lives |
|---|---|
| Windows | a resource **inside** each `.exe`, embedded by `build.rs` |
| macOS | `Samong.app/Contents/Resources/samong.icns`, named by `CFBundleIconFile` |
| Linux | `icons/hicolor/<size>/apps/samong.png` in the archive, found via `Icon=samong` |

There are two artworks, not one. Below about 24px the full mark's four links and
four nodes fall under a pixel each and render as grey haze, so
`samong-icon-small.svg` keeps one node, one link and the lit node — the question,
the path, the answer — and the full mark takes over from 32px up.

`verify-stage.sh` asserts the icon per platform before archiving, and on Windows
that means reading the resource directory back out of the `.exe`: `build.rs` only
warns when no resource compiler is present, so a build can succeed without one.

## Why the names are checked, not assumed

`packaging/verify-stage.sh` runs before each archive is created and fails the
build if a binary is missing or if two differently-named binaries turn out to be
the same file.

That check exists because v0.3.3 shipped a Windows archive whose `samong.exe` was
the GUI launcher instead of the command-line tool: the copy was named
`Samong.exe`, and on a case-insensitive filesystem that is the same file. Every
job passed. The only way to see it was to read the subsystem flag out of the
published binary — which is not a thing anyone does routinely, and so is not a
thing a release should depend on.

## Publishing to crates.io needs `--allow-dirty`

The web UI is embedded at compile time from `web/dist`, which is gitignored yet
deliberately listed in `include`, so `cargo publish` sees 55 files git does not
track and refuses. The flag is unavoidable.

Used bare it would also silence the warning it exists to give — uncommitted source
going out in a release nobody can reproduce. So
`packaging/check-publish-tree.sh` runs first and refuses if anything cargo would
ship is untracked *outside* `web/dist`.

Its file list comes from `cargo package --list`, not from git. Ignored files are
absent from `git status` entirely — it reports zero while cargo counts 55 — and
asking git for every ignored path instead returns `target/` and
`web/node_modules/`, which are not in the package at all.

```sh
bash packaging/check-publish-tree.sh
cargo publish --locked --allow-dirty
```
