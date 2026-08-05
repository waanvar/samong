# winget

```powershell
winget install Waanvar.Samong
```

Once the first submission is accepted. Until then, Scoop is the Windows route:

```powershell
scoop bucket add samong https://github.com/waanvar/scoop-samong
scoop install samong
```

## What is here

| | |
|---|---|
| `generate.py` | writes the three manifests for a released version |
| `manifests/` | the generated manifests for the current release, committed so they can be read and validated |

The manifests are **generated, not hand-edited**. `PackageVersion`, the installer
URL, the SHA-256 and four `RelativeFilePath` entries all have to agree; editing by
hand means six places to keep in step, and the failure is invisible until
Microsoft's validation pipeline rejects the pull request days later.

CI asserts the committed copies are exactly what the generator produces
(`generate.py --check`), so they cannot drift.

## Why `InstallerType: zip` with a portable nested type

The release artefact is a zip of loose executables — there is no MSI and no setup
program, because Samong installs nothing and writes only to the folder of notes you
choose. winget's `NestedInstallerType: portable` is the case for exactly that: it
unpacks the archive and puts aliases on `PATH`.

Four aliases are declared: `samong`, `samong-server`, `samong-mcp`, `samong-app`.
`Open Samong.exe` is deliberately **not** among them — an alias with a space is
awkward to invoke, and it is byte-identical to `samong-app.exe`. It exists in the
archive for people who unzip by hand and want something obvious to double-click.

## Why it is not code-signed, and why that matters less here

There is no certificate; one costs money the project does not have. A browser
download can trip SmartScreen. winget fetches the archive itself and verifies the
SHA-256 in the manifest before unpacking, so nothing arrives through a browser to
accumulate — or fail — a reputation check.

## Submitting

Normally the release workflow does it, if a `WINGET_TOKEN` secret exists: a
fine-grained PAT with public-repo access, used to fork `microsoft/winget-pkgs` and
open the pull request. `GITHUB_TOKEN` cannot write to somebody else's repository.

Without the secret the job **skips and says so** rather than failing. A winget
submission sits in Microsoft's review queue regardless, so a release that opened no
pull request is a decision rather than a defect — unlike a missed crates.io publish,
which drifts silently and is made loud on purpose.

By hand:

```powershell
python packaging\winget\generate.py
winget validate --manifest packaging\winget\manifests
# wingetcreate from https://aka.ms/wingetcreate/latest
wingetcreate submit --token <PAT> packaging\winget\manifests
```

## Local install, and the setting it needs

`winget install --manifest` requires `winget settings --enable LocalManifestFiles`,
which needs an administrator. That is a real change to a machine's configuration, so
it is not something this project's tooling does for you — CI runners are disposable
and enable it there instead, which is where a local-manifest install is actually
exercised on every push.
