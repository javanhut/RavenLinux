#!/bin/bash
# =============================================================================
# RavenLinux Raven Stage: Self-Hosted Toolchain
# =============================================================================
# Builds the software RavenLinux provides for itself -- the shell, package
# managers, version control, editor, task runner, wireless stack and language
# it ships instead of inheriting from elsewhere. See REPOSFORRAVEN.md for the
# roadmap this implements.
#
# This stage is deliberately *not* numbered. Stages 0-4 build the base system
# and the ISO; those must exist and work on their own. This layer sits on top
# of stage3 and must run before stage4, because stage4 squashes the sysroot
# into the ISO -- anything installed after it would not ship.
#
#   stage0 -> stage1 -> stage2 -> stage3 -> raven -> stage4
#
# Every component is built as a *static* binary, so nothing here adds a runtime
# link dependency on the sysroot:
#
#   Go   -> CGO_ENABLED=0                       (static by construction)
#   Rust -> --target x86_64-unknown-linux-musl  (static by construction)
#
# The stage is fail-soft by design. A component that will not clone or will not
# compile is logged and skipped; the rest of the stage, and the ISO build after
# it, still succeed. A base system without Crow is a base system; a build that
# aborts halfway through leaves you with nothing.
#
# Package definitions live in packages/raven, and they are no longer only
# documentation: once a component is built, `rvn build` turns it into a real
# package and the sysroot is populated by installing that. See the Packaging
# section below for what that buys, and for what happens to the files the
# earlier stages already put where a package wants to be.
#
# Environment:
#   RAVEN_ONLY=crow,ivaldi        build only these components
#   RAVEN_SKIP=oxigen             skip these components
#   RAVEN_OFFLINE=1               never touch the network; use existing clones
#   RAVEN_<KEY>_REF=<git-ref>     pin one component (e.g. RAVEN_IVALDI_REF=v0.1.2),
#                                 overriding its manifest pin. A 40-character
#                                 commit id works here as well as a tag.
#   RAVEN_USE_MANIFEST_PINS=1     honour the packages/*/package.toml pins.
#                                 Off by default: components track their
#                                 default branch. See the Pins section of
#                                 scripts/lib/components.sh.
#   RAVEN_IGNORE_MANIFEST_PINS=1  ignore every packages/*/package.toml pin and
#                                 build the default branch of everything
#
# With neither of those set, each component is fetched at the [source] commit
# in packages/raven/<key>/package.toml. Those pins are what make two builds of
# an unchanged tree produce the same software; a component whose manifest
# carries no pin tracks its default branch and says so in the log.
#   RAVEN_KEEP_BASH_DEFAULT=1     install ravenshell but leave bash as the
#                                 default login shell
#   RAVEN_PACMAN_FROM_HOST=1      give rvn the build host's pacman.conf and
#                                 mirrorlist instead of the canonical ones in
#                                 configs/rvn. The [raven] section is appended
#                                 to it rather than lost with the file it was
#                                 written in -- see install_raven_repo_section.
#   RAVEN_REPO=1                  ship the [raven] repository enabled, using
#                                 the Server line in configs/rvn/pacman.conf.
#                                 Off by default: a repository that answers
#                                 nothing warns on every sync.
#   RAVEN_REPO_SERVER=<url>       enable [raven] with this Server instead. A
#                                 file:// URL naming a directory `rvn repo-add`
#                                 has written is a repository rvn can install
#                                 from with no web server in front of it.
#   RAVEN_SIGNING_PUBKEY=<path>   the Raven signing key's PUBLIC half, which
#                                 the image needs to verify anything from
#                                 [raven]. Defaults to
#                                 configs/rvn/raven-signing-key.{gpg,asc} if
#                                 either exists. With [raven] enabled and no
#                                 key, the build FAILS -- see
#                                 docs/raven-repository-signing.md.
#   RAVEN_SKIP_PACKAGING=1        install the built binaries as loose files and
#                                 do not package them. The image is then what
#                                 it was before the Packaging section existed:
#                                 it works, and nothing in it owns anything.
#   RAVEN_PACKAGING_RVN=<path>    the rvn to package with, instead of the one
#                                 this build just produced. See the Packaging
#                                 section for the order the candidates are
#                                 tried in.
# =============================================================================

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
PACKAGES_DIR="${PACKAGES_DIR:-${BUILD_DIR}/packages}"
SOURCES_DIR="${SOURCES_DIR:-${BUILD_DIR}/sources}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"
RAVEN_JOBS="${RAVEN_JOBS:-$(nproc)}"

# Where component checkouts live, and where built binaries are staged before
# they are installed into the sysroot.
RAVEN_SRC_DIR="${SOURCES_DIR}/raven"
RAVEN_STAGE_DIR="${PACKAGES_DIR}/raven"

# Static build target for Rust components. Overridable so this stage can be
# exercised against a host target when the musl std is not installed.
RUST_MUSL_TARGET="${RUST_MUSL_TARGET:-x86_64-unknown-linux-musl}"

# The musl cross toolchain stage0 builds. Its bin/ is not on PATH by default.
TOOLCHAIN_DIR="${TOOLCHAIN_DIR:-${BUILD_DIR}/toolchain}"
RAVEN_CROSS_PREFIX="${RAVEN_CROSS_PREFIX:-x86_64-linux-musl}"

# =============================================================================
# The component table. Shared with the GUI stage and, more to the point, with
# stage4's sysroot check -- so that adding a component here is what makes
# stage4 start looking for it. See scripts/lib/components.sh.
# =============================================================================
if [[ -f "${PROJECT_ROOT}/scripts/lib/components.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/components.sh"
else
    echo "FATAL: scripts/lib/components.sh not found; nothing declares what to build" >&2
    exit 1
fi

# =============================================================================
# Logging (use shared library or define fallbacks)
# =============================================================================

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
    log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }
fi

# =============================================================================
# Component Table
# =============================================================================
# RAVEN_COMPONENTS is declared in scripts/lib/components.sh, sourced above, and
# is read here, by stage-gui.sh and by stage4's sysroot check. It was a literal
# in this file until the check grew its own copy and the two drifted: `imlazy`
# was built by this stage and absent from the check for long enough that an ISO
# could ship without it and pass. The table lives in one place now.
#
# Its format, and the reason the graphical components are not in it, are
# documented at the declaration.

# Populated by build_component() so print_summary and the default-shell switch
# can tell what actually landed.
declare -a RAVEN_BUILT=()
declare -a RAVEN_FAILED=()
declare -a RAVEN_SKIPPED=()

# =============================================================================
# Toolchain checks
# =============================================================================

# Returns 0 if Go is usable. Go tools are skipped wholesale when it is not.
have_go() {
    command -v go &>/dev/null
}

# Returns 0 if cargo can produce static musl binaries. Adds the musl target
# via rustup when it is missing; without rustup we can only hope the
# distribution shipped it.
have_rust_musl() {
    command -v cargo &>/dev/null || return 1

    if command -v rustup &>/dev/null; then
        if ! rustup target list --installed 2>/dev/null | grep -qx "${RUST_MUSL_TARGET}"; then
            log_info "Adding Rust target ${RUST_MUSL_TARGET}..."
            rustup target add "${RUST_MUSL_TARGET}" >/dev/null 2>&1 || return 1
        fi
        return 0
    fi

    # No rustup: ask rustc where the target's libdir would be and check that it
    # actually exists. --print target-libdir only *computes* the path, so it
    # succeeds for targets that were never installed -- the directory test is
    # what makes this a real probe.
    local libdir
    libdir="$(rustc --print target-libdir --target "${RUST_MUSL_TARGET}" 2>/dev/null)" || return 1
    [[ -n "${libdir}" && -d "${libdir}" ]]
}

# rustup supplies the musl *std*, but not a musl *C* compiler. Any dependency
# that ships C sources -- ring, the tree-sitter grammars, ... -- builds them
# through cc-rs, which for this target looks for `x86_64-linux-musl-gcc` on
# PATH. Nothing installs that system-wide, so without this every component
# carrying a C dependency dies with
#   error occurred in cc-rs: failed to find tool "x86_64-linux-musl-gcc"
# and build_component() records it as a skip. stage0 already built exactly that
# compiler into ${TOOLCHAIN_DIR}/bin; all that is missing is pointing at it.
#
# Only the compiler is wired up, deliberately. The link stays on rustc's
# default self-contained musl -- that is what already links the components with
# no C dependencies (caw, huginn), and driving the link through the cross gcc
# instead would pull in a second set of crt objects and libc.
setup_cross_cc() {
    local bin="${TOOLCHAIN_DIR}/bin"
    [[ -x "${bin}/${RAVEN_CROSS_PREFIX}-gcc" ]] || return 1

    case ":${PATH}:" in
        *":${bin}:"*) ;;
        *) export PATH="${bin}:${PATH}" ;;
    esac

    # cc-rs reads the target-suffixed form, with dashes turned into underscores.
    local suffix="${RUST_MUSL_TARGET//-/_}"
    export "CC_${suffix}=${RAVEN_CROSS_PREFIX}-gcc"
    export "CXX_${suffix}=${RAVEN_CROSS_PREFIX}-g++"
    export "AR_${suffix}=${RAVEN_CROSS_PREFIX}-ar"
    return 0
}

# Point git at a build-local global config that trusts the component checkouts.
#
# The build runs as root inside the container while build/sources is a bind
# mount owned by the host user, so git refuses every repository there for
# "dubious ownership" and exits 128. That is what turns every fetch into
# "could not update <repo>, using the existing checkout" and pins every
# revision at "unknown".
#
# GIT_CONFIG_GLOBAL is used rather than `git config --global` because
# safe.directory is honoured only in protected (system/global) scope, and this
# way the stage never writes to the invoking user's real ~/.gitconfig. Any
# existing global config is carried over so credentials and proxies still work.
setup_git_trust() {
    command -v git &>/dev/null || return 0

    local cfg="${BUILD_DIR}/raven-gitconfig"
    mkdir -p "${BUILD_DIR}"

    if [[ ! -f "${cfg}" ]]; then
        local existing="${GIT_CONFIG_GLOBAL:-${HOME:-/root}/.gitconfig}"
        [[ -f "${existing}" ]] && cat "${existing}" > "${cfg}" || : > "${cfg}"
        printf '[safe]\n\tdirectory = *\n' >> "${cfg}"
    fi

    export GIT_CONFIG_GLOBAL="${cfg}"
}

# =============================================================================
# Source fetching
# =============================================================================
# Clones a component, or updates an existing clone. Honours RAVEN_OFFLINE and
# the per-component RAVEN_<KEY>_REF pin.
fetch_component() {
    local key="$1" repo="$2"
    local dest="${RAVEN_SRC_DIR}/${repo}"
    local url
    url="$(raven_component_url "${repo}")"

    # RAVEN_IVALDI_REF, RAVEN_CROW_REF, ... override everything. With none set,
    # the pin comes from packages/raven/<key>/package.toml -- the manifest
    # directory is the component key, which is why there is no manifest column
    # in RAVEN_COMPONENTS.
    local ref_var="RAVEN_${key^^}_REF"
    local ref origin
    IFS=$'\t' read -r origin ref < <(raven_resolve_ref "${!ref_var:-}" "raven/${key}")

    raven_fetch_repo "${repo}" "${url}" "${dest}" "${ref}" \
        "${RAVEN_OFFLINE:-0}" "${origin}"
}

# =============================================================================
# Builders
# =============================================================================

# Build a Go component into a fully static binary.
#   CGO_ENABLED=0  no libc linkage at all
#   -trimpath      strips local paths out of the binary
#   -s -w          drops the symbol table and DWARF (these are user tools,
#                  not something we ship debug info for)
#   -buildvcs=false
#                  no VCS stamping. Go shells out to git to stamp the revision,
#                  and a git that refuses the checkout makes the *build* fail
#                  ("error obtaining VCS status: exit status 128"), not just the
#                  stamp. setup_git_trust() fixes the usual cause, but the stamp
#                  is worthless here anyway -- these are pinned checkouts, and
#                  -trimpath already drops build paths.
build_go_component() {
    local src="$1" binaries="$2" targets="$3" outdir="$4"

    # `binaries` and `targets` are positionally paired: the Nth binary is built
    # from the Nth package path. A component with one target and several
    # binaries falls back to that single target for all of them.
    local -a bins targs
    IFS=',' read -r -a bins  <<< "${binaries}"
    IFS=',' read -r -a targs <<< "${targets}"

    local i
    for i in "${!bins[@]}"; do
        (
            cd "${src}"
            CGO_ENABLED=0 \
            GOOS=linux \
            GOARCH="${RAVEN_GOARCH:-amd64}" \
            GOFLAGS="${GOFLAGS:-}" \
            go build -trimpath -buildvcs=false -ldflags "-s -w" \
                -o "${outdir}/${bins[i]}" "${targs[i]:-${targs[0]}}"
        ) || return 1
    done
}

