#!/bin/bash
# =============================================================================
# build-evdi.sh -- out-of-tree DisplayLink kernel module for the Raven kernel
# =============================================================================
#
# DisplayLink adapters come in two generations. DL-1x5 (USB 2, up to 2010ish)
# are driven in tree by udl, which configs/kernel enables. Everything since --
# DL-3xxx, DL-5xxx, DL-6xxx, i.e. every USB 3 and USB-C DisplayLink dock --
# talks a proprietary protocol that the in-tree driver does not speak. Its
# kernel half is evdi, a GPL virtual-DRM module maintained by DisplayLink,
# which creates one /dev/dri/cardN per attached screen and hands the pixels to
# the userspace DisplayLinkManager. That daemon is proprietary and under an
# EULA that forbids redistribution, so it is not in the image; this ships the
# module so that installing the daemon is the only step left on the machine.
#
# Builds against the kernel tree build-kernel.sh left in build/sources and
# installs into the same module output directory, so stage4 picks it up with
# the rest of /lib/modules. Run after build-kernel.sh; stage1 does.
#
# Usage: scripts/build-evdi.sh [--clean]
#
# Environment:
#   RAVEN_EVDI_VERSION   git tag to build (default: v1.14.16)
#   RAVEN_OFFLINE=1      fail rather than clone when the source is absent
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="${PROJECT_ROOT}/build"
SOURCES_DIR="${BUILD_DIR}/sources"
OUTPUT_DIR="${BUILD_DIR}/kernel"

KERNEL_FULL_VERSION="6.17.11"
KERNEL_BUILD_DIR="${SOURCES_DIR}/linux-${KERNEL_FULL_VERSION}"

EVDI_VERSION="${RAVEN_EVDI_VERSION:-v1.14.16}"
EVDI_URL="https://github.com/DisplayLink/evdi.git"
EVDI_DIR="${SOURCES_DIR}/evdi-${EVDI_VERSION}"

log_info()    { echo -e "\033[0;34m[INFO]\033[0m $*"; }
log_warn()    { echo -e "\033[1;33m[WARN]\033[0m $*"; }
log_error()   { echo -e "\033[0;31m[ERROR]\033[0m $*"; }
log_success() { echo -e "\033[0;32m[OK]\033[0m $*"; }

if [[ "${1:-}" == "--clean" ]]; then
    rm -rf "${EVDI_DIR}"
    log_success "Removed ${EVDI_DIR}"
    exit 0
fi

# The kernel tree has to be configured and built: an external module needs
# Module.symvers and the generated headers, both of which only a compiled tree
# has. build-kernel.sh --config-only is not enough.
if [[ ! -f "${KERNEL_BUILD_DIR}/Module.symvers" ]]; then
    log_error "No built kernel at ${KERNEL_BUILD_DIR}; run scripts/build-kernel.sh first"
    exit 1
fi

release="$(make -s -C "${KERNEL_BUILD_DIR}" kernelrelease 2>/dev/null)"
if [[ -z "${release}" ]]; then
    log_error "Could not read kernelrelease from ${KERNEL_BUILD_DIR}"
    exit 1
fi

if [[ ! -d "${EVDI_DIR}/module" ]]; then
    if [[ "${RAVEN_OFFLINE:-0}" == "1" ]]; then
        log_error "RAVEN_OFFLINE=1 and no evdi source at ${EVDI_DIR}"
        exit 1
    fi
    log_info "Fetching evdi ${EVDI_VERSION}..."
    mkdir -p "${SOURCES_DIR}"
    rm -rf "${EVDI_DIR}"
    git clone --quiet --depth 1 --branch "${EVDI_VERSION}" "${EVDI_URL}" "${EVDI_DIR}"
fi

log_info "Building evdi ${EVDI_VERSION} against ${release}..."
# evdi's own Makefile wraps this same invocation and adds its version defines;
# calling the kernel's build system directly with the module dir as M= is what
# it does underneath, minus its host-kernel autodetection, which is wrong here
# because the target kernel is not the one running the build.
make -C "${KERNEL_BUILD_DIR}" M="${EVDI_DIR}/module" \
    EVDI_VERSION="${EVDI_VERSION#v}" modules

modules_dir="${OUTPUT_DIR}/lib/modules/${release}"
if [[ ! -d "${modules_dir}" ]]; then
    log_error "Kernel modules not installed at ${modules_dir}; run scripts/build-kernel.sh"
    exit 1
fi

mkdir -p "${modules_dir}/extra"
cp "${EVDI_DIR}/module/evdi.ko" "${modules_dir}/extra/evdi.ko"

# stage4 runs depmod over the sysroot again; this keeps build/kernel usable
# on its own (the QEMU path boots it directly).
if command -v depmod >/dev/null 2>&1; then
    depmod -b "${OUTPUT_DIR}" "${release}" 2>/dev/null || true
fi

log_success "evdi installed to ${modules_dir}/extra/evdi.ko"
