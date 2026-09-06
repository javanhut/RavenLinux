#!/bin/bash
# =============================================================================
# RavenLinux QEMU Test Harness
# =============================================================================
# Boots a built ISO. Stage 4 has advertised this script in its "Next steps"
# output for a while; this is it.
#
# Three modes, because RavenLinux has three faces:
#
#   console    (default) serial console, no graphics. Works on a headless host
#              and captures all output as text, so it is the mode to use for a
#              smoke test or over SSH.
#   desktop    a real display with a virtio GPU, for the Wayland session.
#              Huginn's udev/DRM backend needs a DRM device it can take master
#              on -- QEMU's default emulated VGA does not provide one, so this
#              mode passes virtio-vga-gl and turns on the host GL backend.
#   bootloader RavenBoot on its own, with no ISO and no kernel. Boots the .efi
#              off a throwaway ESP so the boot menu can be looked at, at a
#              chosen resolution. Implies --uefi: RavenBoot is a UEFI
#              application and BIOS cannot load it.
#
#              --resolution drives which rung of the menu's scale ladder is
#              exercised. OVMF honours 1920x1080, 2048x2048 and 2560x1440 here
#              and refuses 3840x2160, falling back to 1280x800 in silence -- so
#              2048x2048 is the one to use for the 2.0 scale.
#
# Usage:
#   ./scripts/test-qemu.sh                        # console boot
#   ./scripts/test-qemu.sh --desktop              # Wayland session
#   ./scripts/test-qemu.sh --bootloader           # just the boot menu
#   ./scripts/test-qemu.sh --bootloader --resolution 3840x2160
#   ./scripts/test-qemu.sh --uefi                 # boot the ISO through OVMF
#   ./scripts/test-qemu.sh --timeout 120          # quit after N seconds
#   ./scripts/test-qemu.sh --iso path.iso         # a specific image
#   ./scripts/test-qemu.sh --bootloader --efi path/to/raven-boot.efi
#   ./scripts/test-qemu.sh --bootloader --screenshot menu.png [--at 20]
#
# --screenshot boots headless and photographs the panel through the QEMU
# monitor, so it needs no display backend and works over SSH. --at is how many
# seconds to wait first; the default suits software emulation, and with KVM the
# menu is up in about two.
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
EFI_APP=""
SCREENSHOT=""
SHOT_AT="20"
# The panel RavenBoot's `scale_for` is asked to reason about. 1080p is the size
# every metric in its theme is written against, so it is the default; the point
# of being able to change it is to check the other two rungs of the ladder.
RESOLUTION="1920x1080"
MEM="${RAVEN_QEMU_MEM:-2G}"
CPUS="${RAVEN_QEMU_CPUS:-2}"

show_help() {
    sed -n '2,49p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --desktop|--gui)  MODE="desktop"; shift ;;
        --console)        MODE="console"; shift ;;
        # BIOS cannot load a UEFI application, so this is not a default the
        # user can be left to discover by getting a blank screen.
        --bootloader|--boot-menu) MODE="bootloader"; FIRMWARE="uefi"; shift ;;
        --efi)            EFI_APP="$2"; shift 2 ;;
        --resolution)     RESOLUTION="$2"; shift 2 ;;
        --screenshot)     SCREENSHOT="$2"; shift 2 ;;
        --at)             SHOT_AT="$2"; shift 2 ;;
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

# Split WxH. Validated rather than trusted: a typo here reaches QEMU as a device
# property and comes back as an unhelpful "Property 'VGA.xres' not found"-shaped
# failure, long after the point where it could be explained.
if [[ "${RESOLUTION}" =~ ^([0-9]+)[xX]([0-9]+)$ ]]; then
    RES_X="${BASH_REMATCH[1]}"
    RES_Y="${BASH_REMATCH[2]}"
else
    log_error "Bad --resolution '${RESOLUTION}' (expected WIDTHxHEIGHT, e.g. 1920x1080)"
    exit 1
fi

# std VGA ships 16MB of video memory, which is not enough for every mode worth
# asking for (2560x1600 needs 16.4MB). Size it to the mode. The property takes
# powers of two, up to 256.
VGAMEM=16
VGAMEM_NEEDED=$(( RES_X * RES_Y * 4 / 1048576 + 1 ))
while (( VGAMEM < VGAMEM_NEEDED && VGAMEM < 256 )); do
    VGAMEM=$(( VGAMEM * 2 ))
done