# Build a Rust component into a static musl binary.
# --locked is used only when the component actually ships a Cargo.lock;
# RavenPackageManager and OxigenLang currently do not.
build_rust_component() {
    local src="$1" binaries="$2" targets="$3" outdir="$4"

    local -a cargo_args=(build --release --target "${RUST_MUSL_TARGET}")

    [[ -f "${src}/Cargo.lock" ]] && cargo_args+=(--locked)

    # Every -p goes into a single cargo invocation. Workspace members share a
    # dependency graph and a target directory, so building them together is
    # both faster than one run each and what the committed Cargo.lock was
    # resolved against.
    if [[ "${targets}" != "." ]]; then
        local -a targs
        IFS=',' read -r -a targs <<< "${targets}"
        local t
        for t in "${targs[@]}"; do
            cargo_args+=(-p "${t}")
        done
    fi

    (
        cd "${src}"
        cargo "${cargo_args[@]}" -j "${RAVEN_JOBS}"
    ) || return 1

    local -a bins
    IFS=',' read -r -a bins <<< "${binaries}"
    local b built
    for b in "${bins[@]}"; do
        built="${src}/target/${RUST_MUSL_TARGET}/release/${b}"
        [[ -f "${built}" ]] || return 1
        cp "${built}" "${outdir}/${b}" || return 1
    done
}

# Fetch, build, stage and install one component. Never fails the stage.
build_component() {
    local spec="$1"
    IFS='|' read -r key repo lang binaries targets desc <<< "${spec}"

    local -a bins
    IFS=',' read -r -a bins <<< "${binaries}"

    log_step "${bins[*]} (${desc})"

    if ! fetch_component "${key}" "${repo}"; then
        log_warn "  ${bins[*]}: source unavailable, skipping"
        RAVEN_FAILED+=("${bins[@]}")
        return 0
    fi

    local src="${RAVEN_SRC_DIR}/${repo}"
    mkdir -p "${RAVEN_STAGE_DIR}"

    local ok=0
    case "${lang}" in
        go)
            build_go_component "${src}" "${binaries}" "${targets}" "${RAVEN_STAGE_DIR}" || ok=1
            ;;
        rust)
            build_rust_component "${src}" "${binaries}" "${targets}" "${RAVEN_STAGE_DIR}" || ok=1
            ;;
        *)
            log_error "  ${bins[*]}: unknown language '${lang}'"
            ok=1
            ;;
    esac

    # A component is all or nothing. caw without cawd is a CLI with no daemon
    # to talk to, so a partial build installs nothing rather than something
    # that looks present and does not work.
    local b
    if (( ok == 0 )); then
        for b in "${bins[@]}"; do
            [[ -f "${RAVEN_STAGE_DIR}/${b}" ]] || ok=1
        done
    fi

    if (( ok != 0 )); then
        log_warn "  ${bins[*]}: build failed, skipping"
        RAVEN_FAILED+=("${bins[@]}")
        return 0
    fi

    for b in "${bins[@]}"; do
        install_component_binary "${b}" "${RAVEN_STAGE_DIR}/${b}"
        RAVEN_BUILT+=("${b}")
        log_success "  ${b} installed ($(du -h "${RAVEN_STAGE_DIR}/${b}" | cut -f1))"
    done
}

# Install a staged binary into the sysroot.
#
# /usr/bin is the only destination. /bin is a symlink onto it (see
# scripts/lib/usrmerge.sh), so the binary is reachable at both names with no
# link of our own. The compat link this used to create --
#   ln -sf "../usr/bin/${binary}" "${SYSROOT_DIR}/bin/${binary}"
# -- resolved to /usr/usr/bin/${binary} post-merge, and because ln -sf unlinks
# its target first it DELETED the binary it had just installed. Silently, exit
# 0, for every Raven-layer component: ravenshell, rvn, caw, cawd, crow, ivaldi,
# oxigen, poxy.
install_component_binary() {
    local binary="$1" src="$2"

    mkdir -p "${SYSROOT_DIR}/usr/bin"
    install -m 0755 "${src}" "${SYSROOT_DIR}/usr/bin/${binary}"
}

# =============================================================================
# Packaging
# =============================================================================
# The step that makes this a distribution rather than an image.
#
# Everything above installs a file: `install -m 0755 <built> <sysroot>/usr/bin`.
# That produces a working image and nothing else -- no version, no file list,
# no integrity record, no uninstall and, most of all, no upgrade. `rvn owns
# /usr/bin/crow` on a machine booted from that image answers "no package owns
# this file", because none does, and `rvn update` has nothing to compare a new
# crow against. Every program this distribution wrote for itself is in that
# position: the image is the only thing that knows they exist.
#
# So each component is built into a real package with `rvn build`, and the
# package -- not the loose binary -- is what the sysroot is populated from.
# The archive carries the manifest's metadata and a .MTREE, and installing it
# writes a record under /var/lib/pacman/local, which is what `rvn owns`,
# `rvn list`, `rvn uninstall` and every upgrade read.
#
# WHY `rvn build --no-build`
#
# The stage has already compiled the component with its own toolchain, its own
# target directory and its own flags. Letting rvn run [build] again would at
# best duplicate a cargo build and at worst use different flags than the ones
# this stage documents. With --no-build, rvn stages exactly the files the
# manifest's [install] table names and packages those -- so packages/ stays
# the single description of what a component installs, and this file stays the
# single description of how it is compiled. See the interface note at the top
# of RavenPackageManager/src/ops/build.rs.
#
# WHY NOT `rvn install` TO PUT THE PACKAGES IN
#
# It is the obvious answer, and the reasons it is not taken have changed since
# they were first written down. Two of the three are now simply false, and a
# comment that justifies seven hundred lines of hand-written alpm code with a
# limitation the package manager no longer has is worse than no comment: it
# stops the next reader from trying the path that works. So this says what was
# checked, against which code, rather than what was once true.
#
# TWO OF THE THREE ORIGINAL BLOCKERS NO LONGER EXIST.
#
#   - file:// works. RavenPackageManager/src/fetch.rs has is_file_url,
#     file_path and a copy branch inside stream_to, so mirror failover, the
#     .part-then-rename and the progress accounting all apply to a local
#     directory too. A directory `rvn repo-add` has written IS a repository,
#     with no web server in front of it -- which is what the [raven] block in
#     configs/rvn/pacman.conf has been telling administrators all along. The
#     two files disagreed, in the same change, and this one was wrong.
#   - the conflict pre-flight does not refuse an unowned file. extract.rs's
#     find_conflicts builds its owner map out of the LOCAL DATABASE
#     (owned_by_others), so a regular file in /usr/bin that no package claims
#     is not a conflict at all -- it is overwritten. What it does refuse is a
#     file another package owns, and an entry whose TYPE disagrees with what
#     is on disk (Arch's `filesystem` shipping /bin as a symlink onto a real
#     directory is the case it was written for). Neither describes this
#     sysroot's /usr/bin.
#
# AND THE THIRD NEVER APPLIED TO THESE TEN. `rvn install` does refuse a
# transaction with an unresolvable dependency, and there are manifests under
# packages/ that name programs stage2 copied off the build container rather
# than packaged. None of them is one this stage builds: every manifest
# package_built_components reaches declares `runtime = []`. Checked, one file
# at a time, not assumed.
#
# WHAT ACTUALLY STILL STANDS is one thing, and it is not a limitation of rvn:
#
#   - .pacnew in the image. extract.rs's unpack writes `<file>.pacnew` beside
#     any file the package lists as `backup` whose bytes on disk differ. It
#     has no notion of "there was no previous version", so a first install
#     over a file somebody else wrote behaves exactly like an upgrade. stage2
#     hand-writes /etc/raven/init.toml, power.toml and time.toml long before
#     any package exists, so `rvn install` would put three .pacnew files
#     inside the ISO. A .pacnew means "your edit was kept, here is the new
#     default"; on a machine that has never been booted it is a note from
#     nobody about an edit nobody made. The loop below exists to get that one
#     case right, and it is the piece a straight `rvn install` cannot do.
#
# The rest is work rather than obstacle, and this is what it would take, so
# that the next person reads a plan instead of an excuse: a build-time
# pacman.conf whose RootDir, DBPath and CacheDir point into the sysroot (rvn
# takes no --root -- RootDir in the configuration file is the only way, see
# RavenPackageManager/src/config.rs), a [raven] section whose Server is
# file://${RAVEN_PKG_DIR}, --no-sync so an ISO build never reaches for Arch's
# mirrors, and a SigLevel the build can actually satisfy: the shipped policy
# is `Required` and an ordinary build signs nothing. That is a deliberate
# change to how the image is assembled, and it is not made in passing here.
#
# So the packages are installed here instead: the payload is extracted over
# the sysroot and the local database record is written from the package's own
# .PKGINFO and .MTREE. That is a deliberately small amount of code doing what
# `pacman -U --dbonly` plus an extraction would do, and every field it writes
# is one rvn's db::local reads back.
#
# WHAT HAPPENS TO THE FILES THAT ARE ALREADY THERE
#
# /usr/bin already holds the binaries this stage installed minutes ago, and in
# some cases copies from an earlier run. The package's copies OVERWRITE them,
# because they are byte-for-byte the same files -- `rvn build --no-build`
# packaged the very tree the loose install copied from -- and because no
# package owns what is there, so there is nothing to conflict with. Emptying
# /usr/bin first was the alternative and it is much worse: that directory also
# holds the base layer stage2 copied in, and clearing it to make room for a
# package would take out the system the package needs.
#
# /etc is the exception and is handled the other way round: a file the package
# declares as `backup` is a file an administrator edits, and stage2 writes
# several of them by hand long before any package existed. Those are KEPT, and
# the record stores the checksum of the package's copy -- so the file reads as
# locally modified, which is precisely what it is, and the first upgrade after
# the image is installed leaves a .pacnew rather than overwriting somebody's
# init.toml. Nothing ships a .pacnew into the image itself.
#
# FAIL-SOFT, exactly like the build above it. A component that cannot be
# packaged keeps the files that were already installed for it and is listed at
# the end of the stage. A packaging failure never fails the stage and never
# fails the ISO: a half-packaged image that boots beats a fully-packaged one
# that does not.

# Where the packages this stage builds are written, and where `rvn repo-add`
# builds their database. A build output directory and not part of the image:
# the ISO ships the installed files, and a second compressed copy of every
# Raven binary inside it would grow the image by the size of the whole layer
# to no end. Publishing that directory is what turns a build into a repository.
RAVEN_PKG_DIR="${RAVEN_PKG_DIR:-${PACKAGES_DIR}/raven-repo}"

# Packaging is noisy and most of it is only interesting when it fails, so it
# goes to a log of its own and the stage prints the one-line outcome.
RAVEN_PKG_LOG="${RAVEN_PKG_LOG:-${LOGS_DIR}/raven-packaging.log}"

# Answered once by find_package_builder, and empty until it has run.
RAVEN_PKG_RVN=""
RAVEN_PKG_CONF=""
RAVEN_PKG_HAS_PKGVER=0

# Set by package_component so its caller does not have to read a path off
# stdout -- every function here logs to stdout, and a command substitution
# around one would swallow the log with the answer.
RAVEN_PKG_ARCHIVE=""

# Components whose package was built and recorded, and components left as
# loose files. Reported by print_summary. Deliberately NOT RAVEN_FAILED: that
# list means "this software is not in the image", and a component that built
# and installed but could not be packaged is in the image.
declare -a RAVEN_PACKAGED=()
declare -a RAVEN_UNPACKAGED=()

