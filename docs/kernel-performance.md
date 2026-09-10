# Kernel: performance consistency and observability

What `scripts/kernel-performance.sh` turns on, why, and how to see each one
working on a running system. `build-kernel.sh` applies the script both when it
generates a config and when it restores the saved one, so these hold across
menuconfig sessions and kernel bumps.

The choices were made against measurements on an Intel N97 laptop with 12 GB
of RAM, a SATA SSD, a 12 GB swap partition and a browser open. Before this
change that machine had no pressure accounting, swapped uncompressed to SATA,
and had no hugepage support compiled at all.

| Option | Setting | What it buys | What it costs |
|---|---|---|---|
| `PSI` | on, not default-disabled | `/proc/pressure/{cpu,memory,io}`: time tasks spent stalled on each resource. The number that says *why* the machine stuttered. | A few instructions on scheduler paths. |
| `ZSWAP` and friends | on by default, zstd, zsmalloc, shrinker on | Pages headed for swap are compressed in RAM first and only written to the SSD when the pool fills. Microseconds instead of milliseconds on the way back. | RAM for the pool (the shrinker returns it when pressure drops) and CPU to compress. |
| `TRANSPARENT_HUGEPAGE` | madvise | 2 MB pages for programs that ask (browsers, Mesa, allocators). Fewer TLB misses where it matters. | Nothing for programs that do not ask. "always" would have cost small processes compaction time. |
| `PCIEASPM_PERFORMANCE` | on | ASPM off on every link, which the RTL8821CE needs to power on more than once. Was a modprobe.d line raven-init could not apply. | Some link power on laptops with other cards. Override with `pcie_aspm.policy=` on the kernel command line. |

## Verifying on a running system

```
cat /proc/pressure/memory                    # avg10=0.00 ... ; nonzero under load
cat /sys/module/zswap/parameters/enabled     # Y
cat /sys/module/zswap/parameters/compressor  # zstd
grep -E 'zswpin|zswpout' /proc/vmstat        # pages in and out of the pool
cat /sys/kernel/mm/transparent_hugepage/enabled   # always [madvise] never
grep AnonHugePages /proc/meminfo             # nonzero once a browser is up
cat /sys/module/pcie_aspm/parameters/policy  # [performance] ...
```

`raven-rc blame` and the timestamps in `/var/log/raven/init.log` are the
other half of this: they say where boot time goes, PSI says where run time
goes.

## Not changed, on purpose

`DEBUG_KERNEL`, `SLUB_DEBUG`, `SCHEDSTATS`, `KALLSYMS_ALL` and `FTRACE` stay.
Their runtime switches are off by default, so they cost text and rodata and
nothing measurable on hot paths, and they are the tools a stall gets
diagnosed with. How much text and rodata, and which of the smaller options
under them do sit on a hot path, is in the audit below. Remove one only after
measuring what its absence buys, and record the number in the script.

`vm.swappiness` stays at the kernel default. With zswap in front of the
partition the old advice to lower it no longer applies.

## Debug options audit

Everything below is what the shipped config (`configs/kernel/config-6.17-raven`)
has switched on under "Kernel hacking" and the debug knobs scattered through
the rest of the tree, with what it costs and what would decide its removal.
Nothing was removed: there is no kernel tree on the machine this was written
on, so no second build to compare against. The numbers that are here come
from the installed build instead: symbol sizes are next-symbol deltas in
`/lib/modules/6.17.11-raven/build/System.map`, section sizes are the
`__start_*`/`__stop_*` pairs in the same file, switch states were read from
the running kernel, whose `/proc/config.gz` differs from the shipped config
only by the options this script adds. Kconfig quotes are from 6.17 as
remembered, not copied from a tree; where the memory is fuzzy the row says so.

Recommendations mean:

- **keep**: it is a diagnosis tool or a safety check, and it costs nothing on
  a hot path. Nothing to measure.
- **remove-after-measuring**: it has a cost on a real path, small enough that
  only a two-build comparison can say whether it matters. The row names the
  comparison.