# Video memory is not the only ceiling, and the other one cannot be raised from
# here: OVMF picks the mode from its own table, and a request it does not
# recognise is not an error -- it silently uses its default of 1280x800, so the
# screen looks perfectly plausible and is simply not what was asked for.
#
# Measured against edk2-ovmf on this machine, not read off a spec: 1920x1080,
# 2048x2048 and 2560x1440 are honoured; 3840x2160 is refused, and stays refused
# with 64MB of video memory and with bochs-display in place of std VGA. So this
# is OVMF's limit rather than QEMU's, and there is nothing to pass to fix it.
#
# 2048x2048 is the useful one to know about: it is over the 1800px threshold
# where RavenBoot's `scale_for` switches to 2.0, so it exercises the top of the
# scale ladder without needing a mode OVMF will refuse.
if (( RES_X > 2560 || RES_Y > 2048 )); then
    log_warn "OVMF may refuse ${RES_X}x${RES_Y} and fall back to 1280x800 without saying so"
    log_info "  If the menu looks smaller than expected, that is what happened."
    log_info "  For the 2.0 scale rung, use --resolution 2048x2048 instead."
fi

if [[ -n "${SCREENSHOT}" ]]; then
    [[ "${MODE}" == "bootloader" ]] || {
        log_error "--screenshot is only implemented for --bootloader"
        exit 1
    }
    [[ "${SHOT_AT}" =~ ^[0-9]+$ ]] || {
        log_error "Bad --at '${SHOT_AT}' (expected a whole number of seconds)"
        exit 1
    }
fi

QEMU="qemu-system-x86_64"
command -v "${QEMU}" &>/dev/null || {
    log_error "${QEMU} not found"
    log_info "  Arch:   pacman -S qemu-base qemu-system-x86-firmware"
    log_info "  Debian: apt install qemu-system-x86 ovmf"
    exit 1
}

QEMU_ARGS=(
    -m "${MEM}"
    -smp "${CPUS}"
)

if [[ "${MODE}" == "bootloader" ]]; then
    # Find the bootloader. stage3 copies it to build/packages/boot; a plain
    # `cargo build` in bootloader/ leaves it under that crate's target/. Take
    # whichever is newer, because the usual reason to be running this at all is
    # that one of the two was just rebuilt.
    #
    # A loop and not `ls -t ... | head -1`: `set -o pipefail` is in force and
    # `ls` exits non-zero when any argument is missing, which is the normal case
    # here since almost nobody has both. The pipeline would fail the assignment
    # and errexit would kill the script before it could say anything useful.
    if [[ -z "${EFI_APP}" ]]; then
        for candidate in \
            "${PROJECT_ROOT}/build/packages/boot/raven-boot.efi" \
            "${PROJECT_ROOT}/bootloader/target/x86_64-unknown-uefi/release/raven-boot.efi"
        do
            [[ -f "${candidate}" ]] || continue
            if [[ -z "${EFI_APP}" || "${candidate}" -nt "${EFI_APP}" ]]; then
                EFI_APP="${candidate}"
            fi
        done
    fi

    [[ -n "${EFI_APP}" && -f "${EFI_APP}" ]] || {
        log_error "No raven-boot.efi found (pass one with --efi PATH)"
        log_info "  Build it with:"
        log_info "    cd bootloader && cargo build --release --target x86_64-unknown-uefi"
        log_info "  If that fails with \"can't find crate for \`core\`\", the target is missing:"
        log_info "    rustup target add x86_64-unknown-uefi"
        exit 1
    }

    # A directory, not an image. QEMU's vvfat driver presents one as a FAT
    # volume with an MBR in front of it, which is all OVMF needs to find
    # \EFI\BOOT\BOOTX64.EFI -- and it needs no mkfs.vfat, no mtools, no loop
    # device and no root, none of which are a given on a machine that has just
    # been handed a .efi to look at.
    ESP_DIR="${PROJECT_ROOT}/build/qemu/bootmenu-esp"
    rm -rf "${ESP_DIR}"
    mkdir -p "${ESP_DIR}/EFI/BOOT"
    cp "${EFI_APP}" "${ESP_DIR}/EFI/BOOT/BOOTX64.EFI"

    # No kernel and no initrd are staged alongside it, on purpose. RavenBoot's
    # built-in defaults point at \EFI\raven\vmlinuz, so every entry fails to
    # boot -- which exercises the menu's failure line and returns you to the
    # menu, rather than ending the session. What is being tested here is the
    # menu, not the loading.
    QEMU_ARGS+=(-drive "file=fat:rw:${ESP_DIR},format=raw")
