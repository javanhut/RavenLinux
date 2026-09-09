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
Their runtime switches are off by default, so they cost text size and nothing
measurable on hot paths, and they are the tools a stall gets diagnosed with.
Remove one only after measuring what its absence buys, and record the number
in the script.

`vm.swappiness` stays at the kernel default. With zswap in front of the
partition the old advice to lower it no longer applies.
