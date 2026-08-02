# Packaging

What turns the binaries into something a person can double-click. Assembled by
the release workflow; kept here as scripts rather than inline YAML so the logic
can be read, reviewed and run by hand.

| Platform | What ships | How it is opened |
|---|---|---|
| Windows | `Samong.exe` (a copy of `samong-app.exe`) | Double-click. Built for the windows subsystem, so no console window appears. |
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
- **Windows** SmartScreen may warn on `Samong.exe` until the file has been
  downloaded enough times to earn reputation. "More info" → "Run anyway".

This is the honest state, and it is the largest remaining barrier for exactly the
non-technical audience the launcher exists for. The fix is money, not code.

## Icons

Not yet. The brand assets in `site/brand/` are SVG, and turning them into `.ico`
and `.icns` needs a rasteriser in CI plus (for Windows) a build script to embed
the resource. A bundle without an icon still opens; it just looks unfinished.
