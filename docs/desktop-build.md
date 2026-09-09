# Building the complete Raven desktop

On the builder PC, use the updated RavenLinux checkout (including
`configs/desktop/` and the new Python helpers) and run:

```bash
imlazy build
```

Or, without ImLazy:

```bash
./scripts/docker-build.sh all
```

The full build recreates stage2's sysroot and rebuilds the Raven and GUI layers.
`imlazy iso` only repackages an existing sysroot; it does not fetch application
fixes. After updating this build recipe, run the full build. No files from the
developer PC's home directory, installed binaries, or sibling checkouts are
needed on the builder.

The image includes Huginn and raven-output, the Raven applications, login,
wallpaper, status bar, init daemons, and audio/Bluetooth runtime. PipeWire,
WirePlumber, and pipewire-pulse start in the graphical session independently of
the bar. BlueZ runs in the foreground under raven-init. Polkit has its own
image-local service account. The Realtek rtw88 deep-power-save workaround
from the working PC is included in fresh installs.

New users and the live user receive the defaults in `configs/desktop/`: dark
appearance, teal accent, 0.9 interface scale, top bar, Files preferences, and
the bundled wallpaper. Personal bookmarks, credentials, monitor coordinates,
and local executable paths are not part of those defaults. The installer uses
`/etc/skel`; these defaults do not overwrite existing users' preferences.

## Required image checks

Immediately before squashfs creation, after cleanup and stripping, packaging
checks executable locations, enabled daemon commands/accounts, shared-library
and ELF-loader availability, audio plugins, desktop data, and source records.
A missing component or failed check stops the ISO. Failed source updates cannot
reuse stale checkouts silently, invalid branch/tag requests cannot fall back to
the default branch, and full GUI rebuilds invalidate previous component output.

For deliberately incomplete diagnostic images only, set
`RAVEN_ALLOW_INCOMPLETE=1`. Their manifest records validation errors and
`complete: false`. This switch does not turn a broken runtime-staging operation
into a successful GUI stage.

These checks do not replace boot testing on the target hardware. They check
ELF dependencies without executing target programs; they do not prove ABI
symbol-version compatibility, GPU initialization, or successful login.

## Component versions and repeat builds

The default build fetches the remote default branch of each first-party
component. Publish component fixes to the configured remote before building on
another PC. The build does not infer which local developer binaries were tested.
Every successful ISO has two adjacent records:

- `raven-<version>-<arch>.iso.manifest.json`: component revisions, final binary
  and configuration SHA-256 hashes, and build-recipe input hashes.
- `raven-<version>-<arch>.iso.sources.tsv`: exact first-party repository commits.

The same records ship under `/usr/share/raven/build/`; the runtime package
versions used for desktop staging are in `desktop-packages.txt` there.

To reuse an ISO's first-party commits on another builder:

```bash
RAVEN_SOURCE_LOCK=/path/to/previous.iso.sources.tsv imlazy build
```

The wrapper mounts that file read-only. Every component needs a full commit
entry, including RavenTerminal. A lock conflicting with an explicit ref or an
enabled manifest pin fails. This locks first-party source selection, not the
Arch container, external dependencies, toolchains, or byte-for-byte ISO output.

For a one-off component selection, the wrapper forwards all registered
component `REF`, `OFFLINE`, and `SKIP` settings, plus GUI controls. For example:

```bash
GUI_REF=<published-commit> SETTINGS_REF=<published-commit> imlazy build
```

Offline builds require existing clean checkouts; requested refs are checked
against the actual checkout. A fresh builder should use an online full build.

## Regression checks

```bash
python3 scripts/test-desktop-build.py
./scripts/check-manifests.sh
```

The regression suite uses temporary local repositories and synthetic image
roots. It does not access the network or modify the running desktop.
