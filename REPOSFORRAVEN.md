# GitHub Repos for RavenLinux

The software RavenLinux provides for itself. Everything marked **wired** is
built and installed by `scripts/stages/stage-raven.sh` and has a definition
under `packages/raven/`.

Build it with `make raven` (or `./scripts/build.sh raven`). See the Build Stages
section of [README.md](README.md) for the per-component env knobs.

## Shell — not bash, not fish

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `ravenshell` | [javanhut/RavenShell](https://github.com/javanhut/RavenShell) |

The default login shell. `/etc/raven/raven-shell` prefers it, falls back to
bash, and finally to `/bin/sh`. bash and fish stay installed and listed in
`/etc/shells` — the fallback path has to work on a base build that skipped the
Raven layer.

## Package Managers

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `rvn` | [javanhut/RavenPackageManager](https://github.com/javanhut/RavenPackageManager) — the default |
| **wired** | `poxy` | [javanhut/Poxy](https://github.com/javanhut/Poxy) — universal |

Neither consumes the `packages/*/package.toml` definitions yet. Those files
currently document what the system contains; teaching `rvn` to read them
directly is the obvious next step.

## Version Control

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `ivaldi` | [javanhut/Ivaldi](https://github.com/javanhut/Ivaldi) |

Already this repository's own VCS.

## Text Editor

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `crow` | [javanhut/CrowTextEditor](https://github.com/javanhut/CrowTextEditor) |

## Task Runner

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `imlazy` | [javanhut/ImLazy](https://github.com/javanhut/ImLazy) |

## Languages

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `oxigen` | [javanhut/OxigenLang](https://github.com/javanhut/OxigenLang) |
| not wired | — | Rust toolchain |
| not wired | — | Go toolchain |

`oxigen` ships as a static interpreter, so it needs nothing else installed.

Rust and Go are a different problem. They are currently **build host**
dependencies — the Dockerfile installs both, and the Raven layer uses them to
cross-compile every component to a static binary. Shipping the compilers
*inside* the distribution is a separate bootstrap job (a musl-hosted rustc plus
cargo, and a Go toolchain built for musl), large enough to deserve its own stage
rather than a row in the component table.

## GUI

| Status | Binary | Repo |
|--------|--------|------|
| not wired | `raven-terminal` | [javanhut/RavenTerminal](https://github.com/javanhut/RavenTerminal) |

RavenTerminal is GPU-accelerated and links OpenGL and GLFW through cgo, so it
needs a display server and a graphics stack. The console base has none of that.
It belongs with whatever graphical layer gets built back — at which point it
wants its own stage, not a slot in the Raven layer.