else
    # Find the ISO. Stage 4 writes it to the repo root (the container bind
    # mount), not into build/, which is a common place to go looking for it.
    if [[ -z "${ISO}" ]]; then
        ISO="$(find "${PROJECT_ROOT}" -maxdepth 2 -name 'raven-*.iso' -type f -print0 2>/dev/null \
               | xargs -0 -r ls -t 2>/dev/null | head -1)"
    fi

    [[ -n "${ISO}" && -f "${ISO}" ]] || {
        log_error "No ISO found. Build one with 'imlazy build' (or pass --iso PATH)."
        exit 1
    }

    QEMU_ARGS+=(
        -cdrom "${ISO}"
        -boot d
    )
fi

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

        # Distinguish "the module is not loaded" from "the CPU is not offering
        # virtualisation at all". They look identical from QEMU but have
        # completely different fixes, and modprobe cannot help with the second:
        # the vendor module refuses to load while the flag is absent.
        if grep -qw vmx /proc/cpuinfo 2>/dev/null; then
            log_info "  load the module with: sudo modprobe kvm_intel"
        elif grep -qw svm /proc/cpuinfo 2>/dev/null; then
            log_info "  load the module with: sudo modprobe kvm_amd"
        else
            vendor=""
            vendor="$(awk -F': ' '/^vendor_id/{print $2; exit}' /proc/cpuinfo 2>/dev/null)"
            case "${vendor}" in
                AuthenticAMD) log_info "  This CPU reports no 'svm' flag: AMD-V is disabled in firmware." ;;
                GenuineIntel) log_info "  This CPU reports no 'vmx' flag: VT-x is disabled in firmware." ;;
                *)            log_info "  This CPU reports no virtualisation flag." ;;
            esac
            log_info "  Enable it in the BIOS/UEFI setup (usually Advanced -> CPU Configuration,"
            log_info "  called 'SVM Mode' on AMD or 'Intel Virtualization Technology' on Intel),"
            log_info "  then reboot. No modprobe can work until that flag appears."
        fi
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

    # The variable store, if this OVMF ships one. It has to be writable and it
    # has to be per-run, so it is copied rather than referenced: the packaged
    # file is read-only and shared, and a firmware that cannot write its
    # variables will not remember a boot order -- or, for RavenBoot's "System
    # UEFI Settings" entry, act on the OsIndications it just set.
    #
    # Optional because a missing varstore is not fatal: OVMF falls back to
    # emulated non-volatile storage and boots the removable media path anyway,
    # which is the path both of the modes here rely on.
    OVMF_VARS_SRC=""
    for candidate in "${OVMF/OVMF_CODE/OVMF_VARS}" \
        /usr/share/edk2-ovmf/x64/OVMF_VARS.4m.fd \
        /usr/share/edk2/x64/OVMF_VARS.4m.fd \
        /usr/share/OVMF/OVMF_VARS.fd
    do
        # The substitution above is a no-op on a path that does not spell
        # OVMF_CODE -- Debian's /usr/share/ovmf/OVMF.fd, for one -- and would
        # then name the code image itself. Handing OVMF its own firmware as a
        # variable store is worse than handing it none.
        [[ "${candidate}" != "${OVMF}" && -f "${candidate}" ]] || continue
        OVMF_VARS_SRC="${candidate}"
        break
    done

    if [[ -n "${OVMF_VARS_SRC}" ]]; then
        OVMF_VARS="${PROJECT_ROOT}/build/qemu/OVMF_VARS.fd"
        mkdir -p "$(dirname "${OVMF_VARS}")"
        cp -f "${OVMF_VARS_SRC}" "${OVMF_VARS}"
        chmod u+w "${OVMF_VARS}"
        QEMU_ARGS+=(-drive "if=pflash,format=raw,file=${OVMF_VARS}")
    else
        log_warn "No OVMF_VARS found next to ${OVMF}; the firmware will not persist variables"
    fi
fi

# =============================================================================
# Mode
# =============================================================================