- **remove-now-if-measurement-confirms**: there is no plausible consumer on a
  desktop; a single bloat-o-meter run confirming that nothing else moves is
  enough.

### Gate

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `DEBUG_KERNEL` | Kconfig: "Say Y here if you are developing drivers or trying to debug the kernel." It compiles nothing itself; it is the `depends on` for most of the rows below. | none | none | **keep.** Turning it off lets `make olddefconfig` drop `KALLSYMS_ALL`, `SCHEDSTATS`, `RCU_TRACE`, `DEBUG_STACK_USAGE`, `X86_DEBUG_FPU`, `DEBUG_ENTRY` and `DEBUG_MISC` in one move, which is the opposite of measuring one at a time. |
| `DEBUG_MISC` | Kconfig: "Say Y here if you need to enable miscellaneous debug code that should be under a more specific debug option but isn't." Defaults to `DEBUG_KERNEL`. | none | a handful of `#ifdef` printks in odd corners; not separable in System.map | **keep.** Nothing to measure. |

### Symbols, stack traces, printk

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `KALLSYMS_ALL` | Adds data, bss and rodata symbols to the in-kernel symbol table, not only text. Kconfig, roughly: normally kallsyms only contains the symbols of functions, which is sufficient for most cases; say N unless you really need all symbols or kernel live patching. | none | Measured: the kallsyms tables (`kallsyms_names` 3,392,960 B, `kallsyms_offsets` 956,544 B, `kallsyms_seqs_of_names` 717,414 B, markers and tokens 5,160 B) are 5,072,078 B of rodata, half of the 10,182,656 B `.rodata` section. 46,823 of the 239,138 symbols (19.6%) are data symbols, so about 1 MB of that table is this option, by count. No hot-path cost; `kallsyms_lookup_name` walks are longer, and only kprobe registration and oops output do those. | **remove-now-if-measurement-confirms.** The consumers of data symbols are BPF (`BPF_SYSCALL` is off), kgdb (off), livepatch (off) and `perf probe` on variables. Measurement: `size vmlinux` rodata delta between two builds, expected around 1 MB, and a check that no tool in the image reads non-text names from `/proc/kallsyms`. The old "a few kilobytes" in this document was wrong by three orders of magnitude. |
| `DEBUG_BUGVERBOSE` | Kconfig: "Say Y here to make BUG() panics output the file name and line number of the BUG call as well as the EIP and oops trace. This aids debugging but costs about 70-100K of memory." | none | Measured: `__bug_table` is 140,100 B, 11,675 entries at 12 B, in its own read-only section after `.data` (`__start___bug_table` is `_edata`), not in `.rodata`. Without this each entry drops the file and line fields (4 B, from memory of `struct bug_entry`), so about 47 KB plus the file-name strings. Nothing at runtime. | **keep.** Every WARN in a bug report names its file and line. |
| `SYMBOLIC_ERRNAME`, `PRINTK_TIME` | `-EINVAL` instead of `-22` in printk; timestamps on every line. | none / `printk.time` | a name table of a few KB; a clock read per printk | **keep.** The boot-time analysis in this document reads those timestamps. |
| `STACKTRACE`, `UNWINDER_ORC` | Stack traces for oops, sysrq-t, perf callchains, without frame pointers. | none | Measured: `.orc_unwind_ip` 2,276,864 B plus `.orc_unwind` 3,415,296 B, 5.7 MB in their own read-only sections after `.data` (`__start_orc_unwind_ip` is above `_edata`), so they are not in the 9944K "rodata" of the dmesg `Memory:` line. That is the price of `UNWINDER_ORC=y` over `UNWINDER_FRAME_POINTER`, which is not set (so `FRAME_POINTER` is off): the frame-pointer unwinder keeps `%rbp` out of general use in every function, a runtime cost on everything, and ORC pays in tables instead. `SCHED_OMIT_FRAME_POINTER=y`, also set, is unrelated: it is the "Single-depth WCHAN output" option, and only decides whether `kernel/sched/core.o` keeps frame pointers for a deeper `wchan`. | **keep.** Not a debug option so much as the reason the kernel can say where it was. |
| `SYSCTL_EXCEPTION_TRACE` | Kconfig: "Enable support for /proc/sys/debug/exception-trace." No prompt; x86 selects it (from memory of arch/x86/Kconfig). The `segfault at ... ip ... sp ... error ... in ...` dmesg line for a user process that dies of SIGSEGV, SIGBUS or a general protection fault. | `debug.exception-trace` sysctl, backed by `show_unhandled_signals` (verified in System.map); reads **1** on the running system, the kernel's default | A load and a branch on the fault path of a process that is already dying, and a rate-limited printk when it is 1. Nothing on any path that succeeds. | **keep.** It is the one line a crashed program leaves in the log, and it cannot be deselected on x86 anyway. If the log noise ever matters, `sysctl debug.exception-trace=0`. |

