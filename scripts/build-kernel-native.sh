#!/bin/bash
# =============================================================================
# build-kernel-native.sh -- build and install the Raven kernel on Raven itself
# =============================================================================
#
# The image pipeline builds the kernel inside an Arch container
# (scripts/docker-build.sh), where every build tool is part of the image.
# A Raven machine is not that container. It has whatever its install profile
# brought, it has no pacman workflow, and it boots its kernel from the ESP
# rather than from a squashfs. This is the path for that machine: rebuild
# the kernel it is running (to pick up a config change such as
# scripts/kernel-virtualization.sh) without building the whole OS.
#
# It does not compile anything itself. The kernel is built by the same
# scripts/build-kernel.sh and the DisplayLink module by the same
# scripts/build-evdi.sh that stage1 runs, so a native kernel is the image's
# kernel, config floors and all. What this adds is the part a running system
# needs around them:
#
#   deps      install the missing build tools with rvn, and record exactly
#             which packages that added, transitive dependencies included
#   build     build-kernel.sh --clean, build-kernel.sh, build-evdi.sh, all
#             as the invoking user: nothing is compiled as root
#   install   back up the kernel and modules that are running now, then
#             install the new ones onto the ESP, /boot and /usr/lib/modules,
#             and add a boot entry for the previous kernel
#   cleanup   uninstall the packages `deps` added (only those) and delete
#             the kernel sources and the staged build output
#
# Usage:
#   scripts/build-kernel-native.sh              all four phases, in order
#   scripts/build-kernel-native.sh --no-install deps, build, stop: the kernel
#                                               stays staged in build/kernel
#   scripts/build-kernel-native.sh --no-cleanup keep deps and sources
#   scripts/build-kernel-native.sh --cleanup    only the cleanup phase
#   scripts/build-kernel-native.sh --rollback   put the backed-up kernel back
#   scripts/build-kernel-native.sh --jobs N     parallel make jobs
#
# A failed phase stops the run and leaves everything in place: the packages
# stay installed and the tree stays built, so a rerun picks up where it
# stopped. `--cleanup` removes them when you are done retrying.
#
# Privilege: run it as your own user. rvn, the install phase and the cleanup
# of root-owned files go through sudo; the build does not.
#
# The initramfs is not rebuilt. It holds no kernel modules -- everything on
# the path to the root filesystem is built in (see build-initramfs.sh,
# raven_wait_for_devices) -- so the one on the ESP is correct for any kernel
# built from configs/kernel/config-6.17-raven.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="${PROJECT_ROOT}/build"
STATE_DIR="${BUILD_DIR}/kernel-native"
KERNEL_FULL_VERSION="6.17.11"
KERNEL_TREE="${BUILD_DIR}/sources/linux-${KERNEL_FULL_VERSION}"
STAGED="${BUILD_DIR}/kernel"

ESP="${RAVEN_ESP:-/boot/efi}"
ESP_DIR="${ESP}/EFI/raven"
BOOT_DIR="/boot"
MODULES_ROOT="/usr/lib/modules"
BACKUP_ROOT="/var/lib/raven/kernel-backup"
# Backups kept after an install; older ones are deleted. Each one holds a
# modules directory, a few hundred megabytes.
KEEP_BACKUPS=3

# Free space the build needs under build/: about 1.5 GB of extracted source
# and 2-3 GB of objects, with no debug info in this config.
MIN_FREE_GB=8

export RAVEN_BUILD="${BUILD_DIR}"
# shellcheck source=lib/logging.sh
source "${SCRIPT_DIR}/lib/logging.sh"

DO_DEPS=true
DO_BUILD=true
DO_INSTALL=true
DO_CLEANUP=true
DO_ROLLBACK=false
JOBS="$(nproc)"

usage() {
    sed -n '/^# Usage:/,/^# A failed phase/p' "$0" | sed '$d; s/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        # Cleanup would delete the staged kernel this exists to keep.
        --no-install) DO_INSTALL=false; DO_CLEANUP=false ;;
        --no-cleanup) DO_CLEANUP=false ;;
        --cleanup)    DO_DEPS=false; DO_BUILD=false; DO_INSTALL=false ;;
        --rollback)   DO_ROLLBACK=true ;;
        --jobs)       JOBS="${2:?--jobs needs a number}"; shift ;;
        -h|--help)    usage; exit 0 ;;
        *)            log_error "Unknown option: $1"; usage; exit 1 ;;
    esac
    shift
done