# A display backend is a separate package on most distributions, and a QEMU
# built without one reports only "none" here. Checking up front turns an
# inscrutable runtime failure into an install instruction. `-display help`
# prints a header, the backend names one per line, then a blank line and prose.
# Take only the names.
pick_display_backend() {
    local available
    available="$(${QEMU} -display help 2>/dev/null \
                 | awk '/^Available display backend types:/{f=1;next} f&&NF==0{exit} f{print $1}')"
    DISPLAY_BACKEND=""
    for backend in gtk sdl; do
        grep -qx "${backend}" <<< "${available}" && { DISPLAY_BACKEND="${backend}"; break; }
    done

    [[ -n "${DISPLAY_BACKEND}" ]] || {
        log_error "This QEMU has no graphical display backend (has: $(tr '\n' ' ' <<< "${available}"))"
        log_info "  Arch:   pacman -S qemu-ui-gtk qemu-ui-opengl"
        log_info "  Debian: apt install qemu-system-gui"
        log_info ""
        log_info "  Without one, only console mode can run:"
        log_info "    ${BASH_SOURCE[0]} --console"
        exit 1
    }
}

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

    bootloader)
        # A screenshot needs no window, which is the point: a QEMU built without
        # a UI backend -- the default on a server, and on any host that installed
        # only qemu-base -- can still produce one, and so can a session over SSH.
        # The monitor goes on stdio so the capture can be driven by writing HMP
        # commands to it, with no QMP socket and no Python in the way.
        if [[ -n "${SCREENSHOT}" ]]; then
            QEMU_ARGS+=(-display none -monitor stdio)
        else
            pick_display_backend
            QEMU_ARGS+=(-display "${DISPLAY_BACKEND}" -serial mon:stdio)
        fi

        # -vga none plus an explicit VGA device, because xres/yres are
        # properties of the device and `-vga std` takes none. OVMF reads them
        # and picks that mode for its GOP, which is the whole reason this mode
        # can answer "what does the menu look like on a 4K panel" without a 4K
        # panel.
        #
        # Plain VGA rather than the desktop mode's virtio-vga-gl: that exists so
        # Huginn can take DRM master, and nothing here is a kernel.
        QEMU_ARGS+=(
            -vga none
            -device "VGA,xres=${RES_X},yres=${RES_Y},vgamem_mb=${VGAMEM}"
        )
        if [[ -n "${SCREENSHOT}" ]]; then
            log_info "Boot menu mode: ${RES_X}x${RES_Y}, ${VGAMEM}MB vgamem, screenshot at ${SHOT_AT}s"
        else
            log_info "Boot menu mode: ${RES_X}x${RES_Y}, ${VGAMEM}MB vgamem, display=${DISPLAY_BACKEND}"
        fi
        log_info "  Every entry will fail to boot -- there is no kernel on this ESP."
        log_info "  That is the point: the menu is what is under test."
        ;;

    desktop)
        pick_display_backend

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
if [[ "${MODE}" == "bootloader" ]]; then
    log_step "Booting $(basename "${EFI_APP}") ($(du -h "${EFI_APP}" | cut -f1))"
    log_info "  from:  ${EFI_APP}"
else
    log_step "Booting $(basename "${ISO}") ($(du -h "${ISO}" | cut -f1))"
fi
log_info "  mode:  ${MODE} / ${FIRMWARE}"
log_info "  accel: ${ACCEL}"
log_info "  guest: ${MEM} RAM, ${CPUS} cpu(s)"
echo ""

if [[ -n "${SCREENSHOT}" ]]; then
    mkdir -p "$(dirname "${SCREENSHOT}")"
    rm -f "${SCREENSHOT}"

    # HMP commands, fed to the monitor on stdin. The waits are wall clock and
    # not a handshake, because there is nothing in the guest to hand shake with
    # -- RavenBoot does not know it is being photographed. 20s is comfortable
    # for OVMF plus RavenBoot under TCG; with KVM the menu is up in about two,
    # so --at can come right down when /dev/kvm is available.
    #
    # The monitor's own output is dropped: it echoes every keystroke it is fed,
    # which is unreadable and says nothing.
    {
        sleep "${SHOT_AT}"
        echo "screendump ${SCREENSHOT} -f png"
        sleep 3
        echo "quit"
    } | "${QEMU}" "${QEMU_ARGS[@]}" >/dev/null 2>&1 || true

    if [[ -s "${SCREENSHOT}" ]]; then
        log_success "Wrote ${SCREENSHOT}"
        # The capture always succeeds once the monitor is up -- it photographs
        # whatever is on the panel. Too small an --at therefore does not fail,
        # it returns a picture of the firmware's own splash, which is a
        # confusing thing to be handed without warning.
        log_info "  If that is OVMF's screen rather than the menu, raise --at."
    else
        log_error "No screenshot was produced"
        log_info "  The monitor did not run: check that ${QEMU} started at all."
        exit 1
    fi
elif [[ -n "${TIMEOUT}" ]]; then
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
