# Building RavenLinux with Docker / Podman

RavenLinux is a Linux-From-Scratch style distribution. Its build performs
operations that only exist on a Linux host — `chroot`, `overlayfs` mounts,
loop-device setup — and runs a `musl` cross-toolchain. As a result it **cannot
be built natively on macOS or Windows**.

The repository ships a `Dockerfile` and a helper script that provide a
reproducible Linux build environment, so you can produce a RavenLinux ISO from
any host that runs Docker or Podman.

## Quick Start

```bash
# Build everything; the ISO lands in ./build/ on your host
./scripts/docker-build.sh all

# Boot/test the resulting ISO with QEMU (macOS example)
brew install qemu
qemu-system-x86_64 -machine q35 -m 4096 -smp 4 \
    -drive if=pflash,format=raw,readonly=on,file=$(brew --prefix qemu)/share/qemu/edk2-x86_64-code.fd \
    -cdrom build/ravenlinux-*.iso -vga virtio
```

## How It Works

| Piece | Role |
|-------|------|
| `Dockerfile` | Defines an Arch Linux build host with the full toolchain installed (mirrors `scripts/check-deps.sh`). |
| `scripts/docker-build.sh` | Auto-detects Docker or Podman, builds the image (cached), and runs the container with the correct flags. |
| `.dockerignore` | Keeps the image build context to just the `Dockerfile` — the repo is bind-mounted at run time, not copied in. |

The container runs **`--privileged`** because the build needs `chroot`, mounts
and loop devices. The repository is **bind-mounted at `/raven`**, so every
artifact — the ISO, toolchain, sysroot — is written to `./build/` on your host
and survives after the container exits.

Arch Linux is used as the base image because RavenLinux is primarily tested on
Arch, and `check-deps.sh` maps the most complete package set to `pacman`.

## Requirements

- **Docker** (Docker Desktop on macOS/Windows) **or Podman**
- ~**20 GB+ free disk** and **8 GB+ RAM** (the build downloads and compiles a
  toolchain, kernel, and packages)
- Internet access (the build fetches sources, including the Raven compositor)

No other host packages are required — everything the build needs lives inside
the image.

## Usage

### Helper script

```bash
./scripts/docker-build.sh [BUILD_ARGS...]
```

`BUILD_ARGS` are passed straight through to `scripts/build.sh`. With no
arguments it runs a full build (`build.sh all`).

| Command | Description |
|---------|-------------|
| `./scripts/docker-build.sh` | Full build (equivalent to `all`) |
| `./scripts/docker-build.sh all` | Build everything and generate the ISO |
| `./scripts/docker-build.sh stage0` | Build only the cross-compilation toolchain |
| `./scripts/docker-build.sh stage1` | Build the base system |
| `./scripts/docker-build.sh stage2` | Native rebuild |
| `./scripts/docker-build.sh stage3` | Build additional packages |
| `./scripts/docker-build.sh stage4` | Generate the bootable ISO |
| `./scripts/docker-build.sh -j 8 stage1` | Pass options through (8 parallel jobs) |
| `./scripts/docker-build.sh --clean all` | Clean rebuild from scratch |
| `./scripts/docker-build.sh shell` | Drop into an interactive shell in the build environment |

See [the main build documentation](../README.md#building-from-source) for the
full list of stages.

### Running the engine directly

If you prefer not to use the helper:

```bash
# Build the image once (cached afterwards)
docker build -t ravenlinux-build .

# Run a build
docker run --rm -it --privileged \
    -v "$PWD:/raven" -w /raven \
    ravenlinux-build ./scripts/build.sh all

# Interactive shell
docker run --rm -it --privileged \
    -v "$PWD:/raven" -w /raven \
    ravenlinux-build /bin/bash
```

Substitute `podman` for `docker` if you use Podman.

## Environment Variables

The helper script honours these:

| Variable | Default | Description |
|----------|---------|-------------|
| `RAVEN_ENGINE` | auto-detected | Force the container engine: `docker` or `podman`. |
| `RAVEN_IMAGE` | `ravenlinux-build` | Image tag to build and run. |
| `RAVEN_NO_BUILD` | `0` | Set to `1` to skip the image build and reuse an existing image. |

Example:

```bash
RAVEN_ENGINE=podman RAVEN_NO_BUILD=1 ./scripts/docker-build.sh stage4
```

## Notes for Apple Silicon (M1/M2/M3/M4)

The RavenLinux ISO targets **`x86_64`**. On Apple Silicon, Docker and Podman run
x86_64 containers under emulation:

- The build **works**, but is **noticeably slower** than on a native x86_64 host.
  A full from-scratch build can take a long time — build individual stages while
  iterating, and only run `all` for a complete ISO.
- For faster builds, run the same `Dockerfile` on a native **x86_64 Linux host**
  (e.g. a cloud VM) and copy the ISO back.
- To **boot/test** the finished ISO on macOS, use QEMU (`qemu-system-x86_64`
  with a UEFI firmware file). RavenLinux is UEFI-only — see the
  [README QEMU instructions](../README.md).

## Podman Specifics

- **Rootful vs. rootless:** the build's `chroot`/`mount` steps expect to run as
  root inside the container. The helper uses `--privileged` with the container's
  default root user, which works for both rootful Podman and Docker. On some
  rootless Podman setups you may need `sudo podman` or a rootful machine.
- **SELinux:** on SELinux-enabled Linux hosts, the helper adds the `:z` relabel
  flag to the bind mount automatically. (It is omitted on macOS, where it is
  unsupported.)

## Troubleshooting

| Symptom | Cause / Fix |
|---------|-------------|
| `neither 'docker' nor 'podman' found` | Install Docker Desktop or Podman, or set `RAVEN_ENGINE`. |
| `mount: permission denied` / `chroot: ... Operation not permitted` | The container is not privileged. Use the helper script, or add `--privileged` to your `docker run`. |
| `no space left on device` | The Docker/Podman VM ran out of disk. Increase the VM disk size (Docker Desktop → Settings → Resources) or prune images. |
| Build artifacts owned by `root` on the host | Expected: the privileged container runs as root. Re-run a build (the build script fixes ownership), or `sudo chown -R "$USER" build`. |
| Image build fails on a package name | Arch package names occasionally change. Update the offending package in the `Dockerfile` and rebuild. |
| Very slow build on a Mac | x86_64 emulation on Apple Silicon — see the notes above; consider a native x86_64 Linux host. |

## Related

- [`README.md` → Building from Source](../README.md#building-from-source)
- [`docs/dependency-check.md`](dependency-check.md) — the dependency list the
  `Dockerfile` mirrors