if [[ $EUID -eq 0 ]]; then
    SUDO=()
else
    SUDO=(sudo)
fi

# =============================================================================
# Build dependencies
# =============================================================================
# What build-kernel.sh and build-evdi.sh run, as "probe:package". A probe
# starting with / is a file that must exist (headers have no command to look
# for); anything else is a command. Package names are Arch's, which is what
# rvn installs -- the same names check-deps.sh uses in its Arch column.
KERNEL_DEPS=(
    "gcc:gcc"
    "make:make"
    "bc:bc"
    "flex:flex"
    "bison:bison"
    "perl:perl"
    "pkg-config:pkgconf"
    "ld:binutils"
    "strip:binutils"
    "objcopy:binutils"
    "awk:gawk"
    "sed:sed"
    "find:findutils"
    "xargs:findutils"
    "diff:diffutils"
    "tar:tar"
    "xz:xz"
    "zstd:zstd"
    "gzip:gzip"
    "cpio:cpio"
    "depmod:kmod"
    "curl:curl"
    "git:git"
    "/usr/include/libelf.h:libelf"
    "/usr/include/openssl/ssl.h:openssl"
)

installed_packages() {
    # `repo/name version (reason)` -> name, as check-deps.sh reads it.
    rvn list 2>/dev/null | awk '{ sub(/^[^\/]*\//, "", $1); print $1 }' | sort -u
}

