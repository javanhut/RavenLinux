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
# Package definitions live in packages/raven.
#
# Environment:
#   RAVEN_ONLY=crow,ivaldi        build only these components
#   RAVEN_SKIP=oxigen             skip these components
#   RAVEN_OFFLINE=1               never touch the network; use existing clones
#   RAVEN_<KEY>_REF=<git-ref>     pin one component (e.g. RAVEN_IVALDI_REF=v0.1.2)
#   RAVEN_KEEP_BASH_DEFAULT=1     install ravenshell but leave bash as the
#                                 default login shell
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
# key|repo|lang|binaries|build_targets|description
#
#   key           short name; also the RAVEN_<KEY>_REF env suffix and the
#                 packages/raven/<key> directory
#   repo          github.com/javanhut/<repo>
#   lang          go | rust
#   binaries      the installed name(s) in /usr/bin. Comma-separated for a
#                 component that ships more than one -- caw is a CLI plus the
#                 daemon it drives, and neither is useful without the other.
#   build_targets go:   the package path(s) to build, positionally paired with
#                       `binaries`
#                 rust: the workspace package(s) to pass to -p, or "." for a
#                       plain crate
#
# The graphical components are intentionally absent. RavenGUI (huginn/muninn)
# links libudev, libdrm, libseat and libinput through smithay, so it cannot be
# a static musl binary the way everything here is, and its udev/TTY backend is
# not written yet. RavenTerminal needs the display server RavenGUI is meant to
# become. Both belong to a graphical stage that does not exist yet.

RAVEN_COMPONENTS=(
    "ravenshell|RavenShell|go|ravenshell|.|Raven Shell - interactive shell and scripting language"
    "rvn|RavenPackageManager|rust|rvn|.|Raven Package Manager"
    "poxy|Poxy|go|poxy|./cmd|Poxy - universal package manager"
    "ivaldi|Ivaldi|rust|ivaldi|.|Ivaldi - version control system"
    "crow|CrowTextEditor|rust|crow|.|Crow - text editor"
    "imlazy|ImLazy|go|imlazy|.|ImLazy - task runner"
    "oxigen|OxigenLang|rust|oxigen|oxigen|OxigenLang - interpreted language"
    "caw|CAW|rust|caw,cawd|caw,cawd|CAW - wireless and network utility, with its daemon"
)

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
    local url="https://github.com/javanhut/${repo}.git"

    # RAVEN_IVALDI_REF, RAVEN_CROW_REF, ...
    local ref_var="RAVEN_${key^^}_REF"
    local ref="${!ref_var:-}"

    if [[ "${RAVEN_OFFLINE:-0}" == "1" ]]; then
        if [[ -d "${dest}/.git" ]]; then
            log_info "  offline: using existing clone of ${repo}"
            return 0
        fi
        log_warn "  offline: no clone of ${repo} in ${RAVEN_SRC_DIR}"
        return 1
    fi

    if ! command -v git &>/dev/null; then
        log_warn "  git not found, cannot fetch ${repo}"
        return 1
    fi

    mkdir -p "${RAVEN_SRC_DIR}"

    if [[ -d "${dest}/.git" ]]; then
        log_info "  updating ${repo}..."
        # Unshallow-safe: --depth on fetch keeps shallow clones shallow.
        if ! (cd "${dest}" && git fetch --tags --depth 1 origin "${ref:-HEAD}" 2>/dev/null \
              && git reset --hard FETCH_HEAD >/dev/null 2>&1); then
            log_warn "  could not update ${repo}, using the existing checkout"
        fi
    else
        log_info "  cloning ${repo}..."
        rm -rf "${dest}"
        if [[ -n "${ref}" ]]; then
            git clone --depth 1 --branch "${ref}" -q "${url}" "${dest}" 2>/dev/null \
                || git clone --depth 1 -q "${url}" "${dest}" || return 1
        else
            git clone --depth 1 -q "${url}" "${dest}" || return 1
        fi
    fi

    local rev
    rev="$(cd "${dest}" && git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    log_info "  ${repo} @ ${rev}"
    return 0
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

# Install a staged binary into the sysroot, with the /bin compatibility
# symlink the rest of the base system uses.
install_component_binary() {
    local binary="$1" src="$2"

    mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/bin"
    install -m 0755 "${src}" "${SYSROOT_DIR}/usr/bin/${binary}"
    ln -sf "../usr/bin/${binary}" "${SYSROOT_DIR}/bin/${binary}"
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
# Summary
# =============================================================================
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

    # ravenshell config into /etc and /etc/skel, if configs/ravenshell exists
    install_raven_configs

    # Take over the default shell, but only if ravenshell actually landed
    set_ravenshell_default

    print_summary

    log_success "Raven stage complete!"
    echo ""
}

# Run main (whether executed directly or sourced)
main "$@"
