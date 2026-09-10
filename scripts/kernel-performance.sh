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
# text and rodata (measured in docs/kernel-performance.md; KALLSYMS_ALL alone
# is about a megabyte) and nothing measurable on the paths that matter. They
# are the tools a stall gets diagnosed with. Remove any of them only after
# measuring what its absence buys, and say so here.

# --- Debug options: candidates, pending measurement ---------------------------
# The audit is in docs/kernel-performance.md ("Debug options audit"). Every
# line below is a candidate, not a decision: it stays commented until the
# measurement named beside it has been run on two builds and the number is
# written here next to it. Most likely to matter first.
#
# DEBUG_KERNEL itself is not a candidate: it is the gate for most of these,
# and disabling it lets olddefconfig drop KALLSYMS_ALL, SCHEDSTATS, RCU_TRACE,
# DEBUG_STACK_USAGE, X86_DEBUG_FPU and DEBUG_ENTRY in one move, which is the
# opposite of measuring one at a time. SLUB_DEBUG and DEBUG_MEMORY_INIT are
# not candidates either: their prompts are behind EXPERT, which this config
# does not set, so olddefconfig would put them straight back. The same goes
# for what has no prompt at all: SYSCTL_EXCEPTION_TRACE (x86 selects it),
# TASKS_TRACE_RCU (UPROBES selects it) and the NOP_TRACER, TRACE_CLOCK,
# CONTEXT_SWITCH_TRACER, GENERIC_TRACER, PROBE_EVENTS pieces under FTRACE;
# they are in the audit, and a --disable on any of them is undone by
# olddefconfig while its selector stays.
#
# ACPI_DEBUG: the ACPICA build calls acpi_ut_trace() on entry to and
# acpi_ut_status_exit() on return from every ACPICA function, inside the AML
# interpreter that runs on each EC, battery, thermal and lid event; Kconfig
# says ~50K of text on top. Measure: bloat-o-meter on drivers/acpi/acpica,
# the ACPI init span in dmesg timestamps, a loop over
# /sys/class/power_supply/BAT*/uevent.
#n ACPI_DEBUG
#
# DEBUG_ENTRY: asserts in the entry asm on every interrupt and exception
# return; Kconfig: "may slow down kernel entries and exits". Not a default,
# so it was chosen at some point. Measure: perf bench syscall basic, and
# an interrupt-heavy load, on two builds.
#n DEBUG_ENTRY
#
# X86_DEBUG_FPU: WARN_ON_FPU() checks in the FPU switch path on every context
# switch; Kconfig: "some small amount of runtime overhead". Arrived with
# DEBUG_KERNEL as a default y. Measure: perf bench sched pipe on two builds.
#n X86_DEBUG_FPU
#
# DEBUG_STACK_USAGE: walks the unused part of the stack at every process
# exit; Kconfig: "will slow down process creation somewhat". Nothing on Raven
# reads the dmesg line it produces. Measure: perf bench sched messaging and
# raven-rc blame totals on two builds.
#n DEBUG_STACK_USAGE
#
# KALLSYMS_ALL: about 1 MB of rodata (19.6% of a 5.07 MB symbol table, by
# symbol count) for data symbols nothing in the image reads: no BPF, no kgdb,
# no livepatch. Measure: size vmlinux rodata delta; confirm no tool in the
# image wants non-text names from /proc/kallsyms.
#n KALLSYMS_ALL
#
# RCU_TRACE: 22 rcu:* tracepoints and 24.6 KB of generated event code with no
# desktop consumer. Measure: bloat-o-meter, nothing else moving.
#n RCU_TRACE
#
# CGROUP_DEBUG: Kconfig says "Say N"; about 1 KB, hidden on cgroup2 without
# cgroup_debug= on the command line. Measure: bloat-o-meter.
#n CGROUP_DEBUG
#
# DEBUG_DEVRES: a name and size stored in every devres node for dev_dbg
# messages that compile to nothing without DYNAMIC_DEBUG. Measure:
# bloat-o-meter on drivers/base/devres.o.
#n DEBUG_DEVRES
