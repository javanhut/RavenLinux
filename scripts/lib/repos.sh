#!/bin/bash
# =============================================================================
# RavenLinux Repo Fetch/Build Library
# =============================================================================
# Central library for cloning and building external repos at build time.
# All build scripts should source this file.
#
# Usage:
#   source scripts/lib/repos.sh
#   fetch_repo compositor
#   build_cargo_repo compositor
#
# Environment:
#   RAVEN_BUILD  - Build directory (default: <project_root>/build)

# Guard against double-sourcing
[[ -n "${_REPOS_SH_LOADED:-}" ]] && return 0
_REPOS_SH_LOADED=1

# =============================================================================
# Configuration
# =============================================================================

_REPOS_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_REPOS_PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$_REPOS_SCRIPT_DIR")")}"
export RAVEN_BUILD="${RAVEN_BUILD:-${_REPOS_PROJECT_ROOT}/build}"
_REPOS_SOURCES_DIR="${RAVEN_BUILD}/sources"
_REPOS_OUTPUT_DIR="${RAVEN_BUILD}/packages"

# =============================================================================
# Repo Registry
# =============================================================================
# Format: name -> "github_repo|build_type|binary_names|extra_args"
#   build_type: cargo, go-cgo0, go-cgo1
#   binary_names: comma-separated list of binaries produced. For cargo repos an
#                 entry may be "srcname:dstname" to install the built binary
#                 (srcname in target/release) under a different name (dstname).
#   extra_args: optional build target/args (e.g., ./src/main.go for carrion,
#               ./src for terminal, ./cmd/poxy for poxy). Defaults to '.' (repo
#               root) for go builds when empty.

declare -A REPO_REGISTRY=(
    [compositor]="javanhut/RavenCompositor|cargo|raven-compositor,raven-shell,raven-settings|"
    # Cargo [[bin]] is 'ravenfilemanager'; install it as 'raven-file-manager'
    # (the name the desktop keybind in hyprland-config.sh execs) via src:dst.
    [file-manager]="javanhut/RavenFileManager|cargo|ravenfilemanager:raven-file-manager|"
    # package main lives in ./src, not the repo root.
    [terminal]="javanhut/RavenTerminal|go-cgo1|raven-terminal|./src"
    [shell]="javanhut/RavenShell|go-cgo0|raven-shell-utils|"
    # package main lives in ./cmd/poxy, not the repo root.
    [poxy]="javanhut/Poxy|go-cgo0|poxy|./cmd/poxy"
    [vem]="javanhut/Vem|go-cgo1|vem|"
    [carrion]="javanhut/TheCarrionLanguage|go-cgo0|carrion|./src/main.go"
    [ivaldi]="javanhut/IvaldiVCS|go-cgo0|ivaldi|"
)

# =============================================================================
# Internal helpers
# =============================================================================

_repo_field() {
    local name="$1"
    local field="$2"
    local entry="${REPO_REGISTRY[$name]:-}"
    if [[ -z "$entry" ]]; then
        echo ""
        return 1
    fi
    echo "$entry" | cut -d'|' -f"$field"
}

# =============================================================================
# Public API
# =============================================================================

# get_repo_dir <name>
#   Returns the local source directory for a repo.
get_repo_dir() {
    local name="$1"
    echo "${_REPOS_SOURCES_DIR}/${name}"
}

# fetch_repo <name>
#   Clones the repo (shallow) or updates it if already present.
fetch_repo() {
    local name="$1"
    local repo_url
    repo_url="$(_repo_field "$name" 1)" || {
        echo "ERROR: Unknown repo '${name}'" >&2
        return 1
    }

    local src_dir
    src_dir="$(get_repo_dir "$name")"
    mkdir -p "${_REPOS_SOURCES_DIR}"

    if [[ -d "$src_dir/.git" ]]; then
        echo "[repos] Updating ${name}..."
        cd "$src_dir"
        git fetch origin
        git reset --hard origin/main
    else
        echo "[repos] Cloning ${name}..."
        rm -rf "$src_dir"
        git clone --depth 1 "https://github.com/${repo_url}.git" "$src_dir"
        cd "$src_dir"
    fi

    cd "${_REPOS_PROJECT_ROOT}"
}