### Tracing

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `FTRACE`, `TRACING`, `EVENT_TRACING`, `TRACEPOINTS` | Kconfig for `FTRACE`: "Enable the kernel tracing infrastructure." The tracefs interface, the ring buffer, and one trace event per tracepoint: `perf trace`, `perf sched`, `trace-cmd`, `echo 1 > events/.../enable`. `NOP_TRACER`, `TRACE_CLOCK`, `CONTEXT_SWITCH_TRACER`, `GENERIC_TRACER` and `PROBE_EVENTS` are the promptless pieces selected under these (the default tracer, the trace clocks, the sched-switch hooks for `EVENT_TRACING`, the tracer core that `BLK_DEV_IO_TRACE` selects, the argument parser shared by kprobe and uprobe events; which selects which is from memory of kernel/trace/Kconfig); they come and go with `FTRACE`. | tracefs `tracing_on` (1) with the `nop` tracer and no events enabled, so nothing is recorded; each event has its own `enable` (0) | Measured: 2,374 tracepoints, 2,375 trace events. Each site is a static branch, a 5-byte nop when its event is off; the image has 7,750 jump-table entries in total. Generated per-event code and data, summed by symbol prefix: `trace_event_raw_event_*` 397,312 B, `perf_trace_*` 421,888 B, `trace_raw_output_*` 161,792 B, `print_fmt_*` 645,120 B, `event_class_*` 120,832 B, `trace_event_type_funcs_*` 40,960 B, `__traceiter_*` 262,144 B, `__tracepoint_*` 227,328 B, `__tpstrtab_*` 67,584 B, static-call sites 53,248 B, `__event_*` 18,432 B: about 2.4 MB across text, rodata and data. The ring buffer core in kernel/trace is on top of that and is not cleanly separable by name. RAM: from memory of kernel/trace/trace.c the top-level buffer starts at one page per CPU and grows to `trace_buf_size` (1,441,792 B per CPU) on first use; check `buffer_size_kb` once tracefs is mounted. | **keep.** This is the tool. `FUNCTION_TRACER` is already off, so there is no `fentry` nop at every function, no `DYNAMIC_FTRACE`, no `function_graph`, no `STACK_TRACER`; what is left is the cheap half. Measurement, if someone wants the 2.4 MB back: bloat-o-meter between `FTRACE=y` and `n`, and a list of what stops working (`perf trace`, `perf sched`, blktrace, kprobe events). |
| `KPROBE_EVENTS`, `UPROBE_EVENTS`, `EPROBE_EVENTS`, `DYNAMIC_EVENTS`, `KPROBES`, `OPTPROBES`, `UPROBES`, `KRETPROBES`, `RETHOOK` | Trace events placed at runtime on any kernel function or user address (`perf probe`, `kprobe_events`). | none until a probe is written | none. Without `FUNCTION_TRACER` a kprobe is an `int3` (or an optimized jump); that only matters while a probe is armed. | **keep.** |
| `TASKS_TRACE_RCU` | A third RCU flavour beside `TASKS_RCU` (also on, from `NEED_TASKS_RCU` and `PREEMPT`): readers mark themselves with `rcu_read_lock_trace()` instead of disabling preemption, so a probe handler can sleep. Kconfig, roughly: enables a task-based RCU whose readers may appear in the idle loop and on CPU-hotplug paths; it can force IPIs on online CPUs, including idle ones, so use with caution. No prompt. With `BPF_SYSCALL` off the selector is `UPROBES` (from memory: since 6.12 uprobe handlers run under tasks-trace RCU and `config UPROBES` selects it), and `UPROBES` is what `UPROBE_EVENTS` needs. | none | One kthread, `rcu_tasks_trace_kthread` (verified running), which sleeps until `call_rcu_tasks_trace()` queues something, and nothing queues without a uprobe being removed. Read on the running system: 0 CPU ticks and 2 wakeups in 21,174 s of uptime. Two or three counters in every `task_struct`, and the `trc_*` bookkeeping on the context-switch path is a store only when the outgoing task is inside a reader (from memory of kernel/rcu/tasks.h). | **keep.** It cannot go while `UPROBE_EVENTS` stays, and the measured cost of an idle flavour is a sleeping kthread. |
| `BLK_DEV_IO_TRACE` | Kconfig: "Say Y here if you want to be able to trace the block layer actions on a given queue." `blktrace`, `btrace`, `iowatcher`, and `/sys/block/*/trace/enable`. Selects `RELAY`. | per-queue `trace/enable` (0) | none: the block tracepoints exist regardless under `EVENT_TRACING`, and blktrace attaches to them only when enabled. | **keep.** A SATA stall is exactly what this was made for. |
| `RCU_TRACE` | Kconfig: "This option enables additional tracepoints for ftrace-style event tracing." `default y if TREE_RCU`. | per-event `enable` (0) | Measured: 22 `rcu:*` tracepoints, 24,576 B of generated event code. Nop'd static branches on the callback path. | **remove-now-if-measurement-confirms.** Nobody debugging a desktop reads `rcu:*`. Measurement: bloat-o-meter, expected about 25 KB and nothing else moving. |
| `TRACEFS_AUTOMOUNT_DEPRECATED` | New in 6.17: keeps the old automount of tracefs under `/sys/kernel/debug/tracing` for tools that have not moved to `/sys/kernel/tracing`, with a deprecation warning (from memory; the exact behaviour was not checked). | none | none | **keep** until the tools in the image are known to use `/sys/kernel/tracing`. |