phase_deps() {
    log_section "Build dependencies"

    command -v rvn >/dev/null 2>&1 \
        || log_fatal "rvn is not installed; this path is for Raven systems"

    local entry probe pkg missing=()
    for entry in "${KERNEL_DEPS[@]}"; do
        probe="${entry%%:*}"
        pkg="${entry#*:}"
        if [[ "$probe" == /* ]]; then
            [[ -e "$probe" ]] && continue
        else
            command -v "$probe" >/dev/null 2>&1 && continue
        fi
        [[ " ${missing[*]} " == *" ${pkg} "* ]] || missing+=("$pkg")
    done

    mkdir -p "$STATE_DIR"
    if [[ ${#missing[@]} -eq 0 ]]; then
        log_success "Every build tool is already installed"
        return 0
    fi

    log_info "Installing with rvn: ${missing[*]}"
    # The difference between these two lists is what this run added,
    # dependencies of the missing tools included. Cleanup removes exactly
    # that set, so nothing that was here before is ever touched.
    installed_packages > "${STATE_DIR}/packages.before"
    "${SUDO[@]}" rvn install --yes --repo-only "${missing[@]}" \
        || log_fatal "rvn could not install the build tools"
    installed_packages > "${STATE_DIR}/packages.after"

    # Appended rather than overwritten: a rerun after a failure may add more,
    # and the first run's additions still need removing.
    comm -13 "${STATE_DIR}/packages.before" "${STATE_DIR}/packages.after" \
        >> "${STATE_DIR}/added-packages"
    sort -u -o "${STATE_DIR}/added-packages" "${STATE_DIR}/added-packages"
    log_success "Added $(wc -l < "${STATE_DIR}/added-packages") package(s); cleanup removes them"
}

# =============================================================================
# Build
# =============================================================================
phase_build() {
    log_section "Build"

    [[ $EUID -ne 0 ]] || log_warn "Building as root; run as your own user to keep the tree yours"

    mkdir -p "$BUILD_DIR"
    local free_gb
    free_gb=$(( $(df -Pk "$BUILD_DIR" | awk 'NR == 2 { print $4 }') / 1024 / 1024 ))
    (( free_gb >= MIN_FREE_GB )) \
        || log_fatal "Only ${free_gb} GB free under ${BUILD_DIR}; the build needs about ${MIN_FREE_GB} GB"

    # --clean drops the tree's .config and the staged output, so the build
    # restores configs/kernel/config-6.17-raven and reapplies every floor
    # script to it. An incremental build would keep whatever .config the tree
    # had, which is how a config change silently fails to arrive.
    log_step "Cleaning the previous kernel build"
    "${SCRIPT_DIR}/build-kernel.sh" --clean

    log_step "Building the kernel (${JOBS} jobs)"
    "${SCRIPT_DIR}/build-kernel.sh" --jobs "$JOBS"

    # The running system has evdi in /usr/lib/modules/<release>/extra from
    # the image build. Installing the new modules directory replaces it, so
    # it is rebuilt here, against this tree, into the staged output.
    log_step "Building evdi against the new kernel"
    "${SCRIPT_DIR}/build-evdi.sh"

    local release
    release="$(kernel_release)"
    [[ -f "${KERNEL_TREE}/arch/x86/boot/bzImage" ]] || log_fatal "No bzImage in ${KERNEL_TREE}"
    [[ -d "${STAGED}/lib/modules/${release}" ]] || log_fatal "No staged modules for ${release}"
    log_success "Built ${release}"
}

kernel_release() {
    make -s -C "$KERNEL_TREE" --no-print-directory kernelrelease 2>/dev/null \
        || log_fatal "Could not read kernelrelease from ${KERNEL_TREE}"
}

# =============================================================================
# Install
# =============================================================================
# The release string does not change between builds (6.17.11-raven), so the
# new modules directory has the same name as the running one. The backup is
# therefore the whole directory, moved aside rather than copied: same
# filesystem, so the move is instant and the running kernel's modules are
# kept byte for byte.
phase_install() {
    log_section "Install"

    local release bzimage stamp backup
    release="$(kernel_release)"
    bzimage="${KERNEL_TREE}/arch/x86/boot/bzImage"
    [[ -f "$bzimage" ]] || log_fatal "Nothing to install: build first"

    "${SUDO[@]}" test -f "${ESP_DIR}/vmlinuz" \
        || log_fatal "${ESP_DIR}/vmlinuz not found; is the ESP mounted at ${ESP}?"

    # What is on disk now is only worth backing up if it is what this boot
    # is running. A backup newer than the boot means an earlier run installed
    # a kernel that has not been booted yet; backing that up would replace
    # the last known-good kernel with an untested one, and --rollback would
    # then restore the wrong thing. So that backup stays the rollback target
    # and the unbooted build is simply replaced.
    local btime latest unbooted=false
    btime="$(awk '/^btime/ { print $2 }' /proc/stat)"
    latest="$("${SUDO[@]}" readlink -f "${BACKUP_ROOT}/latest" 2>/dev/null || true)"
    if [[ -n "$latest" ]] && "${SUDO[@]}" test -f "${latest}/esp-vmlinuz" \
        && "${SUDO[@]}" test -d "${latest}/modules" \
        && (( $("${SUDO[@]}" stat -c %Y "$latest") > btime )); then
        unbooted=true
        backup="$latest"
        stamp="$(basename "$latest")"
        log_info "The kernel installed at ${stamp} has not been booted yet; replacing it,"
        log_info "and the kernel before it stays the rollback target"
    else
        stamp="$(date +%Y%m%d-%H%M%S)"
        backup="${BACKUP_ROOT}/${stamp}"
        log_step "Backing up the current kernel to ${backup}"
        "${SUDO[@]}" mkdir -p "$backup"
        "${SUDO[@]}" cp -p "${ESP_DIR}/vmlinuz" "${backup}/esp-vmlinuz"
        if "${SUDO[@]}" test -f "${BOOT_DIR}/vmlinuz"; then
            "${SUDO[@]}" cp -p "${BOOT_DIR}/vmlinuz" "${backup}/boot-vmlinuz"
        fi
        echo "$release" | "${SUDO[@]}" tee "${backup}/release" >/dev/null

        # The previous kernel stays bootable from the ESP as well. 8.3-safe
        # name, for the same firmware raven-install's initrd.img comment is
        # about.
        "${SUDO[@]}" cp -p "${ESP_DIR}/vmlinuz" "${ESP_DIR}/vmlinuz.old"
    fi
    add_previous_kernel_entry

    log_step "Installing modules for ${release}"
    local new="${MODULES_ROOT}/${release}.new"
    "${SUDO[@]}" rm -rf "$new"
    "${SUDO[@]}" cp -R "${STAGED}/lib/modules/${release}" "$new"
    # The staged tree is owned by whoever built it; installed modules are
    # root's, like everything else under /usr.
    "${SUDO[@]}" chown -R 0:0 "$new"
    if "${SUDO[@]}" test -d "${MODULES_ROOT}/${release}"; then
        if $unbooted; then
            # The unbooted build's modules; the good ones are already in
            # the backup.
            "${SUDO[@]}" rm -rf "${MODULES_ROOT}/${release}"
        else
            "${SUDO[@]}" mv "${MODULES_ROOT}/${release}" "${backup}/modules"
        fi
    fi
    "${SUDO[@]}" mv "$new" "${MODULES_ROOT}/${release}"
    "${SUDO[@]}" depmod -a "$release"

    log_step "Installing the kernel image"
    # Written next to the target and renamed over it, so an interrupted copy
    # never leaves the ESP with half a kernel.
    "${SUDO[@]}" cp "$bzimage" "${ESP_DIR}/vmlinuz.new"
    "${SUDO[@]}" mv -f "${ESP_DIR}/vmlinuz.new" "${ESP_DIR}/vmlinuz"
    "${SUDO[@]}" install -m 0644 "$bzimage" "${BOOT_DIR}/vmlinuz"

    "${SUDO[@]}" ln -sfn "$stamp" "${BACKUP_ROOT}/latest"
    prune_backups

    # Modules some other package builds through DKMS. None are registered on
    # a stock install (evdi comes from build-evdi.sh above), but a machine
    # that registered its own would otherwise lose them with the old tree.
    if command -v dkms >/dev/null 2>&1 && [[ -n "$("${SUDO[@]}" dkms status 2>/dev/null)" ]]; then
        log_step "Rebuilding DKMS modules for ${release}"
        "${SUDO[@]}" dkms autoinstall -k "$release" || log_warn "dkms autoinstall failed; see dkms status"
    fi

    # Secure Boot: an unsigned kernel will not boot once keys are enrolled.
    # sbctl re-signs every file it was told about, which on a machine set up
    # the way raven-install describes includes \EFI\raven\vmlinuz.
    if command -v sbctl >/dev/null 2>&1 \
        && "${SUDO[@]}" sbctl list-files 2>/dev/null | grep -q 'EFI/raven/vmlinuz'; then
        log_step "Re-signing for Secure Boot"
        "${SUDO[@]}" sbctl sign-all || log_warn "sbctl sign-all failed; sign ${ESP_DIR}/vmlinuz before rebooting"
    fi

    sync
    log_success "Installed ${release}; previous kernel backed up to ${backup}"
    log_warn "Reboot soon: the running kernel now has a modules directory built for the new one"
    log_info "If the new kernel does not boot, pick \"RavenLinux (previous kernel, rescue shell)\""
    log_info "in RavenBoot and run: ${PROJECT_ROOT}/scripts/build-kernel-native.sh --rollback"
}

# Deletes all but the newest KEEP_BACKUPS backups. The stamps sort by time,
# and `latest` is always the newest, so it is never among them.
prune_backups() {
    local old
    "${SUDO[@]}" find "$BACKUP_ROOT" -mindepth 1 -maxdepth 1 -type d -name '20*' -printf '%f\n' \
        | sort -r | tail -n +$((KEEP_BACKUPS + 1)) \
        | while read -r old; do
            log_info "Removing old kernel backup ${old}"
            "${SUDO[@]}" rm -rf "${BACKUP_ROOT:?}/${old}"
        done
}

# Adds a RavenBoot entry that boots \EFI\raven\vmlinuz.old into a root
# shell, copied from the rescue entry raven-install writes so it carries this
# machine's root= and resume=. It is a rescue entry on purpose: the modules
# directory now belongs to the new kernel, and init=/bin/bash starts no udev,
# so nothing loads a module built for the other kernel. From that shell,
# --rollback puts the old kernel and its modules back.
add_previous_kernel_entry() {
    local cfg="${ESP_DIR}/boot.cfg"
    "${SUDO[@]}" test -f "$cfg" || { log_warn "No ${cfg}; no previous-kernel entry added"; return 0; }
    if "${SUDO[@]}" grep -q 'vmlinuz\.old' "$cfg"; then
        return 0
    fi

    local entry
    # One [entry] block at a time; comments and blank lines are not part of
    # an entry (raven-install puts a comment paragraph right after this one).
    entry="$("${SUDO[@]}" awk '
        /^\[entry\]/ { if (block ~ /rescue shell/) { found = 1; exit } block = $0 "\n"; next }
        /^[[:space:]]*(#|$)/ { next }
        { block = block $0 "\n" }
        END { if (found || block ~ /rescue shell/) printf "%s", block }
    ' "$cfg")"
    if [[ -z "$entry" ]]; then
        log_warn "No rescue entry in ${cfg} to copy; no previous-kernel entry added"
        return 0
    fi

    entry="$(printf '%s' "$entry" \
        | sed -e 's/^name = .*/name = "RavenLinux (previous kernel, rescue shell)"/' \
              -e 's/^kernel = .*/kernel = "\\EFI\\raven\\vmlinuz.old"/')"
    "${SUDO[@]}" cp -p "$cfg" "${cfg}.bak"
    printf '\n%s\n' "$entry" | "${SUDO[@]}" tee -a "$cfg" >/dev/null
    log_info "Added \"RavenLinux (previous kernel, rescue shell)\" to boot.cfg"
}

# =============================================================================
# Rollback
# =============================================================================
phase_rollback() {
    log_section "Rollback"

    local backup release
    backup="$("${SUDO[@]}" readlink -f "${BACKUP_ROOT}/latest" 2>/dev/null || true)"
    [[ -n "$backup" ]] && "${SUDO[@]}" test -f "${backup}/esp-vmlinuz" \
        || log_fatal "No kernel backup under ${BACKUP_ROOT}"
    release="$("${SUDO[@]}" cat "${backup}/release")"

    # From the rescue shell nothing has mounted the ESP yet.
    if ! "${SUDO[@]}" test -f "${ESP_DIR}/vmlinuz"; then
        "${SUDO[@]}" mount "$ESP" 2>/dev/null || log_fatal "Cannot mount the ESP at ${ESP}"
    fi

    log_step "Restoring the kernel from ${backup}"
    "${SUDO[@]}" cp "${backup}/esp-vmlinuz" "${ESP_DIR}/vmlinuz.new"
    "${SUDO[@]}" mv -f "${ESP_DIR}/vmlinuz.new" "${ESP_DIR}/vmlinuz"
    if "${SUDO[@]}" test -f "${backup}/boot-vmlinuz"; then
        "${SUDO[@]}" cp "${backup}/boot-vmlinuz" "${BOOT_DIR}/vmlinuz"
    fi

    if "${SUDO[@]}" test -d "${backup}/modules"; then
        log_step "Restoring modules for ${release}"
        "${SUDO[@]}" rm -rf "${backup}/modules.replaced"
        if "${SUDO[@]}" test -d "${MODULES_ROOT}/${release}"; then
            "${SUDO[@]}" mv "${MODULES_ROOT}/${release}" "${backup}/modules.replaced"
        fi
        "${SUDO[@]}" mv "${backup}/modules" "${MODULES_ROOT}/${release}"
        "${SUDO[@]}" depmod -a "$release"
    fi

    # The backup has been spent: its modules are live again. Left as
    # `latest`, the next install would take it for the backup of an unbooted
    # kernel and delete the modules that were just restored.
    "${SUDO[@]}" rm -f "${BACKUP_ROOT}/latest"

    sync
    log_success "Previous kernel restored; reboot into the normal RavenLinux entry"
}

# =============================================================================
# Cleanup
# =============================================================================
phase_cleanup() {
    log_section "Cleanup"

    if [[ -s "${STATE_DIR}/added-packages" ]]; then
        local added=()
        mapfile -t added < "${STATE_DIR}/added-packages"
        log_step "Uninstalling what the deps phase added: ${added[*]}"
        # --keep-orphans: everything this run pulled in is already in the
        # list, and anything else that happens to be an orphan was not ours
        # to remove.
        if "${SUDO[@]}" rvn uninstall --yes --keep-orphans "${added[@]}"; then
            rm -f "${STATE_DIR}/added-packages"
        else
            log_warn "rvn would not remove them all; the list is kept in ${STATE_DIR}/added-packages"
        fi
    else
        log_info "No packages were added by this pipeline"
    fi

    log_step "Removing kernel sources and staged output"
    # The staged tree and the sources are the user's, but the evdi clone and
    # a build that ran as root may not be; sudo covers both.
    "${SUDO[@]}" rm -rf \
        "$KERNEL_TREE" \
        "${BUILD_DIR}/sources/linux-${KERNEL_FULL_VERSION}.tar.xz" \
        "${BUILD_DIR}/sources/evdi-"* \
        "$STAGED"
    rm -f "${STATE_DIR}/packages.before" "${STATE_DIR}/packages.after"
    rmdir "$STATE_DIR" 2>/dev/null || true
    log_success "Cleanup done"
}

# =============================================================================
# Main
# =============================================================================
main() {
    grep -qE '^ID="?raven"?$' /etc/os-release 2>/dev/null \
        || log_fatal "This is the native Raven path; on other hosts use scripts/docker-build.sh"

    if $DO_ROLLBACK; then
        phase_rollback
        return
    fi

    $DO_DEPS    && phase_deps
    $DO_BUILD   && phase_build
    $DO_INSTALL && phase_install
    $DO_CLEANUP && phase_cleanup
    return 0
}

main
