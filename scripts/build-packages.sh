#!/bin/bash
# =============================================================================
# RavenLinux Custom Packages Build Script
# =============================================================================
# Builds custom Go packages from GitHub for RavenLinux
#
# Usage: ./scripts/build-packages.sh [package-name|all]
#   vem        Build Vem text editor
#   carrion    Build Carrion programming language
#   ivaldi     Build Ivaldi VCS
#   all        Build all packages (default)

set -e

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="${PROJECT_ROOT}/build"
SOURCES_DIR="${BUILD_DIR}/sources"
OUTPUT_DIR="${BUILD_DIR}/packages"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check dependencies
check_dependencies() {
    log_info "Checking build dependencies..."

    local missing=()

    if ! command -v go &> /dev/null; then
        missing+=("go")
    fi

    if ! command -v git &> /dev/null; then
        missing+=("git")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        log_error "Missing dependencies: ${missing[*]}"
        exit 1
    fi

    log_success "All dependencies found (Go $(go version | awk '{print $3}'))"
}

# Build a Go package from GitHub
build_go_package() {
    local name="$1"
    local repo="$2"
    local binary="$3"
    local cgo="${4:-0}"

    log_info "Building ${name}..."

    local src_dir="${SOURCES_DIR}/${name}"

    # Clone or update repository
    if [ -d "$src_dir" ]; then
        log_info "Updating ${name} from GitHub..."
        cd "$src_dir"
        git fetch origin main
        git reset --hard origin/main
    else
        log_info "Cloning ${name} from GitHub..."
        git clone --depth 1 "https://github.com/${repo}.git" "$src_dir"
        cd "$src_dir"
    fi

    # Download Go dependencies
    log_info "Downloading dependencies for ${name}..."
    go mod download

    # Build
    log_info "Compiling ${name}..."
    CGO_ENABLED="$cgo" go build -o "${binary}" .

    # Copy to output
    mkdir -p "${OUTPUT_DIR}/bin"
    cp "${binary}" "${OUTPUT_DIR}/bin/"

    log_success "${name} built successfully -> ${OUTPUT_DIR}/bin/${binary}"
}

# Build Vem text editor
build_vem() {
    echo ""
    echo "=========================================="
    echo "  Building Vem Text Editor"
    echo "=========================================="

    # Vem requires CGO for Gio UI/Wayland support
    build_go_package "vem" "javanhut/Vem" "vem" "1"
}

# Build Carrion programming language
build_carrion() {
    echo ""
    echo "=========================================="
    echo "  Building Carrion Language"
    echo "=========================================="

    local name="carrion"
    local repo="javanhut/TheCarrionLanguage"
    local src_dir="${SOURCES_DIR}/${name}"

    log_info "Building ${name}..."

    # Clone or update repository
    if [ -d "$src_dir" ]; then
        log_info "Updating ${name} from GitHub..."
        cd "$src_dir"
        git fetch origin main
        git reset --hard origin/main
    else
        log_info "Cloning ${name} from GitHub..."
        git clone --depth 1 "https://github.com/${repo}.git" "$src_dir"
        cd "$src_dir"
    fi

    # Download Go dependencies
    log_info "Downloading dependencies for ${name}..."
    go mod download

    # Build - Carrion has main.go in src/ directory
    log_info "Compiling ${name}..."
    CGO_ENABLED=0 go build -o carrion ./src/main.go

    # Copy to output
    mkdir -p "${OUTPUT_DIR}/bin"
    cp carrion "${OUTPUT_DIR}/bin/"

    log_success "${name} built successfully -> ${OUTPUT_DIR}/bin/carrion"
}

# Build Ivaldi VCS
build_ivaldi() {
    echo ""
    echo "=========================================="
    echo "  Building Ivaldi VCS"
    echo "=========================================="

    build_go_package "ivaldi" "javanhut/IvaldiVCS" "ivaldi" "0"
}

# Build all packages
build_all() {
    build_vem
    build_carrion
    build_ivaldi
}

# Main
main() {
    echo ""
    echo "=========================================="
    echo "  RavenLinux Custom Packages Builder"
    echo "=========================================="
    echo ""

    mkdir -p "$SOURCES_DIR" "$OUTPUT_DIR"

    check_dependencies

    local target="${1:-all}"

    case "$target" in
        vem)
            build_vem
            ;;
        carrion)
            build_carrion
            ;;
        ivaldi)
            build_ivaldi
            ;;
        all)
            build_all
            ;;
        *)
            log_error "Unknown package: $target"
            echo "Usage: $0 [vem|carrion|ivaldi|all]"
            exit 1
            ;;
    esac

    echo ""
    echo "=========================================="
    echo "  Build Complete!"
    echo "=========================================="
    echo ""
    echo "Built packages are in: ${OUTPUT_DIR}/bin/"
    ls -lh "${OUTPUT_DIR}/bin/" 2>/dev/null || true
    echo ""
}

main "$@"