# build_cargo_repo <name>
#   Builds a Rust/Cargo repo and copies binaries to output.
build_cargo_repo() {
    local name="$1"
    local binaries
    binaries="$(_repo_field "$name" 3)"

    local src_dir
    src_dir="$(get_repo_dir "$name")"

    if [[ ! -d "$src_dir" ]]; then
        echo "ERROR: Source not found for '${name}'. Run fetch_repo first." >&2
        return 1
    fi

    echo "[repos] Building ${name} (cargo)..."
    cd "$src_dir"
    cargo build --release

    mkdir -p "${_REPOS_OUTPUT_DIR}/bin"
    IFS=',' read -ra bins <<< "$binaries"
    for bin in "${bins[@]}"; do
        # Optional "srcname:dstname" — build produces srcname, install as dstname.
        local src="${bin%%:*}"
        local dst="${bin##*:}"
        if [[ -f "target/release/${src}" ]]; then
            cp "target/release/${src}" "${_REPOS_OUTPUT_DIR}/bin/${dst}"
            echo "[repos]   -> ${_REPOS_OUTPUT_DIR}/bin/${dst}"
        else
            echo "[repos]   WARNING: binary '${src}' not found in target/release/" >&2
        fi
    done

    cd "${_REPOS_PROJECT_ROOT}"
}

# build_go_repo <name>
#   Builds a Go repo and copies binaries to output.
#   CGO setting and build args are read from the registry.
build_go_repo() {
    local name="$1"
    local build_type binaries extra_args
    build_type="$(_repo_field "$name" 2)"
    binaries="$(_repo_field "$name" 3)"
    extra_args="$(_repo_field "$name" 4)"

    local src_dir
    src_dir="$(get_repo_dir "$name")"

    if [[ ! -d "$src_dir" ]]; then
        echo "ERROR: Source not found for '${name}'. Run fetch_repo first." >&2
        return 1
    fi

    local cgo="0"
    case "$build_type" in
        go-cgo1) cgo="1" ;;
        go-cgo0) cgo="0" ;;
    esac

    echo "[repos] Building ${name} (go, CGO_ENABLED=${cgo})..."
    cd "$src_dir"
    go mod download

    IFS=',' read -ra bins <<< "$binaries"
    local build_target="${extra_args:-.}"

    # If there's only one binary, build directly with -o
    if [[ ${#bins[@]} -eq 1 ]]; then
        env CGO_ENABLED="$cgo" go build -o "${bins[0]}" $build_target
        mkdir -p "${_REPOS_OUTPUT_DIR}/bin"
        cp "${bins[0]}" "${_REPOS_OUTPUT_DIR}/bin/"
        echo "[repos]   -> ${_REPOS_OUTPUT_DIR}/bin/${bins[0]}"
    else
        # Multiple binaries: build each
        for bin in "${bins[@]}"; do
            env CGO_ENABLED="$cgo" go build -o "${bin}" $build_target
            mkdir -p "${_REPOS_OUTPUT_DIR}/bin"
            cp "${bin}" "${_REPOS_OUTPUT_DIR}/bin/"
            echo "[repos]   -> ${_REPOS_OUTPUT_DIR}/bin/${bin}"
        done
    fi

    cd "${_REPOS_PROJECT_ROOT}"
}

# build_repo <name>
#   Auto-detects build type from registry and builds accordingly.
build_repo() {
    local name="$1"
    local build_type
    build_type="$(_repo_field "$name" 2)" || {
        echo "ERROR: Unknown repo '${name}'" >&2
        return 1
    }

    case "$build_type" in
        cargo)
            build_cargo_repo "$name"
            ;;
        go-cgo0|go-cgo1)
            build_go_repo "$name"
            ;;
        *)
            echo "ERROR: Unknown build type '${build_type}' for '${name}'" >&2
            return 1
            ;;
    esac
}

# list_repos
#   Lists all registered repos.
list_repos() {
    echo "Registered repos:"
    for name in "${!REPO_REGISTRY[@]}"; do
        local repo build_type binaries
        repo="$(_repo_field "$name" 1)"
        build_type="$(_repo_field "$name" 2)"
        binaries="$(_repo_field "$name" 3)"
        printf "  %-15s %-35s %-10s %s\n" "$name" "$repo" "$build_type" "$binaries"
    done
}
