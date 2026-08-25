#!/bin/bash
# =============================================================================
# RavenLinux QEMU Test Harness
# =============================================================================
# Boots a built ISO. Stage 4 has advertised this script in its "Next steps"
# output for a while; this is it.
#
# Two modes, because RavenLinux has two faces:
#
#   console   (default) serial console, no graphics. Works on a headless host
#             and captures all output as text, so it is the mode to use for a
#             smoke test or over SSH.
#   desktop   a real display with a virtio GPU, for the Wayland session.
#             Huginn's udev/DRM backend needs a DRM device it can take master
#             on -- QEMU's default emulated VGA does not provide one, so this
#             mode passes virtio-vga-gl and turns on the host GL backend.
#
# Usage:
#   ./scripts/test-qemu.sh                 # console boot
#   ./scripts/test-qemu.sh --desktop       # Wayland session
#   ./scripts/test-qemu.sh --uefi          # boot through OVMF instead of BIOS
#   ./scripts/test-qemu.sh --timeout 120   # quit after N seconds (smoke test)
#   ./scripts/test-qemu.sh --iso path.iso  # a specific image
#
# Environment:
#   RAVEN_QEMU_MEM=2G       guest memory
#   RAVEN_QEMU_CPUS=2       guest cpus
#   RAVEN_QEMU_EXTRA="..."  extra flags appended verbatim
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    log_info()    { echo "[INFO] $*"; }
    log_warn()    { echo "[WARN] $*" >&2; }
    log_error()   { echo "[ERROR] $*" >&2; }
    log_success() { echo "[SUCCESS] $*"; }
    log_step()    { echo ""; echo "==> $*"; }
fi

MODE="console"
FIRMWARE="bios"
TIMEOUT=""
ISO=""
MEM="${RAVEN_QEMU_MEM:-2G}"
CPUS="${RAVEN_QEMU_CPUS:-2}"

show_help() {
    sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --desktop|--gui)  MODE="desktop"; shift ;;
        --console)        MODE="console"; shift ;;
        --uefi|--efi)     FIRMWARE="uefi"; shift ;;
        --bios)           FIRMWARE="bios"; shift ;;
        --timeout)        TIMEOUT="$2"; shift 2 ;;
        --iso)            ISO="$2"; shift 2 ;;
        -h|--help)        show_help; exit 0 ;;
        *) log_error "Unknown option: $1"; show_help; exit 1 ;;
    esac
done

# =============================================================================
# Preflight
# =============================================================================
QEMU="qemu-system-x86_64"
command -v "${QEMU}" &>/dev/null || {
    log_error "${QEMU} not found"
    log_info "  Arch:   pacman -S qemu-base qemu-system-x86-firmware"
    log_info "  Debian: apt install qemu-system-x86 ovmf"
    exit 1
}

# Find the ISO. Stage 4 writes it to the repo root (the container bind mount),
# not into build/, which is a common place to go looking for it.
if [[ -z "${ISO}" ]]; then
    ISO="$(find "${PROJECT_ROOT}" -maxdepth 2 -name 'raven-*.iso' -type f -print0 2>/dev/null \
           | xargs -0 -r ls -t 2>/dev/null | head -1)"
fi

[[ -n "${ISO}" && -f "${ISO}" ]] || {
    log_error "No ISO found. Build one with 'make build' (or pass --iso PATH)."
    exit 1
}

QEMU_ARGS=(
    -cdrom "${ISO}"
    -m "${MEM}"
    -smp "${CPUS}"
    -boot d
)

# KVM or software emulation. Without /dev/kvm every instruction is interpreted,
# which still boots but takes minutes rather than seconds -- worth saying out
# loud so a slow boot does not read as a hang.
if [[ -w /dev/kvm ]]; then
    QEMU_ARGS+=(-enable-kvm -cpu host)
    ACCEL="KVM"
else
    QEMU_ARGS+=(-accel tcg)
    ACCEL="TCG (software emulation -- slow)"
    if [[ -e /dev/kvm ]]; then
        log_warn "/dev/kvm exists but is not writable by $(id -un); falling back to software emulation"
        log_info "  fix with: sudo usermod -aG kvm $(id -un)   (then log out and back in)"
    else
        log_warn "No /dev/kvm; falling back to software emulation"
        log_info "  load the module with: sudo modprobe kvm_intel   (or kvm_amd)"
    fi
fi