### Scheduler and accounting

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `SCHEDSTATS` | Kconfig: "If you say Y here, additional code will be inserted into the scheduler and related routines to collect statistics about scheduler behavior and provide them in /proc/schedstat. [...] If you aren't debugging the scheduler or trying to tune a specific application, you can say N to avoid the very slight overhead this adds." Per-task wait, sleep and block times in `/proc/<pid>/sched`; `perf sched`'s wait statistics. | `kernel.sched_schedstats` sysctl / `schedstats=enable`; read as **0** on the running system | Static key `sched_schedstats` (16 B of bss, verified in System.map); the sites it guards are nops. Named helpers `__update_stats_wait_start/end`, `__update_stats_enqueue_sleeper`, `show_schedstat`, `sysctl_schedstats`: 1,280 B of text. Per task, a `struct sched_statistics` in each sched entity, on the order of 100 B (from memory). | **keep**, as this document already decided. If measured anyway: `perf bench sched pipe` on two builds; the expected difference is zero, because the code is behind a nop. |
| `SCHED_INFO` | No prompt. Selected by `SCHEDSTATS` and by `TASK_DELAY_ACCT`, both on. Records run delay and run time per task for `/proc/<pid>/schedstat` and delay accounting. | none | A few loads and stores at every enqueue and context switch, unconditionally (from memory of kernel/sched/stats.h). This is the one thing in the "runtime switch off" list that is not behind a switch. | **keep.** It cannot go while either selector stays, and `/proc/<pid>/schedstat` is what `top`-style tools use for per-process run delay. |
| `TASK_DELAY_ACCT`, `TASKSTATS`, `TASK_XACCT`, `TASK_IO_ACCOUNTING` | Per-task delay accounting (`getdelays`, `iotop`'s IO delay column), the netlink taskstats interface. | `kernel.task_delayacct` sysctl / `delayacct=1`; read as **0** on the running system | static key `delayacct_key` (verified), nops when off | **keep.** Same reasoning as PSI, one level down. |

### Memory

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `SLUB_DEBUG` | Kconfig: "SLUB has extensive debug support features. Disabling these can result in significant savings in code size." Red zones, poisoning, allocator tracking and cache validation, per cache, for catching a use-after-free or overrun in a driver. `SLUB_DEBUG_ON` is off, so no cache has any of it at boot. Selects `STACKDEPOT`, which initializes lazily (`stack_depot_disabled` is present). | `slab_debug=` on the command line, or `/sys/kernel/slab/<cache>/{red_zone,poison,store_user,sanity_checks,trace}` (root only; all default 0) | A `kmem_cache_debug()` flag test on the allocation and free slow paths, not the per-CPU fast paths (from memory of mm/slub.c). Text: 52 separately named helpers add up to 6,144 B; the rest is inlined into the slow paths and only bloat-o-meter on `mm/slub.o` can size it. | **keep.** It is the only way to catch slab corruption on a shipped kernel without a rebuild, and `slab_debug=FZPU,<cache>` costs nothing until typed. Note the prompt is `if EXPERT`, and `EXPERT` is off in this config, so `make olddefconfig` would put it straight back to its default of `y`. Measurement if wanted: bloat-o-meter on `mm/slub.o`, and `perf bench sched messaging` (slab-heavy: task structs, files, pipes) on two builds. |
| `DEBUG_STACK_USAGE` | Kconfig: "Enables the display of the minimum amount of free stack which each task has ever had available in the sysrq-T and sysrq-P debug output. Also emits a message to dmesg when a process exits if that process used more stack space than previously exiting processes. This option will slow down process creation somewhat." | none | At every process exit, a walk from the bottom of the 16 KB stack over the unused zero words (`stack_not_used`), then a compare against a global; `lowest_to_date.1` and `low_water_lock.0` are in System.map, so it is compiled in. Stacks are zeroed on allocation anyway under `VMAP_STACK`, so creation is not slower any more; the exit walk is on the order of a thousand loads. | **remove-after-measuring.** Nothing on Raven reads the "used greatest stack depth" line. Measurement: `perf bench sched messaging` (many short-lived tasks) on two builds, and `raven-rc blame` totals, since boot is mostly short processes. Expected: inside noise, which is still a number. |
| `DEBUG_MEMORY_INIT` | Kconfig: "Enable this for additional checks during memory initialisation." Output controlled by `mminit_loglevel=`. `default !EXPERT`, prompt `if EXPERT`. | `mminit_loglevel=` (0) | boot only; a few page-flag and zonelist checks | **keep.** Not switchable without `EXPERT` anyway. |
| `DEBUG_WX`, `PTDUMP` | Kconfig: "Generate a warning if any W+X mappings are found at boot. [...] There is no runtime or memory usage effect of this option once the kernel has booted up - it's a one time check." Verified in dmesg: `x86/mm: Checked W+X mappings: passed, no W+X pages found.` | none | one page-table walk at the end of boot | **keep.** It is a security check, not a debug aid. |

### x86 entry, FPU, early boot

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `DEBUG_ENTRY` | Kconfig: "This option enables sanity checks in x86's low-level entry code. Some of these sanity checks may slow down kernel entries and exits or otherwise impact performance. If unsure, say N." Not default; somebody chose it. | none | From memory of arch/x86/entry/entry_64.S: a `testb $3, CS(%rsp)` plus never-taken branch on the return-to-user and return-to-kernel paths, and an interrupts-off assertion (`pushf`, `test`, branch) on the interrupt return path. A handful of instructions per interrupt and exception return; the `sysret` fast path is untouched. | **remove-after-measuring.** Measurement: `perf bench syscall basic` and an interrupt-heavy load (network or disk throughput with `perf stat -e irq:*`) on two builds. Expected: not measurable; but it is the one option here whose help text says it slows entries and exits. |
| `X86_DEBUG_FPU` | Kconfig: "If this option is enabled then there will be extra sanity checks and (boot time) debug printouts added to the kernel. This debugging adds some small amount of runtime overhead to the kernel. If unsure, say N." `default y` under `DEBUG_KERNEL`, so this one arrived with the gate. | none | `WARN_ON_FPU()` becomes a real check instead of `(void)`; the sites are in the FPU save/restore path on context switch and in `kernel_fpu_begin()` (from memory of arch/x86/kernel/fpu). One or two compare-and-branch per switch. | **remove-after-measuring.** Measurement: `perf bench sched pipe` on two builds, which is the context-switch path with nothing else in it. |
| `EARLY_PRINTK`, `EARLY_PRINTK_DBGP`, `EARLY_PRINTK_USB`, `X86_VERBOSE_BOOTUP`, `DEBUG_BOOT_PARAMS` | `earlyprintk=` console before the real one; decompressor chatter; `boot_params` in debugfs. | `earlyprintk=` (unset) | boot only, then discarded with init text | **keep.** They are how a kernel that dies before the console comes up gets diagnosed. |

### Power management and ACPI

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `ACPI_DEBUG` | Kconfig: "The ACPI subsystem can produce debug output. Saying Y enables this output and increases the kernel size by around 50K. Use the acpi.debug_layer and acpi.debug_level kernel command-line parameters [...] to control the type and amount of debug output." | `acpi.debug_level` / `acpi.debug_layer`, also writable in `/sys/module/acpi/parameters/`. Read on the running system: level `0xC` (`ACPI_LV_INFO` and `ACPI_LV_REPAIR`), layer `0`; nothing prints below that. | More than the 50K: defining `ACPI_DEBUG_OUTPUT` makes every ACPICA function call `acpi_ut_trace()` on entry and `acpi_ut_status_exit()` (or a sibling) on return, and those bump `acpi_gbl_nesting_level` and test the level before doing nothing (from memory of ACPICA's `acmacros.h`; all three symbols are present in System.map, so the tracing is compiled in). That is a call pair per ACPICA function, in the AML interpreter that runs on every EC query, battery and thermal read, lid and brightness key, and during boot enumeration. ACPICA text in this build (`acpi_{ds,ex,ns,ps,ut,ev,hw,tb,rs}_*`) is roughly 260 KB: a next-symbol sum over those prefixes gives 260,096 B, and counting every address whose aliases include one of them gives 303,104 B, so the figure moves by tens of KB with the method and is approximate. | **remove-after-measuring.** The most likely real win on this list. Measurement: bloat-o-meter on `drivers/acpi/acpica/`, the dmesg timestamps from `ACPI: Core revision` to `ACPI: Interpreter enabled` and on to the last `ACPI:` line at boot, and `time` of a loop over `cat /sys/class/power_supply/BAT*/uevent`, on two builds. |
| `PM_DEBUG`, `PM_SLEEP_DEBUG`, `PM_TRACE`, `PM_TRACE_RTC` | `pm_test` stages, `pm_print_times`, `pm_debug_messages`, and the resume-hang tracer. Kconfig for `PM_TRACE_RTC`: "This enables some cheesy code to save the last PM event point in the RTC across reboots, so that you can debug a machine that just hangs during suspend (or more commonly, during resume). [...] CAUTION: this option will cause your machine's real-time clock to be set to an invalid time after a resume." | `/sys/power/pm_trace` (read as **0**), `/sys/power/pm_debug_messages` (read as **0**), `/sys/power/pm_test` | a global-flag test on the suspend and resume paths; 864 B of named helpers (`set_trace_device`, `generate_pm_trace`, `show_trace_dev_match`, `pm_trace_notify`) | **keep.** A machine whose Wi-Fi card cannot come back from L1 is a machine that will need `pm_trace` one day. The clock corruption only happens once someone writes 1 to it, and the help text says so. |

### Miscellaneous

| Option | What it does | Runtime switch (default) | Cost when off | Recommendation and the measurement that decides |
|---|---|---|---|---|
| `DEBUG_FS`, `DEBUG_FS_ALLOW_ALL` | Kconfig: "debugfs is a virtual file system that kernel developers use to put debugging files into. [...] If unsure, say N." Where `dri/`, `sched/`, wifi driver state, and (via the automount above) tracing live. | mount it or not. On the running system **neither debugfs nor tracefs is mounted**: `/sys/kernel/debug` and `/sys/kernel/tracing` are empty mount points, and nothing in `init/` mounts them. | none at runtime; every driver's debugfs code is text, not separable from System.map by name | **keep**, and mount it when it is wanted (see below). The tools this document says are "how a stall gets diagnosed" are unreachable until someone runs the mount, which is fine for a shipping system, but worth knowing. |
| `MAGIC_SYSRQ` (`DEFAULT_ENABLE=0x1`, serial too) | The sysrq keys and `/proc/sysrq-trigger`. `kernel.sysrq` reads **1** (everything enabled). | `kernel.sysrq` | a compare in the keyboard handler | **keep.** Load-bearing, not debug: the initramfs and the live ISO reboot and power off by writing to `/proc/sysrq-trigger` (`build-initramfs.sh`, `stages/stage4-iso.sh`). |
| `CGROUP_DEBUG` | Kconfig: "This option enables a simple cgroup subsystem that exports useful debugging information about the cgroups framework. Say N." Shows as `debug` in `/proc/cgroups` (verified), hidden from `cgroup.controllers` on cgroup2 unless `cgroup_debug` is on the command line (from memory). | `cgroup_debug` (unset) | 288 B of `debug_cgrp_subsys`, about 1 KB of file handlers | **remove-now-if-measurement-confirms.** Measurement: bloat-o-meter confirming nothing else changes. Its own help text is the case. |
| `DEBUG_DEVRES` | Kconfig: "If this option is enabled, devres debug messages are printed. Select this if you are having a problem with devres or want to debug resource management for a managed device." The messages are `dev_dbg`, which without `DYNAMIC_DEBUG` (off here) compile to nothing; what remains is a name and size stored in every devres node (from memory of drivers/base/devres.c). | none | `devres_log` 144 B, plus a pointer and a size per managed resource | **remove-now-if-measurement-confirms.** Nothing reads it without dynamic debug. Measurement: bloat-o-meter on `drivers/base/devres.o`. |
| `PNP_DEBUG_MESSAGES` | Kconfig: "Say Y here if you want the PNP layer to be able to produce debug messages if needed. The debug messages are enabled by the 'pnp.debug' kernel parameter." `default y`. | `pnp.debug` (unset) | a flag test per message site, boot only | **keep.** |
| `IWLWIFI_DEVICE_TRACING`, `RTLWIFI_DEBUG`, `NOUVEAU_DEBUG=5`/`_DEFAULT=3`, `BT_DEBUGFS`, `BLK_DEBUG_FS`, `SND_SOC_SOF_DEBUG_PROBES` | Per-driver tracepoints, debug levels and debugfs files. `iwlwifi`, `rtlwifi` and `nouveau` are modules (`=m`), so their debug code is only in memory when the module is. `rtlwifi` is not the RTL8821CE driver; that is `rtw88`, whose `debug_mask` reads **0**. | module parameters, all default off | nothing while the module is not loaded; nop'd tracepoints and level tests when it is | **keep.** Driver bug reports need them, and they cost nothing on a machine without the hardware. |

### Off, and worth knowing

Nothing on the usual list of shipping-kernel surprises is on: `DEBUG_INFO`
(`DEBUG_INFO_NONE=y`, so no DWARF and no BTF), `FUNCTION_TRACER`,
`DYNAMIC_FTRACE`, `STACK_TRACER`, `PROVE_LOCKING`, `LOCK_STAT`, every
`DEBUG_*LOCK*`, `DEBUG_ATOMIC_SLEEP`, `DEBUG_PREEMPT`, `DEBUG_LIST`,
`DEBUG_VM`, `DEBUG_PAGEALLOC`, `PAGE_OWNER`, `PAGE_POISONING`, `KASAN`,
`KFENCE`, `KCSAN`, `UBSAN`, `KMEMLEAK`, `DEBUG_OBJECTS`, `LATENCYTOP`,
`DYNAMIC_DEBUG`, `KGDB`, `KCOV`, `FAULT_INJECTION`. `LOCKDEP_SUPPORT=y` and
`LOCK_DEBUGGING_SUPPORT=y` only say the architecture could; nothing selects
lockdep.

Two absences cut the other way. `SOFTLOCKUP_DETECTOR`, `HARDLOCKUP_DETECTOR`
and `DETECT_HUNG_TASK` are off, so a wedged CPU or a task stuck in D state
for two minutes produces no dmesg line; the hung-task check is one kthread
waking every 120 s, and most desktop kernels ship it. `BPF_SYSCALL` is off
and there is no BTF, so `bpftrace` and the bcc tools are not an option on
this kernel; what the observability in this document amounts to is ftrace
events, kprobe events, blktrace and `perf` with hardware, software and
tracepoint events. Neither is a debug-option removal question, so neither is
decided here.

### Reaching the tools

```
mount -t debugfs none /sys/kernel/debug     # not mounted by init; root
mount -t tracefs none /sys/kernel/tracing
cat /proc/sys/kernel/sched_schedstats        # 0; echo 1 to start collecting
cat /proc/sys/kernel/task_delayacct          # 0; echo 1 for per-task delays
cat /sys/power/pm_trace                      # 0; leave it, see PM_TRACE_RTC
grep 'debug_level =' /sys/module/acpi/parameters/debug_level   # 0x0000000C
dmesg | grep 'W+X'                           # passed, no W+X pages found
```

## SSD trim

The root filesystem is mounted without `discard`, on purpose: an inline
discard is latency on the write path, which is the wrong trade for a desktop.
Free blocks are reported to the drive periodically instead, by the `fstrim`
service (`configs/raven-fstrim`, template `configs/raven/services/fstrim.toml`).
It is a small daemon rather than a cron entry, because Raven has no cron and a
laptop may run for weeks without a reboot: it waits five minutes after start,
then runs `fstrim -av` at idle I/O and lowest CPU priority whenever the stamp
file `/var/lib/raven/fstrim.stamp` is older than seven days, and sleeps an
hour between checks. On the live image it exits at once; there is nothing
behind a squashfs to trim.

```
raven-rc status fstrim                 # running; the log says when it last trimmed
cat /var/log/raven/fstrim.log
sudo raven-fstrim --now                # trim right now, regardless of the stamp
```

Policy in `/etc/raven/fstrim.conf`: `INTERVAL_DAYS`, `ENABLED`, `BOOT_DELAY_SECS`.

## Initramfs

The early userspace image is packed with zstd (`build-initramfs.sh`). It used
to be `gzip -9`, which is the slow choice on both ends: slowest to make and no
faster to unpack. The kernel is built with `RD_ZSTD`, zstd unpacks several
times faster than gzip on a small core, and the image is unpacked on every
boot and packed once. The installer reads gzip, zstd and xz images alike, by
magic bytes, so an ISO built before the switch still installs.

The same script now maps every library to its runtime path before copying it,
and refuses to pack an image that contains a build-tree path. Earlier images
carried a `libc.so.6` under `raven/build/sysroot/usr/lib/` that nothing could
load.