# Find an rvn that can build packages.
#
# THE CHICKEN AND EGG, stated plainly: rvn is one of the components this stage
# packages, so the tool doing the packaging is also one of the things being
# packaged. There is no way to avoid that, only ways to handle it, and this is
# the one chosen -- package everything at the end of the stage, once rvn has
# been built, rather than as each component finishes. The rvn that does the
# work is then the rvn this build just produced, which is the honest answer:
# the packages in the image were made by the package manager in the image.
#
# Candidates, best first:
#   RAVEN_PACKAGING_RVN   an explicit path, for bisecting a packaging bug
#   ${RAVEN_STAGE_DIR}/rvn   what this run just built, statically linked and
#                            therefore runnable on the build host
#   ${SYSROOT_DIR}/usr/bin/rvn   what an earlier run installed, for a rerun
#                                with RAVEN_SKIP=rvn
#   rvn on PATH           a build host that already has one
#
# Every candidate is PROBED for the subcommand rather than assumed to have it.
# `rvn build` is recent: an rvn from before it, or from the pinned commit in
# packages/raven/rvn/package.toml, is a perfectly good package manager that
# cannot build a package, and `rvn build --help` is the only reliable way to
# tell the two apart.
find_package_builder() {
    [[ -n "${RAVEN_PKG_RVN}" ]] && return 0

    # rvn loads a pacman.conf before it does anything at all, including
    # building -- `rvn build` gets the same Context every other subcommand
    # does. The build host has one; a host that does not gets the canonical
    # file out of this tree rather than failing before the first package.
    if [[ -f /etc/pacman.conf ]]; then
        RAVEN_PKG_CONF="/etc/pacman.conf"
    elif [[ -f "${PROJECT_ROOT}/configs/rvn/pacman.conf" ]]; then
        RAVEN_PKG_CONF="${PROJECT_ROOT}/configs/rvn/pacman.conf"
    else
        return 1
    fi

    local candidate host_rvn
    host_rvn="$(command -v rvn 2>/dev/null || true)"

    for candidate in "${RAVEN_PACKAGING_RVN:-}" \
                     "${RAVEN_STAGE_DIR}/rvn" \
                     "${SYSROOT_DIR}/usr/bin/rvn" \
                     "${host_rvn}"; do
        [[ -n "${candidate}" && -x "${candidate}" ]] || continue

        if ! "${candidate}" build --help >/dev/null 2>&1; then
            log_info "  ${candidate} has no 'build' subcommand, trying the next rvn"
            continue
        fi

        RAVEN_PKG_RVN="${candidate}"

        # --pkgver does not exist yet. scripts/lib/components.sh composes a
        # build-honest version for every component, and there is currently no
        # way to hand it to `rvn build`, which reads the version only from
        # [package].version. Writing a mutated copy of the manifest per
        # component would be the workaround and it is worse than the problem:
        # it makes packages/ stop describing what was built. So the flag is
        # probed for, the composed version is used the moment somebody adds
        # it, and until then this says once, out loud, what the packages are
        # going to call themselves.
        if "${candidate}" build --help 2>/dev/null | grep -q -- '--pkgver'; then
            RAVEN_PKG_HAS_PKGVER=1
        else
            log_warn "  this rvn has no --pkgver: packages carry their manifest version"
            log_warn "  two builds a month apart will produce two packages with one version"
        fi

        return 0
    done

    return 1
}

# tar, configured for the archives rvn writes.
#
# GNU tar and zstd are both installed by the build container (Dockerfile:58),
# which bsdtar is not -- libarchive is on the container because pacman depends
# on it, not because anything asked for it, and a build step should not rest
# on that. The format is plain tar inside a zstd stream either way.
raven_pkg_tar() {
    tar --use-compress-program=zstd "$@"
}

# The first value of a .PKGINFO key, or nothing. Values are `key = value`, and
# a key may repeat, which is why reading one and reading all of them are two
# functions rather than one with a flag.
raven_pkginfo_field() {
    local file="$1" key="$2"

    awk -v key="${key}" '
        /^[[:space:]]*#/ { next }
        {
            eq = index($0, " = ")
            if (eq == 0) next
            if (substr($0, 1, eq - 1) != key) next
            print substr($0, eq + 3)
            exit 0
        }
    ' "${file}"
}

# Every value of a .PKGINFO key, in the order the file lists them. Order is
# not cosmetic: %DEPENDS% and %BACKUP% are read back as lists, and reordering
# them would make two records of the same package differ for no reason.
raven_pkginfo_list() {
    local file="$1" key="$2"

    awk -v key="${key}" '
        /^[[:space:]]*#/ { next }
        {
            eq = index($0, " = ")
            if (eq == 0) next
            if (substr($0, 1, eq - 1) != key) next
            print substr($0, eq + 3)
        }
    ' "${file}"
}