# UEFI needs the firmware image. Arch splits it across two paths depending on
# the package version, and Debian uses a third.
if [[ "${FIRMWARE}" == "uefi" ]]; then
    OVMF=""
    for candidate in \
        /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd \
        /usr/share/edk2/x64/OVMF_CODE.4m.fd \
        /usr/share/OVMF/OVMF_CODE.fd \
        /usr/share/ovmf/OVMF.fd
    do
        [[ -f "${candidate}" ]] && { OVMF="${candidate}"; break; }
    done

    [[ -n "${OVMF}" ]] || {
        log_error "UEFI requested but no OVMF firmware found"
        log_info "  Arch:   pacman -S edk2-ovmf"
        log_info "  Debian: apt install ovmf"
        exit 1
    }
    QEMU_ARGS+=(-drive "if=pflash,format=raw,readonly=on,file=${OVMF}")
fi

# =============================================================================
# Mode
# =============================================================================
case "${MODE}" in
    console)
        # -nographic wires the guest's serial port to this terminal. The live
        # init reads raven.console=serial off the cmdline and puts its shell
        # there, so the whole boot is visible as text.
        QEMU_ARGS+=(
            -nographic
            -serial mon:stdio
        )
        log_info "Console mode: quit with Ctrl-a x"
        # RavenBoot's default entry uses console=tty0, whose output goes to the
        # (absent) VGA console rather than here -- so the serial log goes quiet
        # right after "Booting:". The Serial entry is the one wired to ttyS0.
        log_info "  Pick 'Raven Linux (Serial)' in the menu to see kernel output here;"
        log_info "  the default entry logs to tty0 and will look like it hung."
        ;;

    desktop)
        # A display backend is a separate package on most distributions, and
        # a QEMU built without one reports only "none" here. Checking up front
        # turns an inscrutable runtime failure into an install instruction.
        # `-display help` prints a header, the backend names one per line, then
        # a blank line and prose. Take only the names.
        AVAILABLE="$(${QEMU} -display help 2>/dev/null \
                     | awk '/^Available display backend types:/{f=1;next} f&&NF==0{exit} f{print $1}')"
        DISPLAY_BACKEND=""
        for backend in gtk sdl; do
            grep -qx "${backend}" <<< "${AVAILABLE}" && { DISPLAY_BACKEND="${backend}"; break; }
        done

        [[ -n "${DISPLAY_BACKEND}" ]] || {
            log_error "This QEMU has no graphical display backend (has: $(tr '\n' ' ' <<< "${AVAILABLE}"))"
            log_info "  Arch:   pacman -S qemu-ui-gtk qemu-ui-opengl"
            log_info "  Debian: apt install qemu-system-gui"
            log_info ""
            log_info "  Without one, only console mode can run:"
            log_info "    ${BASH_SOURCE[0]} --console"
            exit 1
        }

        # virtio-vga-gl gives the guest a real DRM device with a virgl driver
        # behind it -- Huginn's udev backend needs to take DRM master on
        # something, and QEMU's default emulated VGA offers nothing to bind.
        # Mesa's virtio_gpu DRI driver is staged into the sysroot by stage2.
        QEMU_ARGS+=(
            -device virtio-vga-gl
            -display "${DISPLAY_BACKEND},gl=on"
            -device virtio-tablet-pci
            -device virtio-keyboard-pci
            -serial mon:stdio
        )
        log_info "Desktop mode: display=${DISPLAY_BACKEND} with GL"
        log_info "  Pick 'Raven Desktop (Huginn)' in the boot menu -- the default entry is the console."
        ;;
esac

[[ -n "${RAVEN_QEMU_EXTRA:-}" ]] && read -r -a extra <<< "${RAVEN_QEMU_EXTRA}" && QEMU_ARGS+=("${extra[@]}")

# =============================================================================
# Run
# =============================================================================
log_step "Booting $(basename "${ISO}") ($(du -h "${ISO}" | cut -f1))"
log_info "  mode:  ${MODE} / ${FIRMWARE}"
log_info "  accel: ${ACCEL}"
log_info "  guest: ${MEM} RAM, ${CPUS} cpu(s)"
echo ""

if [[ -n "${TIMEOUT}" ]]; then
    log_info "Stopping after ${TIMEOUT}s"
    # SIGTERM first so QEMU shuts down cleanly; -k escalates if it ignores it.
    timeout -k 5 "${TIMEOUT}" "${QEMU}" "${QEMU_ARGS[@]}" || {
        rc=$?
        # 124 is timeout's "deadline reached", which is the expected outcome of
        # a timed smoke test rather than a failure.
        [[ $rc -eq 124 ]] && { echo ""; log_success "Reached the ${TIMEOUT}s deadline"; exit 0; }
        exit $rc
    }
else
    exec "${QEMU}" "${QEMU_ARGS[@]}"
fi
