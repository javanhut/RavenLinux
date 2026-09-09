#!/bin/bash
# =============================================================================
# kernel-performance.sh -- consistency and observability for the Raven kernel
# =============================================================================
#
# Applies the Kconfig options that make the machine's performance measurable
# and steady under memory pressure. Each block says what the option costs and
# what it buys; nothing here is a guess, every one was chosen against a
# measurement on a running Raven system (see docs/kernel-performance.md).
#
# Usage: scripts/kernel-performance.sh <kernel-source-dir>
#
# Same contract as kernel-ports.sh: edits <kernel-source-dir>/.config in place
# with the kernel's own scripts/config, and the caller runs `make olddefconfig`
# afterwards so dependencies resolve. Idempotent. build-kernel.sh applies it
# both when it generates a config from scratch and when it restores the saved
# one, so a menuconfig session or a kernel bump cannot quietly drop these.
# =============================================================================

set -euo pipefail

src="$(cd "${1:?usage: $0 <kernel-source-dir>}" && pwd)"
cfg="${src}/scripts/config"
[ -x "$cfg" ] || { echo "no scripts/config in $src" >&2; exit 1; }
cd "$src"

y() { for o in "$@"; do "$cfg" --enable "$o"; done; }
n() { for o in "$@"; do "$cfg" --disable "$o"; done; }

# --- Pressure stall information ---------------------------------------------
# /proc/pressure/{cpu,memory,io}: how much time tasks spent stalled waiting
# for each resource. It is the one number that says *why* a machine stuttered
# rather than that it did. Accounting cost is a handful of instructions on
# the scheduler paths; the kernel's own default has it enabled.
y PSI
n PSI_DEFAULT_DISABLED

# --- Compressed swap cache ----------------------------------------------------
# zswap keeps pages that would go to swap compressed in RAM first, and writes
# to the swap device only when that pool fills. On a laptop with a SATA SSD
# and a browser this is the difference between a stutter and a stall: a
# compressed page comes back in microseconds, a swapped one in milliseconds.
# zstd for the ratio, zsmalloc for the density; the shrinker lets the pool
# give memory back when the pressure is gone. Uses the swap partition the
# installer already makes; no zram device and no second swap to manage.
y ZSWAP ZSWAP_DEFAULT_ON ZSWAP_SHRINKER_DEFAULT_ON
y ZSWAP_COMPRESSOR_DEFAULT_ZSTD CRYPTO_ZSTD
y ZSWAP_ZPOOL_DEFAULT_ZSMALLOC ZSMALLOC

# --- Transparent hugepages, on request only ---------------------------------
# madvise mode: only a program that asks (browsers, Mesa, allocators that
# know what they are doing) gets 2 MB pages, so nothing else pays the
# compaction and memory cost that "always" imposes on small processes.
# "never" was the effective setting before, because the option was not
# built at all.
y TRANSPARENT_HUGEPAGE TRANSPARENT_HUGEPAGE_MADVISE
n TRANSPARENT_HUGEPAGE_ALWAYS TRANSPARENT_HUGEPAGE_NEVER

# --- PCIe ASPM policy, decided at build time ---------------------------------
# The Realtek RTL8821CE cannot power on again behind a link the host parks in
# L1, so Raven runs with ASPM off on every link. That used to be a modprobe.d
# line that raven-init tried to write into a read-only parameter at every
# boot, and warned that it could not. The policy is a Kconfig choice, so it
# is set here and the warning is gone. It stays overridable on the kernel
# command line with pcie_aspm.policy=.
y PCIEASPM PCIEASPM_PERFORMANCE
n PCIEASPM_DEFAULT PCIEASPM_POWERSAVE PCIEASPM_POWER_SUPERSAVE

# --- Deliberately not touched ------------------------------------------------
# DEBUG_KERNEL, SLUB_DEBUG, SCHEDSTATS, KALLSYMS_ALL and FTRACE are all in the
# shipped config. Their runtime switches default to off, so what they cost is
# a few kilobytes of text and nothing measurable on the paths that matter.
# They are the tools a stall gets diagnosed with. Remove any of them only
# after measuring what its absence buys, and say so here.