# One `%KEY%` block of a desc record: the header, a line per value, a blank
# line. A key with no values is omitted entirely rather than written empty,
# because the parser treats a present key as an answer.
raven_desc_field() {
    local file="$1" key="$2"
    shift 2

    (( $# > 0 )) || return 0

    {
        printf '%%%s%%\n' "${key}"
        printf '%s\n' "$@"
        printf '\n'
    } >> "${file}"
}

# Removes any existing local-database record for a package.
#
# The record directory is named `<name>-<version>`, so a rebuild at a new
# version would otherwise leave the old record beside the new one and make the
# next database load ambiguous about which is installed. The name is read out
# of each record rather than matched with a `<name>-*` glob, because package
# names contain hyphens: `raven-init-0.1.0-1` and a hypothetical
# `raven-init-extras-0.1.0-1` both match the glob and only one of them is this
# package.
sysroot_forget_package() {
    local name="$1"
    local localdb="${SYSROOT_DIR}/var/lib/pacman/local"
    local dir existing

    [[ -d "${localdb}" ]] || return 0

    for dir in "${localdb}"/*/; do
        [[ -f "${dir}desc" ]] || continue
        existing="$(awk '/^%NAME%$/ { getline; print; exit }' "${dir}desc")"
        if [[ "${existing}" == "${name}" ]]; then
            rm -rf "${dir}"
        fi
    done
}

# Install one built package into the sysroot: extract the payload, then write
# the record that makes the image's package manager aware of it.
#
# The work directory is removed whether or not this succeeds, which is why the
# body is a second function -- every step in it can fail and each one wants to
# say `return 1` rather than remember the cleanup.
sysroot_install_package() {
    local archive="$1" label="$2"
    local work rc=0

    work="$(mktemp -d "${BUILD_DIR}/raven-pkg-XXXXXX")" || return 1
    sysroot_install_package_body "${archive}" "${label}" "${work}" || rc=1
    rm -rf "${work}"

    return ${rc}
}

sysroot_install_package_body() {
    local archive="$1" label="$2" work="$3"

    # .PKGINFO is the metadata the record is written from. .MTREE is the
    # integrity record `rvn` and `pacman -Qkk` check a file against; it is
    # stored beside the record verbatim rather than regenerated, because a
    # regenerated one would describe the sysroot instead of the package and
    # would agree with itself no matter what had happened to the files.
    if ! raven_pkg_tar -xf "${archive}" -C "${work}" .PKGINFO .MTREE \
            >> "${RAVEN_PKG_LOG}" 2>&1; then
        log_warn "  ${label}: the package has no .PKGINFO/.MTREE pair"
        return 1
    fi

    local name version
    name="$(raven_pkginfo_field "${work}/.PKGINFO" pkgname)"
    version="$(raven_pkginfo_field "${work}/.PKGINFO" pkgver)"
    if [[ -z "${name}" || -z "${version}" ]]; then
        log_warn "  ${label}: the package names no pkgname/pkgver"
        return 1
    fi

    # The payload, which is every member that is not one of alpm's metadata
    # files. rvn writes entry names without a leading ./, and directories with
    # a trailing slash, which is exactly the spelling a %FILES% record uses.
    local -a members=()
    mapfile -t members < <(raven_pkg_tar -tf "${archive}" \
        | grep -v -e '^\.PKGINFO$' -e '^\.MTREE$' -e '^\.BUILDINFO$' \
                  -e '^\.INSTALL$' -e '^\.CHANGELOG$')
    if (( ${#members[@]} == 0 )); then
        log_warn "  ${label}: the package is empty"
        return 1
    fi

    # Configuration files the package says an administrator owns. Each one
    # that is already in the sysroot is copied aside before the extraction and
    # put back after it -- see the note on /etc at the top of this section.
    local -a backups=()
    mapfile -t backups < <(raven_pkginfo_list "${work}/.PKGINFO" backup)

    #
    # The copy goes into the WORK DIRECTORY and not next to the original.
    # Every exit from this function that is not the final `return 0` is a
    # failure, package_and_install fails soft on all of them by design, and
    # the stage carries on to build an ISO -- so a copy left beside the file
    # it was taken from ships. An image whose /etc/raven holds
    # init.toml.raven-stage-keep, power.toml.raven-stage-keep and
    # time.toml.raven-stage-keep hands an administrator three orphaned
    # backups of unknown provenance in the first directory they read, and
    # nothing in stage4 or check-desktop-image.py looks for them.
    # sysroot_install_package removes the work directory whether the body
    # succeeded or not, which is exactly the guarantee this needs.
    local keepdir="${work}/keep"
    local rel abs kept
    for rel in "${backups[@]}"; do
        abs="${SYSROOT_DIR}/${rel#/}"
        if [[ -f "${abs}" ]]; then
            kept="${keepdir}/${rel#/}"
            mkdir -p "$(dirname "${kept}")" || return 1
            cp -a "${abs}" "${kept}" || return 1
        fi
    done

    # -p to keep the modes the manifest asked for, and no --no-same-owner: the
    # archive says root/root for every entry, which is what the image wants
    # and what a build running as root in the container produces.
    if ! raven_pkg_tar -xpf "${archive}" -C "${SYSROOT_DIR}" \
            --exclude '.PKGINFO' --exclude '.MTREE' --exclude '.BUILDINFO' \
            --exclude '.INSTALL' --exclude '.CHANGELOG' \
            >> "${RAVEN_PKG_LOG}" 2>&1; then
        log_warn "  ${label}: could not extract the package into the sysroot"

        # tar writes as it goes, so a failure part way through may already
        # have put the package's copy over a file the administrator owns --
        # and the copies taken above are about to be removed with the work
        # directory. Put them back first. This is the only path between the
        # aside-copy and the restore loop below that gives up, so one loop
        # here is the whole of it; without it the image would keep whatever
        # the failed extraction happened to write over /etc/raven/init.toml.
        for rel in "${backups[@]}"; do
            kept="${keepdir}/${rel#/}"
            if [[ -f "${kept}" ]]; then
                mv -f "${kept}" "${SYSROOT_DIR}/${rel#/}" || true
            fi
        done
        return 1
    fi

    # The checksum is taken here, between the extraction and the restore,
    # because it has to describe the file the PACKAGE ships. That is what
    # makes a later "has this been edited?" answerable: rvn compares the file
    # on disk against this hash, and a hash of the file already on disk would
    # call every file untouched forever.
    #
    # sha256, not the md5 pacman writes in this field, because rvn is the
    # package manager that reads it -- crate::verify::sha256_file is what
    # ops::remove and `rvn config` hash these files with.
    local -a backup_entries=()
    local shipped
    for rel in "${backups[@]}"; do
        abs="${SYSROOT_DIR}/${rel#/}"
        shipped=""
        if [[ -f "${abs}" ]]; then
            shipped="$(sha256sum "${abs}" | cut -d' ' -f1)"
        fi

        kept="${keepdir}/${rel#/}"
        if [[ -f "${kept}" ]]; then
            if cmp -s "${kept}" "${abs}"; then
                # The stage wrote the same bytes the package ships, which is
                # the usual case: both come out of this repository.
                rm -f "${kept}"
            else
                log_warn "  ${label}: keeping the image's ${rel} (the package ships a different one)"
                mv -f "${kept}" "${abs}" || return 1
            fi
        fi

        backup_entries+=("$(printf '%s\t%s' "${rel#/}" "${shipped}")")
    done

    sysroot_forget_package "${name}"

    local dbdir="${SYSROOT_DIR}/var/lib/pacman/local/${name}-${version}"
    mkdir -p "${dbdir}" || return 1

    local desc_file="${dbdir}/desc"
    : > "${desc_file}"

    local -a values=()
    raven_desc_field "${desc_file}" NAME "${name}"
    raven_desc_field "${desc_file}" VERSION "${version}"

    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" pkgbase)
    raven_desc_field "${desc_file}" BASE "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" pkgdesc)
    raven_desc_field "${desc_file}" DESC "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" url)
    raven_desc_field "${desc_file}" URL "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" arch)
    raven_desc_field "${desc_file}" ARCH "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" license)
    raven_desc_field "${desc_file}" LICENSE "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" group)
    raven_desc_field "${desc_file}" GROUPS "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" builddate)
    raven_desc_field "${desc_file}" BUILDDATE "${values[@]}"

    # When the image was made, which is the only honest answer available: the
    # machine that will read this record has not been booted yet.
    raven_desc_field "${desc_file}" INSTALLDATE "$(date -u +%s)"

    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" packager)
    raven_desc_field "${desc_file}" PACKAGER "${values[@]}"

    # The local database calls the installed size SIZE where a sync database
    # calls it ISIZE. Writing the sync spelling here makes every tool report
    # a size of zero.
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" size)
    raven_desc_field "${desc_file}" SIZE "${values[@]}"

    # 0 is "explicitly installed". These packages are the distribution, not
    # something dragged in behind it: recorded as dependencies they would be
    # orphans the moment nothing referenced them, and `rvn update --orphans`
    # would offer to remove the shell the machine is running.
    raven_desc_field "${desc_file}" REASON 0

    # Built here, from this tree, and not signed by anybody at build time --
    # so neither a checksum nor a signature was verified on the way in, and
    # the record says so rather than claiming a check that never happened.
    raven_desc_field "${desc_file}" VALIDATION none
    raven_desc_field "${desc_file}" XDATA "pkgtype=pkg"
    raven_desc_field "${desc_file}" BACKUP "${backup_entries[@]}"

    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" provides)
    raven_desc_field "${desc_file}" PROVIDES "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" depend)
    raven_desc_field "${desc_file}" DEPENDS "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" optdepend)
    raven_desc_field "${desc_file}" OPTDEPENDS "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" conflict)
    raven_desc_field "${desc_file}" CONFLICTS "${values[@]}"
    mapfile -t values < <(raven_pkginfo_list "${work}/.PKGINFO" replaces)
    raven_desc_field "${desc_file}" REPLACES "${values[@]}"

    {
        printf '%%FILES%%\n'
        printf '%s\n' "${members[@]}"
    } > "${dbdir}/files"

    cp "${work}/.MTREE" "${dbdir}/mtree" || return 1

    return 0
}

# Build one component's package. Sets RAVEN_PKG_ARCHIVE on success.
#
#   package_component <manifest-under-packages> <srcdir> <checkout> <label>
#
# `srcdir` is what the manifest's [install] src paths are relative to and
# `checkout` is the git clone the version is derived from. They are usually
# the same directory and for a Go component they are not -- see the caller.
package_component() {
    local manifest_rel="$1" srcdir="$2" checkout="$3" label="$4"
    local manifest="${PROJECT_ROOT}/packages/${manifest_rel}/package.toml"

    RAVEN_PKG_ARCHIVE=""

    if [[ ! -f "${manifest}" ]]; then
        log_warn "  ${label}: no manifest at packages/${manifest_rel}"
        return 1
    fi
    if [[ ! -d "${srcdir}" ]]; then
        log_warn "  ${label}: no built tree at ${srcdir}"
        return 1
    fi

    # A directory of its own, emptied first, so the archive can be found
    # without reconstructing its filename. The name is the manifest's rather
    # than the component's (packages/raven/rvn is `ravenpackagemanager`), the
    # version may be composed, and the architecture comes from [build] -- three
    # chances to guess wrong about a file that is simply the only one there.
    local outdir="${RAVEN_PKG_DIR}/.staging/${label}"
    rm -rf "${outdir}"
    mkdir -p "${outdir}"

    # --no-sync because building a package needs no repository database and
    # an image build may have no network at all (RAVEN_OFFLINE=1 says so out
    # loud); refreshing one here would be a download nothing reads.
    local -a args=(
        build "${manifest}"
        --config "${RAVEN_PKG_CONF}"
        --no-sync
        --no-build
        --srcdir "${srcdir}"
        --outdir "${outdir}"
    )

    # Nothing forces --no-sign: an /etc/rvn/build.toml naming a key is an
    # administrator saying packages from this machine are signed, and a build
    # host that has one means it. With no such file rvn produces unsigned
    # packages and says so, which is what every build does today.

    if (( RAVEN_PKG_HAS_PKGVER == 1 )) && declare -F raven_component_version >/dev/null; then
        local version=""
        if version="$(raven_component_version "${manifest_rel}" "${checkout}")" \
               && [[ -n "${version}" ]]; then
            args+=(--pkgver "${version}")
        else
            # Deliberately not a fall back to the manifest version: that is
            # the collision the Versions section of components.sh exists to
            # prevent, and an unpackaged component is a smaller problem than
            # two different packages calling themselves 0.1.0-1.
            log_warn "  ${label}: no build revision for ${checkout}, not packaging"
            return 1
        fi
    fi

    echo "=== ${label}: rvn build $(date -u +%FT%TZ)" >> "${RAVEN_PKG_LOG}"
    if ! "${RAVEN_PKG_RVN}" "${args[@]}" >> "${RAVEN_PKG_LOG}" 2>&1; then
        log_warn "  ${label}: rvn build failed (see ${RAVEN_PKG_LOG})"
        return 1
    fi

    local built
    built="$(find "${outdir}" -maxdepth 1 -name '*.pkg.tar.zst' -print -quit)"
    if [[ -z "${built}" ]]; then
        log_warn "  ${label}: rvn build produced no archive"
        return 1
    fi

    # Out of the staging directory and into the one `rvn repo-add` reads. The
    # signature moves with it or the database would advertise a signature the
    # repository does not have.
    #
    # The destination's OLD signature is removed first, and unconditionally,
    # before the archive is replaced. Package filenames here are constant
    # across builds -- there is no --pkgver, so a rebuild from a newer commit
    # produces the same name -- and `mv -f` therefore overwrites the archive
    # in place. A .sig written by a build made while this host had a signing
    # key, on a host that no longer has one, would otherwise survive beside
    # bytes it does not cover: `rvn repo-add` reads the .sig next to each
    # package and copies it into %PGPSIG% (RavenPackageManager/src/repodb.rs),
    # so the published database would advertise a signature over an archive
    # that no longer exists. Under the [raven] policy this image ships,
    # `SigLevel = Required`, every install of that package then fails
    # verification -- while this build reported success. Removing the stale
    # one costs nothing in the ordinary case, where there was none.
    local dest="${RAVEN_PKG_DIR}/$(basename "${built}")"
    rm -f "${dest}.sig"
    mv -f "${built}" "${dest}" || return 1
    if [[ -f "${built}.sig" ]]; then
        mv -f "${built}.sig" "${dest}.sig" || return 1
    fi
    rm -rf "${outdir}"

    RAVEN_PKG_ARCHIVE="${dest}"
    return 0
}

# Package one component and install the result. Never fails the stage: a
# component that cannot be packaged keeps the files install_component_binary
# already put in the sysroot, and is named at the end of the run.
package_and_install() {
    local manifest_rel="$1" srcdir="$2" checkout="$3" label="$4" binaries="$5"

    if ! package_component "${manifest_rel}" "${srcdir}" "${checkout}" "${label}"; then
        RAVEN_UNPACKAGED+=("${label}")
        return 0
    fi

    if ! sysroot_install_package "${RAVEN_PKG_ARCHIVE}" "${label}"; then
        RAVEN_UNPACKAGED+=("${label}")
        return 0
    fi

    # The version the record actually carries, read back out of the package
    # rather than the one that was asked for: until `rvn build` takes
    # --pkgver those two are different, and a provenance record that names a
    # version no package has is worse than none.
    if declare -F raven_record_version >/dev/null; then
        local recorded
        recorded="$(raven_pkg_tar -xOf "${RAVEN_PKG_ARCHIVE}" .PKGINFO 2>/dev/null \
            | awk '{ eq = index($0, " = "); if (eq && substr($0, 1, eq - 1) == "pkgver") { print substr($0, eq + 3); exit } }')"
        if [[ -n "${recorded}" ]]; then
            raven_record_version "${label}" "${recorded}" || true
        fi
    fi

    RAVEN_PACKAGED+=("${label}")
    log_success "  ${label} packaged: $(basename "${RAVEN_PKG_ARCHIVE}")"
    return 0
}

# True when every binary a component produces landed in this run. A component
# is packaged all or nothing for the same reason it is built all or nothing:
# a package claiming to own cawd when cawd was never built is a package whose
# file list is a lie, and `rvn owns` would answer with it.
raven_component_is_built() {
    local binaries="$1"
    local -a bins=()
    raven_split_list bins "${binaries}"

    local b built found
    for b in "${bins[@]}"; do
        found=0
        for built in ${RAVEN_BUILT[@]+"${RAVEN_BUILT[@]}"}; do
            if [[ "${built}" == "${b}" ]]; then
                found=1
                break
            fi
        done
        (( found == 1 )) || return 1
    done

    return 0
}

# The repository database, built from the packages this stage produced.
#
# It is the other half of "a distribution": a directory of packages with a
# <repo>.db in it is something a machine can be pointed at, which is what
# turns the next build of these components into an upgrade rather than a new
# ISO. It is written beside the packages in the build output and served from
# nowhere -- publishing it is a decision for whoever runs the build.
#
# Serving it needs no server. rvn reads file:// URLs (fetch.rs), so
# RAVEN_REPO_SERVER=file://<this directory> on the next build points the image
# straight at what this one produced. An earlier version of this comment said
# the opposite; it was wrong, and configs/rvn/pacman.conf was right.
build_raven_repo_db() {
    (( ${#RAVEN_PACKAGED[@]} > 0 )) || return 0

    echo "=== repo-add $(date -u +%FT%TZ)" >> "${RAVEN_PKG_LOG}"
    if "${RAVEN_PKG_RVN}" repo-add "${RAVEN_PKG_DIR}" --name raven \
            --config "${RAVEN_PKG_CONF}" --no-sync >> "${RAVEN_PKG_LOG}" 2>&1; then
        log_success "  raven.db written in ${RAVEN_PKG_DIR}"
    else
        # Not fatal and not silent: every package is still installed and
        # recorded, and what is missing is the index over them.
        log_warn "  could not build raven.db (see ${RAVEN_PKG_LOG})"
    fi
}

# Package everything this run built, after the build rather than during it --
# see find_package_builder for why that ordering is forced.
package_built_components() {
    if [[ "${RAVEN_SKIP_PACKAGING:-0}" == "1" ]]; then
        log_info "RAVEN_SKIP_PACKAGING=1, leaving this stage's output as loose files"
        return 0
    fi

    (( ${#RAVEN_BUILT[@]} > 0 )) || return 0

    if ! find_package_builder; then
        log_warn "No rvn that can build packages, and no pacman.conf to give one"
        log_warn "  the image is what it has always been: the files are installed"
        log_warn "  and no package owns them. Set RAVEN_PACKAGING_RVN=<path> to an"
        log_warn "  rvn with a 'build' subcommand to package this layer."
        # Binaries here where the loop below records components, because
        # nothing got far enough to have a component name attached to it.
        RAVEN_UNPACKAGED+=("${RAVEN_BUILT[@]}")
        return 0
    fi

    log_step "Packaging the Raven layer with ${RAVEN_PKG_RVN}"
    mkdir -p "${RAVEN_PKG_DIR}" "${LOGS_DIR}"
    : > "${RAVEN_PKG_LOG}"

    local spec key repo lang binaries targets desc srcdir
    for spec in "${RAVEN_COMPONENTS[@]}"; do
        IFS='|' read -r key repo lang binaries targets desc <<< "${spec}"

        raven_component_is_built "${binaries}" || continue

        # Where the built files are, which is not the same place for both
        # languages. build_rust_component leaves its binaries in the
        # checkout's target/ directory and the Rust manifests name that path;
        # build_go_component builds straight into the stage directory with
        # `go build -o`, and the Go manifests name a bare filename, which is
        # what a plain `rvn build` would produce in the checkout root. The
        # stage directory is where that bare name resolves here.
        case "${lang}" in
            go) srcdir="${RAVEN_STAGE_DIR}" ;;
            *)  srcdir="${RAVEN_SRC_DIR}/${repo}" ;;
        esac

        package_and_install "raven/${key}" "${srcdir}" \
            "${RAVEN_SRC_DIR}/${repo}" "${repo}" "${binaries}"
    done

    # The two crates that live in this repository rather than in one of their
    # own. Their manifests' src paths are repository-relative, so the srcdir
    # is the checkout you are reading, and there is no component clone to
    # take a revision from -- raven_component_version answers with
    # RAVEN_VERSION for exactly these.
    if raven_component_is_built "${RAVEN_INIT_BINARIES}"; then
        package_and_install "raven/raven-init" "${PROJECT_ROOT}" "" \
            "raven-init" "${RAVEN_INIT_BINARIES}"
    fi
    if raven_component_is_built "${RAVEN_FACED_BINARIES}"; then
        package_and_install "raven/raven-faced" "${PROJECT_ROOT}" "" \
            "raven-faced" "${RAVEN_FACED_BINARIES}"
    fi

    build_raven_repo_db
}

# =============================================================================
# Component selection
# =============================================================================
# RAVEN_ONLY wins over RAVEN_SKIP. Both take comma-separated component keys.
component_selected() {
    local key="$1"

    if [[ -n "${RAVEN_ONLY:-}" ]]; then
        [[ ",${RAVEN_ONLY}," == *",${key},"* ]] && return 0
        return 1
    fi

    if [[ -n "${RAVEN_SKIP:-}" ]]; then
        [[ ",${RAVEN_SKIP}," == *",${key},"* ]] && return 1
    fi

    return 0
}

# =============================================================================
# raven-init (local crate, not a fetched component)
# =============================================================================
# The distro's PID 1 and its control tool live in this repo, not on GitHub, so
# they sit outside RAVEN_COMPONENTS -- and consequently nothing built them.
# raven-rc is what `systemctl` is not: the thing that actually talks to
# RavenLinux's init.
#
# raven-rc dispatches on argv[0], so poweroff/reboot/halt/shutdown are symlinks
# to it rather than four separate binaries.
#
# raven-powerd comes out of the same crate: it is what makes the power button
# and the lid mean anything, and it asks init for the suspend over raven-rc's
# control socket rather than writing /sys/power/state itself.
build_raven_init() {
    local old_init
    for old_init in ${RAVEN_INIT_BINARIES//,/ }; do
        rm -f "${SYSROOT_DIR}/usr/bin/${old_init}"
    done

    local src="${PROJECT_ROOT}/init"

    [[ -f "${src}/Cargo.toml" ]] || {
        log_warn "No init crate at ${src}, skipping raven-init"
        return 0
    }

    log_step "raven-init raven-rc (RavenLinux init and its control tool)"

    local outdir="${RAVEN_STAGE_DIR}"
    mkdir -p "${outdir}"

    # The binary list comes from RAVEN_INIT_BINARIES so that what is built here
    # and what stage4 looks for are the same string. raven-ports was in this
    # call and missing from that check for exactly as long as the two were
    # written out separately.
    local -a init_bins
    raven_split_list init_bins "${RAVEN_INIT_BINARIES}"

    if ! build_rust_component "${src}" "${RAVEN_INIT_BINARIES}" "." "${outdir}"; then
        log_warn "  raven-init: build failed, skipping"
        RAVEN_FAILED+=("${init_bins[@]}")
        return 0
    fi

    # Both land in /usr/bin. /sbin/raven-init and /bin/raven-rc still resolve
    # there -- init.toml, stage4's PID 1 check and the installer all name the
    # /sbin path, and the merge link keeps every one of them working.
    mkdir -p "${SYSROOT_DIR}/usr/bin"
    install -m 0755 "${outdir}/raven-init" "${SYSROOT_DIR}/usr/bin/raven-init"
    install -m 0755 "${outdir}/raven-rc"   "${SYSROOT_DIR}/usr/bin/raven-rc"
    # The lid and power-button daemon. init.toml starts it as the `powerd`
    # service; it is a third binary out of the same crate because it is the
    # other half of init's suspend.
    install -m 0755 "${outdir}/raven-powerd" "${SYSROOT_DIR}/usr/bin/raven-powerd"
    # The port and peripheral inventory, and the `ports` service that gets a
    # wired link a lease when it comes up after boot.
    install -m 0755 "${outdir}/raven-ports" "${SYSROOT_DIR}/usr/bin/raven-ports"
    # The clock daemon. init.toml starts it as the `timed` service; it is the
    # other half of init's boot-time hwclock read -- NTP sync keeps the RTC
    # worth reading, and its socket is how the timezone gets set.
    install -m 0755 "${outdir}/raven-timed" "${SYSROOT_DIR}/usr/bin/raven-timed"
    # Removable storage. init.toml starts it as the `mount` service; without
    # it a plugged-in USB drive is a device node and nothing else, which is
    # what every file manager on the machine shows: nothing.
    install -m 0755 "${outdir}/raven-mount" "${SYSROOT_DIR}/usr/bin/raven-mount"
    # The fingerprint sensor. init.toml starts it as the `fprintd` service on a
    # machine that has a reader; on one that does not it answers "absent" and
    # costs a socket, which is a far better answer for a settings panel than a
    # missing socket it has to guess about. Nothing else on the image can talk
    # to the device -- there is no libfprint here.
    install -m 0755 "${outdir}/raven-fprintd" "${SYSROOT_DIR}/usr/bin/raven-fprintd"

    # stage4 owns the poweroff/reboot/halt/shutdown names and installs a
    # dispatcher that uses raven-rc when raven-init is PID 1, with an emergency
    # kernel fallback for rescue environments.

    RAVEN_BUILT+=("${init_bins[@]}")
    log_success "  raven-init installed ($(du -h "${outdir}/raven-init" | cut -f1))"
    log_success "  raven-rc installed ($(du -h "${outdir}/raven-rc" | cut -f1))"
    log_success "  raven-powerd installed ($(du -h "${outdir}/raven-powerd" | cut -f1))"
    log_success "  raven-ports installed ($(du -h "${outdir}/raven-ports" | cut -f1))"
    log_success "  raven-timed installed ($(du -h "${outdir}/raven-timed" | cut -f1))"
    log_success "  raven-mount installed ($(du -h "${outdir}/raven-mount" | cut -f1))"
    log_success "  raven-fprintd installed ($(du -h "${outdir}/raven-fprintd" | cut -f1))"
}

# =============================================================================
# raven-faced (local crate, not a fetched component)
# =============================================================================
# Face unlock's camera daemon. A crate of its own rather than another binary
# out of init/, because it links an ONNX inference engine and nothing that PID
# 1 builds should have to compile that -- see faced/src/main.rs for why it is
# not part of ravend either.
#
# Its models are fetched rather than built: they are 37 MB and are not in this
# repository, so fetch-models.sh pulls them by pinned SHA-256. They go into the
# image, because face unlock is part of Raven and an image carrying the daemon
# without them is a login screen that cannot use the camera until somebody
# finds a command to run.
#
# The pin is what keeps this reproducible. The fetch is not "whatever is at
# that URL today" -- it is those two exact files or a failure, and a failure
# skips the whole component rather than shipping a package whose file list
# claims models that are not there.
#
# faced/models/ is the cache. It survives between builds, so only the first
# build on a machine reaches the network, and it is the path the package
# manifest installs from.
build_raven_faced() {
    local src="${PROJECT_ROOT}/faced"

    [[ -f "${src}/Cargo.toml" ]] || {
        log_warn "No faced crate at ${src}, skipping raven-faced"
        return 0
    }

    log_step "raven-faced (face unlock)"

    local outdir="${RAVEN_STAGE_DIR}"
    mkdir -p "${outdir}"

    rm -f "${SYSROOT_DIR}/usr/bin/raven-faced"
    if ! build_rust_component "${src}" "${RAVEN_FACED_BINARIES}" "." "${outdir}"; then
        # Not fatal, and not silent. Everything else about the login screen
        # works without it: ravend answers "the face unlock service is not
        # running", and the password and the fingerprint reader are untouched.
        log_warn "  raven-faced: build failed, skipping (face unlock will be unavailable)"
        RAVEN_FAILED+=("raven-faced")
        return 0
    fi

    # Before anything lands in the sysroot, because the manifest installs the
    # models alongside the binary: a raven-faced packaged without them is a
    # package whose file list is a lie, which is the same all-or-nothing the
    # component table applies to binaries.
    local models="${src}/models"
    if ! sh "${src}/fetch-models.sh" "${models}"; then
        log_warn "  raven-faced: could not fetch the models, skipping (face unlock will be unavailable)"
        log_warn "    the fetch needs the network once; its cache is ${models}"
        RAVEN_FAILED+=("raven-faced")
        return 0
    fi

    mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/usr/share/raven-face/models"
    install -m 0755 "${outdir}/raven-faced" "${SYSROOT_DIR}/usr/bin/raven-faced"
    install -m 0755 "${src}/fetch-models.sh" \
        "${SYSROOT_DIR}/usr/share/raven-face/fetch-models.sh"
    install -m 0644 "${models}"/*.onnx \
        "${SYSROOT_DIR}/usr/share/raven-face/models/"

    RAVEN_BUILT+=("raven-faced")
    log_success "  raven-faced installed ($(du -h "${outdir}/raven-faced" | cut -f1))"
    log_success "  models installed ($(du -sh "${models}" | cut -f1 | tr -d ' ') in /usr/share/raven-face/models)"
}

build_all_components() {
    local go_ok=1 rust_ok=1

    setup_git_trust

    have_go || go_ok=0
    have_rust_musl || rust_ok=0

    if (( rust_ok == 1 )) && ! setup_cross_cc; then
        log_warn "No ${RAVEN_CROSS_PREFIX}-gcc in ${TOOLCHAIN_DIR}/bin"
        log_warn "  Rust components with C dependencies (rvn, ivaldi, crow, oxigen)"
        log_warn "  will fail to build. Run stage0 first to produce the cross toolchain."
    fi

    if (( go_ok == 0 )); then
        log_warn "Go toolchain not usable -- Go components will be skipped"
        log_info "  install it with: pacman -S go  (or see scripts/check-deps.sh)"
    fi
    if (( rust_ok == 0 )); then
        log_warn "Rust ${RUST_MUSL_TARGET} target not usable -- Rust components will be skipped"
        log_info "  install it with: rustup target add ${RUST_MUSL_TARGET}"
    fi

    for spec in "${RAVEN_COMPONENTS[@]}"; do
        IFS='|' read -r key repo lang binaries targets desc <<< "${spec}"

        local -a bins
        IFS=',' read -r -a bins <<< "${binaries}"

        if ! component_selected "${key}"; then
            log_info "Skipping ${bins[*]} (deselected)"
            RAVEN_SKIPPED+=("${bins[@]}")
            continue
        fi

        local old_bin
        for old_bin in "${bins[@]}"; do rm -f "${SYSROOT_DIR}/usr/bin/${old_bin}"; done
        rm -f "${SYSROOT_DIR}/usr/share/raven/build/sources/${repo}.tsv"

        if [[ "${lang}" == "go" ]] && (( go_ok == 0 )); then
            RAVEN_SKIPPED+=("${bins[@]}")
            continue
        fi
        if [[ "${lang}" == "rust" ]] && (( rust_ok == 0 )); then
            RAVEN_SKIPPED+=("${bins[@]}")
            continue
        fi

        build_component "${spec}"
    done

    build_raven_init
    build_raven_faced
}

# =============================================================================
# Default shell
# =============================================================================
# stage3 sets bash as the default so that the base system stands on its own.
# Once ravenshell is actually in the sysroot we take that over. If ravenshell
# did not build, this is a no-op and bash stays the default -- which is the
# whole point of doing it here rather than in stage3.
set_ravenshell_default() {
    local rsh="${SYSROOT_DIR}/usr/bin/ravenshell"

    if [[ ! -f "${rsh}" ]]; then
        log_info "ravenshell not installed, leaving bash as the default shell"
        return 0
    fi

    if [[ "${RAVEN_KEEP_BASH_DEFAULT:-0}" == "1" ]]; then
        log_info "RAVEN_KEEP_BASH_DEFAULT=1, leaving bash as the default shell"
        register_shells
        return 0
    fi

    log_step "Making ravenshell the default login shell..."

    register_shells

    # root's login shell
    if [[ -f "${SYSROOT_DIR}/etc/passwd" ]]; then
        sed -i 's|^root:\(.*\):[^:]*$|root:\1:/usr/bin/ravenshell|' \
            "${SYSROOT_DIR}/etc/passwd" 2>/dev/null || true
    else
        mkdir -p "${SYSROOT_DIR}/etc"
        echo "root:x:0:0:root:/root:/usr/bin/ravenshell" > "${SYSROOT_DIR}/etc/passwd"
    fi

    # default for new users
    mkdir -p "${SYSROOT_DIR}/etc/default"
    if [[ -f "${SYSROOT_DIR}/etc/default/useradd" ]]; then
        sed -i 's|^SHELL=.*|SHELL=/usr/bin/ravenshell|' \
            "${SYSROOT_DIR}/etc/default/useradd" 2>/dev/null || true
    fi

    log_success "Default shell set to ravenshell (bash remains available)"
}

# Rewrite /etc/shells with ravenshell first. bash, fish and sh stay listed --
# they are still installed, and /bin/sh in particular is what the boot and
# build scripts use.
register_shells() {
    mkdir -p "${SYSROOT_DIR}/etc"
    cat > "${SYSROOT_DIR}/etc/shells" << 'SHELLS'
# /etc/shells - valid login shells for RavenLinux
#
# Default: ravenshell (Raven Shell). bash and fish remain available as
# alternates; /bin/sh is the POSIX fallback the boot and build scripts use.
/bin/ravenshell
/usr/bin/ravenshell
/bin/bash
/usr/bin/bash
/bin/fish
/usr/bin/fish
/bin/sh
/usr/bin/sh
SHELLS
    log_info "Registered ravenshell in /etc/shells"
}

# =============================================================================
# Skeleton configuration
# =============================================================================
# Ship whatever configs/raven-shell/ holds into /etc and the user skeleton, the
# same way stage3 does for bash and fish.
install_raven_configs() {
    local configs_dir="${PROJECT_ROOT}/configs/ravenshell"

    [[ -d "${configs_dir}" ]] || return 0

    log_step "Installing ravenshell configuration..."
    mkdir -p "${SYSROOT_DIR}/etc/ravenshell" "${SYSROOT_DIR}/etc/skel/.config/ravenshell"
    cp -r "${configs_dir}/." "${SYSROOT_DIR}/etc/ravenshell/" 2>/dev/null || true
    cp -r "${configs_dir}/." "${SYSROOT_DIR}/etc/skel/.config/ravenshell/" 2>/dev/null || true
    log_success "ravenshell configuration installed"
}

# =============================================================================
# rvn configuration
# =============================================================================
# rvn reads pacman's configuration format -- it parses pacman.conf and the
# mirrorlists it includes, fetches each repo's $repo.db itself, and verifies
# package signatures against the keyring in GPGDir. Nothing shells out to
# pacman, and pacman is not installed.
#
# Without these files rvn boots onto a system with no repositories at all: the
# binary ships, runs, and can do nothing, which reads as a broken package
# manager rather than an unconfigured one.
#
# The canonical files in configs/rvn are the default rather than a copy of the
# build host's, because a host pacman.conf carries that host's repos and mirror
# ranking -- and sometimes a file:// path that exists on no other machine. An
# image that depends on the machine that built it is the thing the containerised
# build exists to avoid. RAVEN_PACMAN_FROM_HOST=1 takes the host's anyway, which
# is what you want when the host has a repo the target genuinely needs.
install_rvn_config() {
    # Only if rvn actually landed. RAVEN_SKIP=rvn should not leave a package
    # manager's configuration behind with no package manager to read it.
    if [[ ! -f "${SYSROOT_DIR}/usr/bin/rvn" ]]; then
        log_info "rvn not installed, skipping its configuration"
        return 0
    fi

    log_step "Installing rvn configuration..."

    # DBPath, CacheDir and the sync tree. rvn creates what it needs at run time,
    # but a read-only or oddly-permissioned parent turns that into a first-run
    # failure on the target rather than a build-time one here.
    mkdir -p "${SYSROOT_DIR}/etc/pacman.d" \
             "${SYSROOT_DIR}/var/lib/pacman/sync" \
             "${SYSROOT_DIR}/var/cache/pacman/pkg"

    local from_host="${RAVEN_PACMAN_FROM_HOST:-0}"
    local installed_conf=0

    if [[ "${from_host}" == "1" ]]; then
        if [[ -f /etc/pacman.conf ]]; then
            cp /etc/pacman.conf "${SYSROOT_DIR}/etc/pacman.conf"
            installed_conf=1
            log_info "  pacman.conf copied from the build host"

            # The host's conf almost certainly Includes this, and a config whose
            # include is missing leaves rvn with a repo and no servers.
            if [[ -f /etc/pacman.d/mirrorlist ]]; then
                cp /etc/pacman.d/mirrorlist "${SYSROOT_DIR}/etc/pacman.d/mirrorlist"
                log_info "  mirrorlist copied from the build host ($(grep -c '^Server' /etc/pacman.d/mirrorlist 2>/dev/null || echo 0) servers)"
            else
                log_warn "  host has no /etc/pacman.d/mirrorlist; rvn may have no servers"
            fi
        else
            log_warn "  RAVEN_PACMAN_FROM_HOST=1 but the host has no /etc/pacman.conf"
            log_warn "  falling back to the canonical configuration"
        fi
    fi

    if (( installed_conf == 0 )); then
        if [[ -f "${PROJECT_ROOT}/configs/rvn/pacman.conf" ]]; then
            cp "${PROJECT_ROOT}/configs/rvn/pacman.conf" "${SYSROOT_DIR}/etc/pacman.conf"
            log_info "  pacman.conf installed from configs/rvn"
        else
            log_warn "  configs/rvn/pacman.conf not found; rvn will have no configuration"
            return 0
        fi

        if [[ -f "${PROJECT_ROOT}/configs/rvn/mirrorlist" ]]; then
            cp "${PROJECT_ROOT}/configs/rvn/mirrorlist" "${SYSROOT_DIR}/etc/pacman.d/mirrorlist"
            log_info "  mirrorlist installed from configs/rvn"
        fi
    fi

    # The [raven] section goes in before the keyring, because whether it came
    # out enabled is what turns the Raven signing key from optional into
    # mandatory. Both of these return non-zero for a reason the build must not
    # survive -- see the section below.
    if ! install_raven_repo_section; then
        return 1
    fi

    install_rvn_keyring

    if ! install_raven_key; then
        return 1
    fi

    # After the [raven] section, because whether that section came out enabled
    # is the whole of what decides this. See the AUR shadow guard section.
    install_raven_aur_shadow_guard || return 1

    log_success "rvn configuration installed"
}

# rvn verifies package signatures itself and defaults to SigLevel = Required, so
# a system without a keyring fails every install -- closed rather than silently
# unsigned, which is the right way round but still a system that cannot install
# anything. The host's keyring is the only one available at build time; it is
# the Arch developer and packager keys, which is exactly what signs the packages
# the mirrorlist points at.
install_rvn_keyring() {
    local host_gpg=/etc/pacman.d/gnupg
    local dest="${SYSROOT_DIR}/etc/pacman.d/gnupg"

    if [[ ! -f "${host_gpg}/pubring.gpg" ]]; then
        log_warn "  no keyring at ${host_gpg}/pubring.gpg on the build host"
        log_warn "  rvn will refuse to install: SigLevel = Required with nothing to verify against"
        log_warn "  install archlinux-keyring and run pacman-key --init --populate archlinux"
        return 0
    fi

    mkdir -p "${dest}"
    # pubring and trustdb only. The private keyring, the random seed and the
    # socket files under a live gnupg home are either secret, machine-specific,
    # or meaningless off the machine that made them.
    cp "${host_gpg}/pubring.gpg" "${dest}/pubring.gpg" 2>/dev/null || true
    [[ -f "${host_gpg}/trustdb.gpg" ]] && cp "${host_gpg}/trustdb.gpg" "${dest}/trustdb.gpg" 2>/dev/null || true
    chmod 0755 "${dest}"
    chmod 0644 "${dest}/pubring.gpg" 2>/dev/null || true

    log_info "  keyring staged ($(du -h "${dest}/pubring.gpg" 2>/dev/null | cut -f1))"
}

# =============================================================================
# The [raven] repository and the key that makes it usable
# =============================================================================
# The section above stages somebody else's keyring so the image can install
# somebody else's packages. This one is about the image installing ours.
#
# configs/rvn/pacman.conf carries a [raven] section between two marker lines,
# written out in full and commented out. That file is the single copy of the
# text: everything here either splices it into the staged pacman.conf as it
# stands, or uncomments its three directive lines first. Nothing here writes a
# second version of the stanza, because two copies of a repository definition
# drift and the one that drifts is always the one nobody is reading.
#
# WHY THE KEY IS A HARD FAILURE AND THE ARCH KEYRING IS NOT.
#
# install_rvn_keyring warns and continues when the build host has no keyring,
# which is right for that case: the image is still complete, it just cannot
# reach Arch's mirrors until somebody populates a keyring, and the fix is on
# the installed machine. The Raven key is the opposite. If it does not ship,
# there is nothing on the installed machine that can fix it -- the repository
# it is meant to verify is the one that would deliver the fix, and under
# SigLevel = Required it refuses to serve anything that cannot be checked. A
# warning here would buy a build that finishes and an image whose first
# `rvn update` fails closed, on somebody else's machine, months later, with
# no way forward that does not involve editing pacman.conf by hand. So when
# [raven] is enabled and the key is missing, this stage fails the build. The
# failure belongs to whoever is holding the keyboard now.
#
# The whole key procedure -- generating it, where the private half lives, how
# the public half gets here, and how to build unsigned packages locally -- is
# docs/raven-repository-signing.md. Nothing in this file generates a key, and
# nothing here ever reads private key material.

# The markers in configs/rvn/pacman.conf. Matched at the start of a line, so a
# mention of them in prose elsewhere in that file does not move the boundary.
RAVEN_REPO_BEGIN="# >>> raven repository"
RAVEN_REPO_END="# <<< raven repository"

# The host in the shipped Server line, which does not serve a repository yet.
# Recognised here only so the build can say so once when somebody enables the
# section without naming a server of their own.
RAVEN_REPO_PLACEHOLDER_HOST="packages.ravenlinux.org"

# Whether this build was asked for a live [raven] section.
raven_repo_requested() {
    [[ "${RAVEN_REPO:-0}" == "1" || -n "${RAVEN_REPO_SERVER:-}" ]]
}

# Whether the staged pacman.conf ends up with a live [raven] section. Read off
# the file rather than off the environment on purpose: RAVEN_PACMAN_FROM_HOST
# can bring in a host conf that already enables it, and a key requirement that
# consulted only RAVEN_REPO would miss exactly that case.
raven_repo_is_enabled() {
    local conf="${SYSROOT_DIR}/etc/pacman.conf"
    [[ -f "${conf}" ]] || return 1
    grep -qE '^[[:space:]]*\[raven\][[:space:]]*$' "${conf}"
}

# Prints the [raven] block as it should appear in the staged pacman.conf:
# verbatim when the repository was not asked for, with its directives
# uncommented when it was.
#
# The three directive lines are the only ones in the block with no space after
# the `#`, which is what makes uncommenting them a sed expression rather than a
# parser. The prose examples inside the block -- including the file:// Server
# -- are written with spaces after the hash and stay commented.
raven_repo_block() {
    local canonical="${PROJECT_ROOT}/configs/rvn/pacman.conf"
    [[ -f "${canonical}" ]] || return 1

    local block
    block="$(awk -v b="${RAVEN_REPO_BEGIN}" -v e="${RAVEN_REPO_END}" '
        index($0, b) == 1 { inside = 1 }
        inside            { print }
        index($0, e) == 1 { inside = 0 }
    ' "${canonical}")"

    [[ -n "${block}" ]] || return 1

    if ! raven_repo_requested; then
        printf '%s\n' "${block}"
        return 0
    fi

    block="$(printf '%s\n' "${block}" | sed \
        -e 's|^#\[raven\]$|[raven]|' \
        -e 's|^#SigLevel = |SigLevel = |' \
        -e 's|^#Server = |Server = |')"

    if [[ -n "${RAVEN_REPO_SERVER:-}" ]]; then
        block="$(printf '%s\n' "${block}" | sed -e "s|^Server = .*$|Server = ${RAVEN_REPO_SERVER}|")"
    fi

    printf '%s\n' "${block}"
}

# Puts the [raven] block into the staged pacman.conf, however that file got
# there.
#
# RAVEN_PACMAN_FROM_HOST=1 REPLACES the staged pacman.conf with the build
# host's, which has no reason to carry a [raven] section and every reason not
# to: the host is an Arch machine with Arch's repositories. Taking the host's
# repositories is a deliberate request and this honours it -- but it is a
# request about where the *base system* comes from, and answering it by
# silently dropping the only repository that serves RavenLinux's own software
# is not what anybody meant by it. So the block is appended to the host's
# configuration instead of being lost with the file it was written in.
install_raven_repo_section() {
    local conf="${SYSROOT_DIR}/etc/pacman.conf"

    # No configuration was staged at all -- the caller has already warned, and
    # there is nothing to amend.
    [[ -f "${conf}" ]] || return 0

    # Validated before it reaches sed, where `|` would end the expression and
    # `&` would expand to the whole match. Both are characters no URL has, so
    # refusing them costs nothing and refusing them here is the difference
    # between an error message and a corrupted pacman.conf.
    local server="${RAVEN_REPO_SERVER:-}"
    if [[ -n "${server}" ]]; then
        if [[ ! "${server}" =~ ^(https?|file)://[^[:space:]\&\|]+$ ]]; then
            log_error "  RAVEN_REPO_SERVER=${server} is not a usable Server line"
            log_error "  it must be an http://, https:// or file:// URL with no whitespace"
            return 1
        fi
    fi

    local block
    if ! block="$(raven_repo_block)"; then
        # The canonical file is where the stanza lives; without it there is
        # nothing to install. Fatal only if somebody asked for the repository,
        # because then the alternative is an image that quietly does not have
        # the thing the build was told to give it.
        if raven_repo_requested; then
            log_error "  RAVEN_REPO was set, but configs/rvn/pacman.conf has no [raven] block"
            log_error "  the markers this looks for are '${RAVEN_REPO_BEGIN}' and '${RAVEN_REPO_END}'"
            return 1
        fi
        log_warn "  no [raven] block in configs/rvn/pacman.conf; the image gets no Raven repository"
        return 0
    fi

    # A build host that is itself a RavenLinux machine already answers this
    # question in its own pacman.conf, and it answers it with a Server that is
    # reachable from where it stands. Appending a second [raven] would give
    # rvn two repositories of one name; leaving theirs alone is both correct
    # and the smaller surprise.
    if ! grep -q "${RAVEN_REPO_BEGIN}" "${conf}" && grep -qE '^[[:space:]]*\[raven\]' "${conf}"; then
        log_info "  the staged pacman.conf already defines [raven]; leaving it as it is"
        return 0
    fi

    local tmp
    tmp="$(mktemp "${BUILD_DIR}/raven-repo-XXXXXX")" || return 1
    printf '%s\n' "${block}" > "${tmp}"

    if grep -q "${RAVEN_REPO_BEGIN}" "${conf}"; then
        # The canonical file: replace the block between the markers, so a
        # rerun of this stage is idempotent and an edit to configs/rvn lands.
        if ! awk -v b="${RAVEN_REPO_BEGIN}" -v e="${RAVEN_REPO_END}" -v blockfile="${tmp}" '
            index($0, b) == 1 {
                skipping = 1
                while ((getline line < blockfile) > 0) print line
                close(blockfile)
                next
            }
            skipping && index($0, e) == 1 { skipping = 0; next }
            skipping                      { next }
                                          { print }
        ' "${conf}" > "${tmp}.conf"; then
            rm -f "${tmp}" "${tmp}.conf"
            log_error "  could not rewrite the [raven] section of ${conf}"
            return 1
        fi
        mv -f "${tmp}.conf" "${conf}"
    else
        # The host's file (RAVEN_PACMAN_FROM_HOST=1), or any other conf that
        # predates the markers.
        { printf '\n'; cat "${tmp}"; } >> "${conf}"
        log_info "  [raven] appended to the pacman.conf this build took from the host"
    fi
    rm -f "${tmp}" "${tmp}.conf"

    if raven_repo_is_enabled; then
        local server_line
        server_line="$(printf '%s\n' "${block}" | grep -m1 '^Server = ' || true)"
        log_info "  [raven] enabled: ${server_line:-no Server line}"

        if [[ "${server_line}" == *"${RAVEN_REPO_PLACEHOLDER_HOST}"* ]]; then
            log_warn "  ${RAVEN_REPO_PLACEHOLDER_HOST} does not serve a repository yet"
            log_warn "  set RAVEN_REPO_SERVER=<url> to point this image somewhere real"
        fi

        # The repository this build just produced is the one that would be
        # published under this name, and the policy written above demands a
        # signed database. Said here because the moment to learn it is while
        # the packages are still on this machine, not when `rvn sync` on an
        # installed system refuses a database nobody signed.
        if [[ -f "${RAVEN_PKG_DIR}/raven.db" && ! -e "${RAVEN_PKG_DIR}/raven.db.sig" ]]; then
            log_warn "  ${RAVEN_PKG_DIR}/raven.db is not signed, and [raven] is DatabaseRequired"
            log_warn "  configure [sign] key in /etc/rvn/build.toml on this build host"
            log_warn "  see docs/raven-repository-signing.md"
        fi
    else
        log_info "  [raven] shipped commented out (RAVEN_REPO=1, or RAVEN_REPO_SERVER=<url>)"
    fi
}

# Puts the Raven signing key's PUBLIC half into the image's keyring.
#
# rvn reads one file for this: `pubring.gpg` in GPGDir, parsed as a raw
# OpenPGP packet stream (verify.rs, Keyring::load). It does not read GnuPG's
# modern `pubring.kbx`, which is why the key is appended to that file rather
# than imported with `gpg --import` into the staged directory -- an import
# would land in the keybox on any host with a current gnupg and rvn would
# never see it. Appending public key packets is the whole of what pubring.gpg
# is, and `pacman-key --list-keys` on the installed machine reads it too.
#
# WHERE THE KEY COMES FROM. RAVEN_SIGNING_PUBKEY names an exported public key;
# otherwise configs/rvn/raven-signing-key.gpg (binary, what `gpg --export`
# writes) or .asc (armoured, what `gpg --armor --export` writes) is used if
# either is there. Neither is in this repository, and neither should be
# generated by anything automatic: see docs/raven-repository-signing.md.
install_raven_key() {
    local dest="${SYSROOT_DIR}/etc/pacman.d/gnupg"
    local pubring="${dest}/pubring.gpg"
    local enabled=0
    raven_repo_is_enabled && enabled=1

    local key="${RAVEN_SIGNING_PUBKEY:-}"
    if [[ -z "${key}" ]]; then
        local candidate
        for candidate in "${PROJECT_ROOT}/configs/rvn/raven-signing-key.gpg" \
                         "${PROJECT_ROOT}/configs/rvn/raven-signing-key.asc"; do
            if [[ -f "${candidate}" ]]; then
                key="${candidate}"
                break
            fi
        done
    fi

    if [[ -z "${key}" || ! -f "${key}" ]]; then
        if (( enabled == 0 )); then
            # An image with no [raven] repository has nothing to verify with
            # this key, so its absence is not a problem to report loudly.
            log_info "  no Raven signing key found, and no [raven] repository needing one"
            return 0
        fi

        log_error "  [raven] is enabled in the staged pacman.conf and there is no Raven key to ship"
        log_error "  SigLevel = Required with no key is a repository the image can never install from,"
        log_error "  and the repository is where the fix for that would come from -- so this build stops here."
        log_error "  Export the public half and point the build at it:"
        log_error "    gpg --export --output configs/rvn/raven-signing-key.gpg <fingerprint>"
        log_error "    RAVEN_SIGNING_PUBKEY=/path/to/key.gpg ./scripts/build.sh raven"
        log_error "  The procedure is docs/raven-repository-signing.md."
        return 1
    fi

    mkdir -p "${dest}"
    chmod 0755 "${dest}"

    local material="${key}"
    local dearmored=""
    local first
    first="$(od -An -N1 -tx1 < "${key}" | tr -d ' \n')"

    # An armoured export starts with the '-' of "-----BEGIN PGP PUBLIC KEY
    # BLOCK-----". rvn's parser reads packets, not armour, so it has to be
    # decoded before it goes anywhere near the keyring.
    if [[ "${first}" == "2d" ]]; then
        if ! command -v gpg >/dev/null 2>&1; then
            log_error "  ${key} is armoured and this build host has no gpg to decode it"
            log_error "  export the binary form instead: gpg --export --output <file>.gpg <fingerprint>"
            return 1
        fi
        dearmored="$(mktemp "${BUILD_DIR}/raven-key-XXXXXX")" || return 1
        if ! gpg --dearmor < "${key}" > "${dearmored}" 2>/dev/null; then
            rm -f "${dearmored}"
            log_error "  gpg could not decode ${key}; is it an exported public key?"
            return 1
        fi
        material="${dearmored}"
        first="$(od -An -N1 -tx1 < "${material}" | tr -d ' \n')"
    fi

    # The first packet of an exported key says which half of it this is. Tag 6
    # is a public key; tag 5 is a SECRET key, and an exported private key
    # appended to the image's keyring would publish the thing the whole of
    # docs/raven-repository-signing.md exists to keep off disks like this one.
    # Old-format headers are 0x98-0x9b (tag 6) and 0x94-0x97 (tag 5); the
    # new-format spelling is 0xc6 and 0xc5.
    case "${first}" in
        98|99|9a|9b|c6)
            ;;
        94|95|96|97|c5)
            [[ -n "${dearmored}" ]] && rm -f "${dearmored}"
            log_error "  ${key} is a SECRET key, not a public one. It will not be shipped."
            log_error "  Export the public half: gpg --export --output <file>.gpg <fingerprint>"
            return 1
            ;;
        *)
            [[ -n "${dearmored}" ]] && rm -f "${dearmored}"
            log_error "  ${key} does not begin with an OpenPGP public key packet (first byte 0x${first:-??})"
            log_error "  it should be the output of: gpg --export --output <file>.gpg <fingerprint>"
            return 1
            ;;
    esac

    # install_rvn_keyring rewrites pubring.gpg from the host's on every run, so
    # appending is normally not cumulative. It is not cumulative on the path
    # where the host had no keyring either, because that leaves this the only
    # writer of the file -- but a rerun over a pubring left by a previous run
    # would append a second copy of the same key, which verifies fine and is
    # still wrong. Comparing the tail against the key is exact and cheap.
    local size
    size="$(stat -c%s "${material}")"
    if [[ -f "${pubring}" ]] && cmp -s <(tail -c "${size}" "${pubring}") "${material}"; then
        log_info "  Raven signing key already in the staged keyring"
    else
        cat "${material}" >> "${pubring}"
        chmod 0644 "${pubring}"
        log_info "  Raven signing key added to the staged keyring (${size} bytes)"
    fi

    # Identity, for the build log and for anybody checking afterwards which key
    # an image actually trusts. Best effort: the key is already installed and a
    # build host without gpg is a build host that can still ship one.
    if command -v gpg >/dev/null 2>&1; then
        local fpr
        fpr="$(gpg --with-colons --show-keys "${material}" 2>/dev/null \
                 | awk -F: '$1 == "fpr" { print $10; exit }')"
        [[ -n "${fpr}" ]] && log_info "  fingerprint ${fpr}"
    fi

    [[ -n "${dearmored}" ]] && rm -f "${dearmored}"

    if (( enabled == 0 )); then
        log_info "  ([raven] is not enabled in this image; the key is there for when it is)"
    fi

    return 0
}

# =============================================================================
# The AUR shadow guard
# =============================================================================
# Writing a local-database record for a Raven package while nothing in the
# image's pacman.conf carries that name makes the package FOREIGN, and rvn
# resolves a foreign package against the AUR by name. The chain, read out of
# RavenPackageManager rather than guessed:
#
#   ops/update.rs   foreign = every installed package no sync database has,
#                   and `ctx.aur.prefetch(&foreign)` asks the AUR about all
#                   of them by name.
#   upgrade.rs      for a package no repository carries, any AUR result whose
#                   vercmp beats the installed version becomes a Kind::Upgrade
#                   with Origin::Aur.
#
# The installed version of everything this stage packages is near the bottom
# of the version space, and AUR names are first-come. So anyone who registers
# `crow`, `caw`, `poxy`, `imlazy`, `oxigen`, `ivaldi`, `ravenshell`,
# `raven-init` or `ravenpackagemanager` in the AUR at 1.0.0-1 has their
# PKGBUILD offered to every Raven machine as an upgrade -- to PID 1, to the
# login shell, to the package manager -- and executed as root on any machine
# whose owner says yes. Before these records existed those files were owned by
# nothing and were never looked up: the records are what create the lookup, so
# doing nothing here is not the neutral option.
#
# WHAT THIS DOES ABOUT IT, and what it does not.
#
# rvn already has the mechanism, and RavenPackageManager/src/provides.rs was
# written for the same hazard in the same words: a provision file says the
# base system provides a name, and resolve.rs refuses to install over one
# BY NAME, before anything else in resolve() -- "Named explicitly or not, a
# package is never put over the component the system built in its place."
# ops/install.rs then reports each refusal as "<name> is provided by Raven
# itself; installing it would replace that, so it is skipped". That closes the
# replacement: neither `rvn update` nor `rvn install ravenshell` can put an
# AUR build over the Raven layer.
#
# It does NOT stop `rvn update` from asking the AUR about these names, or from
# listing the answer as an available upgrade before refusing to apply it. That
# needs a change inside rvn, which this stage cannot make. The precise fix
# there, for whoever picks it up: ops/update.rs's foreign filter should also
# exclude packages the local record marks as built by this distribution -- the
# record already carries `%XDATA%\npkgtype=pkg`, so an `origin=raven` beside
# it, written by sysroot_install_package_body and honoured by the filter,
# costs one field and one predicate and removes the lookup entirely rather
# than only its consequence.
#
# ONLY WHILE [raven] IS OFF. With the repository enabled the ten names are in
# a sync database, so they are not foreign, rvn never asks the AUR about them
# at all, and an upgrade published there is a real upgrade this image must not
# refuse. The guard file is therefore written exactly when there is no
# repository to speak for these names, and removed when there is -- including
# on a rerun that turns the repository on, which is why the enabled branch
# deletes rather than simply returning.
RAVEN_AUR_GUARD_FILE="usr/share/rvn/provides.d/raven-layer"

# Every pkgname the manifests under packages/ declare for the Raven layer.
# Read from [package] name so a manifest that renames itself is followed --
# packages/raven/rvn is `ravenpackagemanager`, and guessing that from the
# directory name is how this list would quietly stop covering it.
raven_manifest_package_names() {
    local manifest
    for manifest in "${PROJECT_ROOT}"/packages/raven/*/package.toml \
                    "${PROJECT_ROOT}"/packages/gui/*/package.toml; do
        [[ -f "${manifest}" ]] || continue
        awk '
            /^[[:space:]]*\[/ { in_package = ($0 ~ /^[[:space:]]*\[package\][[:space:]]*$/) }
            in_package && /^[[:space:]]*name[[:space:]]*=/ {
                if (match($0, /"[^"]*"/)) {
                    print substr($0, RSTART + 1, RLENGTH - 2)
                    exit
                }
            }
        ' "${manifest}"
    done
}

# Every package name the sysroot's local database actually records. This is
# the list `rvn update` will iterate on the installed machine, which is why it
# is read off the database rather than off what this particular run packaged:
# a rerun with RAVEN_ONLY=crow packages one component and leaves nine records
# from the previous run in place, and a guard built from this run's output
# would drop the other nine.
raven_installed_package_names() {
    local localdb="${SYSROOT_DIR}/var/lib/pacman/local" dir
    [[ -d "${localdb}" ]] || return 0
    for dir in "${localdb}"/*/; do
        [[ -f "${dir}desc" ]] || continue
        awk '/^%NAME%$/ { getline; print; exit }' "${dir}desc"
    done
}

# The names to guard: recorded in this image AND declared by a manifest in
# this repository. Both halves matter. Without the first, the file would claim
# the system provides software that is not in it; without the second, a future
# stage that records a genuinely third-party package would have it silently
# frozen against its own upstream.
raven_guarded_package_names() {
    local -a manifests=()
    mapfile -t manifests < <(raven_manifest_package_names)
    (( ${#manifests[@]} > 0 )) || return 0

    local installed declared
    while read -r installed; do
        [[ -n "${installed}" ]] || continue
        for declared in "${manifests[@]}"; do
            if [[ "${declared}" == "${installed}" ]]; then
                printf '%s\n' "${installed}"
                break
            fi
        done
    done < <(raven_installed_package_names)
}

install_raven_aur_shadow_guard() {
    local file="${SYSROOT_DIR}/${RAVEN_AUR_GUARD_FILE}"

    if raven_repo_is_enabled; then
        if [[ -f "${file}" ]]; then
            rm -f "${file}"
            log_info "  [raven] is enabled, so its packages are not foreign: AUR guard removed"
        fi
        return 0
    fi

    local -a guarded=()
    mapfile -t guarded < <(raven_guarded_package_names)

    if (( ${#guarded[@]} == 0 )); then
        # Nothing is recorded, so nothing is foreign, so there is nothing to
        # shadow. A guard file left by an earlier run would now be naming
        # software the image does not have.
        rm -f "${file}"
        return 0
    fi

    mkdir -p "$(dirname "${file}")" || return 1
    {
        echo "# The Raven layer, declared to rvn as provided by the system."
        echo "#"
        echo "# Written by install_raven_aur_shadow_guard in"
        echo "# scripts/stages/stage-raven.sh, because this image carries a"
        echo "# local-database record for each of these and no repository that"
        echo "# carries the name. rvn resolves such a package against the AUR,"
        echo "# where the names below are unclaimed and the installed versions"
        echo "# are low -- so without this file an AUR package called"
        echo "# raven-init is offered as an upgrade to this machine's PID 1."
        echo "#"
        echo "# resolve.rs refuses to install over a name listed here, whether"
        echo "# it was asked for explicitly or reached as a dependency."
        echo "#"
        echo "# Delete this file if you enable [raven] in /etc/pacman.conf and"
        echo "# want upgrades from it -- or rebuild the image with RAVEN_REPO=1,"
        echo "# which does not write it in the first place."
        printf '%s\n' "${guarded[@]}"
    } > "${file}" || return 1
    chmod 0644 "${file}"

    log_info "  AUR shadow guard: ${#guarded[@]} Raven package(s) in ${RAVEN_AUR_GUARD_FILE}"
    return 0
}

# =============================================================================
# Summary
# =============================================================================
# Every path the sysroot's local database claims an owner for, one per line
# and without a leading slash -- the spelling a %FILES% record uses.
raven_owned_paths() {
    local localdb="${SYSROOT_DIR}/var/lib/pacman/local" list
    [[ -d "${localdb}" ]] || return 0
    for list in "${localdb}"/*/files; do
        [[ -f "${list}" ]] || continue
        awk '/^%FILES%$/ { on = 1; next } /^%/ { on = 0 } on && NF { print }' "${list}"
    done
}

# How much of the image actually has an owner.
#
# The Packaging section above claims this stage is what makes the image own
# its own userland. That claim needs a number beside it and not a list of the
# components that happened to be packaged, because the number is much smaller
# than the sentence suggests and the difference matters: a file no package
# owns cannot be upgraded, verified or removed, and `rvn owns` answers
# nothing for it.
#
# Two counts, because they are two different questions and only the first is
# this stage's to answer:
#
#   the Raven layer   the programs this distribution wrote for itself. These
#                     are built here and packaged here, so this figure should
#                     read "all of them" on a complete run.
#   /usr/bin          everything a booted machine has on its PATH. Most of it
#                     is the base layer stage2 copied off the build container
#                     with `install -D`, which no stage has ever packaged.
#
# And the manifests with no package, which is the specific gap this was added
# for: packages/raven and packages/gui carry manifests for programs stage2's
# install_raven_* functions and stage-gui.sh put in place as loose files.
# package_built_components walks RAVEN_COMPONENTS plus raven-init and
# raven-faced and nothing else, so those manifests validate and are wired to
# nothing -- `rvn owns /usr/bin/raven-dhcp` answers nothing on a finished
# image for the program /etc/raven/init.toml runs as the `network` service.
# Closing that means giving those stages the same package_and_install path
# this one uses, which means lifting these helpers into
# scripts/lib/packaging.sh first, and neither is a change this file can make
# on its own. Until somebody does, the count says so out loud rather than
# leaving the reader to infer it from a list of ten.
print_ownership() {
    local -A owned=()
    local path
    while read -r path; do
        [[ -n "${path}" ]] || continue
        owned["${path#/}"]=1
    done < <(raven_owned_paths)

    echo ""
    echo "Ownership:"

    local binary present=0 present_owned=0
    while read -r binary; do
        [[ -n "${binary}" ]] || continue
        [[ -e "${SYSROOT_DIR}/usr/bin/${binary}" ]] || continue
        present=$(( present + 1 ))
        if [[ -n "${owned["usr/bin/${binary}"]:-}" ]]; then
            present_owned=$(( present_owned + 1 ))
        fi
    done < <(raven_layer_binaries)
    echo "  Raven layer: ${present_owned} of the ${present} programs in the image are owned by a package"

    local entry rel bins=0 bins_owned=0
    if [[ -d "${SYSROOT_DIR}/usr/bin" ]]; then
        for entry in "${SYSROOT_DIR}"/usr/bin/*; do
            [[ -e "${entry}" || -L "${entry}" ]] || continue
            bins=$(( bins + 1 ))
            rel="usr/bin/$(basename "${entry}")"
            if [[ -n "${owned["${rel}"]:-}" ]]; then
                bins_owned=$(( bins_owned + 1 ))
            fi
        done
    fi
    echo "  /usr/bin:    ${bins_owned} of ${bins} entries are owned by a package"

    local -a declared=()
    mapfile -t declared < <(raven_manifest_package_names)
    local -A recorded=()
    local name
    while read -r name; do
        [[ -n "${name}" ]] || continue
        recorded["${name}"]=1
    done < <(raven_installed_package_names)

    local -a orphaned=()
    for name in ${declared[@]+"${declared[@]}"}; do
        if [[ -z "${recorded["${name}"]:-}" ]]; then
            orphaned+=("${name}")
        fi
    done
    if (( ${#orphaned[@]} > 0 )); then
        echo "  ${#orphaned[@]} of ${#declared[@]} manifests under packages/ have no record in this image:"
        echo "    ${orphaned[*]}"
        echo "    (no stage packages these. Where the program is in the image at all,"
        echo "     stage2 or stage-gui installed it as a file that nothing owns, so"
        echo "     nothing can upgrade, verify or remove it)"
    fi
}

print_summary() {
    echo ""
    echo "=========================================="
    echo "  Raven Stage Summary"
    echo "=========================================="
    echo ""

    echo "Components:"
    for spec in "${RAVEN_COMPONENTS[@]}"; do
        IFS='|' read -r key repo lang binaries targets desc <<< "${spec}"

        local -a bins
        IFS=',' read -r -a bins <<< "${binaries}"

        local b path
        for b in "${bins[@]}"; do
            path="${SYSROOT_DIR}/usr/bin/${b}"
            if [[ -f "${path}" ]]; then
                printf "  [OK] %-14s %-6s %s\n" "${b}" "$(du -h "${path}" | cut -f1)" "${desc}"
            else
                printf "  [--] %-14s %-6s %s\n" "${b}" "" "${desc}"
            fi
        done
    done

    echo ""
    echo "Default shell:"
    if grep -q "ravenshell" "${SYSROOT_DIR}/etc/passwd" 2>/dev/null; then
        echo "  [OK] ravenshell"
    else
        echo "  [--] ravenshell (bash still default)"
    fi

    if (( ${#RAVEN_FAILED[@]} > 0 )); then
        echo ""
        echo "Not built: ${RAVEN_FAILED[*]}"
        echo "  (the ISO still builds; rerun this stage after fixing the cause)"
    fi

    if (( ${#RAVEN_PACKAGED[@]} > 0 )); then
        echo ""
        echo "Packaged: ${RAVEN_PACKAGED[*]}"
        echo "  (recorded in the image's package database; archives in ${RAVEN_PKG_DIR})"
    fi

    # Named separately from "Not built" because these are in the image and
    # working -- what they are missing is an owner, which is the difference
    # between software the machine can upgrade and files that are simply there.
    if (( ${#RAVEN_UNPACKAGED[@]} > 0 )); then
        echo ""
        echo "Not packaged: ${RAVEN_UNPACKAGED[*]}"
        echo "  (installed as files that no package owns; see ${RAVEN_PKG_LOG})"
    fi

    print_ownership
    echo ""
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Raven Stage: Self-Hosted Toolchain"
    echo "=========================================="
    echo ""

    mkdir -p "${LOGS_DIR}" "${RAVEN_SRC_DIR}" "${RAVEN_STAGE_DIR}"

    if [[ ! -d "${SYSROOT_DIR}" ]]; then
        log_error "Sysroot not found at ${SYSROOT_DIR}"
        log_error "Run stage2 and stage3 first."
        return 1
    fi

    # RavenShell, rvn, poxy, ivaldi, crow, imlazy, oxigen
    build_all_components

    # Turn what was just built into packages and install those, so the image
    # owns its own userland. After the build and not inside it, because the
    # tool that does the packaging is rvn -- see find_package_builder.
    package_built_components

    # ravenshell config into /etc and /etc/skel, if configs/ravenshell exists
    install_raven_configs

    # pacman.conf, mirrorlist and keyring for rvn, if rvn actually landed
    install_rvn_config

    # Take over the default shell, but only if ravenshell actually landed
    set_ravenshell_default

    print_summary

    log_success "Raven stage complete!"
    echo ""
}

# Run main (whether executed directly or sourced)
main "$@"
