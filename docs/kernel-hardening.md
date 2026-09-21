# Kernel: hardening, LSMs and hybrid scheduling

What `configs/kernel/config-6.17-raven` turns on in the security and
hybrid-scheduling areas, why, and what `scripts/check-kernel-hardening.sh`
refuses to let a future config regeneration drop again.

The config is not a fragment. It is a full 216 KB `.config` that
`build-kernel.sh` restores verbatim -- `cp saved .config`, `make olddefconfig`,
`kernel-ports.sh`, `kernel-performance.sh`, `make olddefconfig` -- so an option
that is not written in that file is not in the shipped kernel, and nothing in
the build path re-derives the security set. `kernel-ports.sh` and
`kernel-performance.sh` protect their own options by reapplying them on every
build. The security options had no such guard, and it showed.

## What was missing

The 6.17.11-raven kernel shipped without options that the Arch kernel it
replaced has on. Read from the running machine rather than from the file, so
these are what was actually built:

| Option | Was | Now | How it was confirmed |
|---|---|---|---|
| `HARDENED_USERCOPY` | off | **y** | no `__check_heap_object`, no `usercopy_abort` in `/proc/kallsyms` |
| `SLAB_FREELIST_RANDOM` | off | **y** | `/proc/config.gz` |
| `SLAB_FREELIST_HARDENED` | off | **y** | `/proc/config.gz` |
| `INIT_ON_ALLOC_DEFAULT_ON` | off | **y** | `/proc/config.gz` |
| `SECURITY_YAMA` | off | **y** | `/proc/config.gz`; no `kernel.yama.ptrace_scope` sysctl |
| `SECURITY_LANDLOCK` | off | **y** | `/proc/config.gz` |
| `INTEL_HFI_THERMAL` | off | **y** | `/proc/config.gz`; no HFI symbols in `/proc/kallsyms` |
| `LSM` | every LSM the kernel knows | **`"landlock,yama"`** | see below |
| `INIT_ON_FREE_DEFAULT_ON` | off | off, on purpose | see below |
| `MODULE_SIG` | off | off, on purpose | see below |

The argument for each is in a comment at the symbol itself in the config file.
This page is the version that survives a `make menuconfig` session being copied
back over that file, which is a thing that removes every comment in it.

## Why `INIT_ON_FREE_DEFAULT_ON` stays off

`INIT_ON_ALLOC_DEFAULT_ON` zeroes pages and heap objects on the way *out* of
the allocator. That closes the entire "uninitialised kernel memory leaked to
userspace" class, and it is nearly free because the memory being zeroed is
about to be written by its new owner anyway -- upstream measures it at under
one percent.

`INIT_ON_FREE_DEFAULT_ON` zeroes the same memory a second time on the way
*back in*. That pass is not overlapped with anything; upstream's own help text
puts it at 3-5%, and it is worse on allocation-heavy workloads. What it buys
over init-on-alloc is narrow: it shortens the window in which freed data sits
in a slab that nothing has reallocated yet. That helps against a
use-after-free *read*. It adds nothing against the leak-to-userspace class,
which the allocation side already covers, and the double free that makes
use-after-free exploitable in the first place is what `SLAB_FREELIST_HARDENED`
catches.

A few percent of a laptop's CPU, permanently, for that remainder is the wrong
trade on a desktop machine. `init_on_free=1` on the kernel command line turns
it on for anyone who disagrees about a particular machine, with no rebuild.

## Why `MODULE_SIG` stays off

Not because it is a bad idea, and not because flipping the symbol is hard.
Because enabling it honestly is four separate pieces of work, and enabling it
dishonestly gets a taint flag and nothing else.

**It is not one symbol.** `MODULE_SIG` plus `MODULE_SIG_ALL` (otherwise
`make modules_install` installs modules unsigned and the kernel distrusts its
own), plus a hash choice (`MODULE_SIG_SHA512`), plus `MODULE_SIG_KEY` pointing
at a private key. Leave `MODULE_SIG_KEY` at its default and the kernel
generates `certs/signing_key.pem` itself on every clean tree, which means every
rebuild produces a kernel that distrusts the modules the previous build
installed, and the build stops being reproducible.

**The key has to live somewhere, and not in this repository.** A persistent key
injected by the build (a CI secret, a file outside the tree) is the only
arrangement that survives a rebuild. `install_headers()` in
`scripts/build-kernel.sh` copies `scripts/` into
`lib/modules/<release>/build` so that out-of-tree modules can be built on the
installed system; it must never grow a copy of `certs/`, because the private
half of a signing key on every user's disk is not a signing key.

**Out-of-tree modules are where it actually breaks.** `scripts/build-evdi.sh`
builds evdi against the same kernel tree in the same build session, so
`modfinal` would sign it and it would load. DKMS on an installed machine would
not: there is no key there. A module the user rebuilds themselves -- evdi after
a DisplayLink update, nvidia, anything -- comes out unsigned. Without
`MODULE_SIG_FORCE` it still loads and taints the kernel with

    module verification failed: signature and/or required key missing

With `MODULE_SIG_FORCE` it does not load at all, and the dock stops working the
first time the user updates the module.

**And `MODULE_SIG_FORCE` is the only setting that enforces anything.** The
other enforcement path is lockdown in integrity mode, which needs
`SECURITY_LOCKDOWN_LSM` (off) and a Secure Boot chain to trigger it.
RavenLinux has no such chain: `raven-install` writes an unsigned
`raven-boot.efi` to `\EFI\BOOT\BOOTX64.EFI` and the installer's preflight tells
the user to turn Secure Boot off. Signed modules under an unsigned bootloader
and an unsigned kernel verify a link in a chain whose other links are missing.

The order, when this is done: sign `raven-boot.efi` and the kernel first; then
`MODULE_SIG` + `MODULE_SIG_ALL` with a persistent key injected by the build;
then `MODULE_SIG_FORCE`, and only once a user can enrol their own key (MOK,
`sbctl`) so DKMS still works on their machine. `scripts/check-kernel-hardening.sh`
will print a warning rather than fail if someone turns it on, so that this page
gets updated in the same change.

## The LSM list

`CONFIG_LSM` was the kernel's full default -- every LSM the kernel knows, in
the canonical order. It is now `"landlock,yama"`.

The removed names fall into two groups. Most of them were never built at all:
`lockdown`, `loadpin`, `safesetid`, `smack`, `tomoyo`, `ipe`, and `bpf` (which
needs `BPF_SYSCALL`, off here). The other two, `selinux` and `apparmor`, are
compiled in and were being initialised with no policy to load. RavenLinux ships
no `/etc/selinux`, no `apparmor_parser`, no profiles and no service that would
load them; an LSM that initialises empty costs its hooks on every syscall path
it registered for and enforces nothing.

What is left is the two that need no policy and no userspace at all:

- **landlock** -- unprivileged sandboxing. It enforces nothing until a process
  asks for it with `landlock_create_ruleset(2)`, so its cost on a system where
  nothing asks is a handful of hooks that return immediately, and the day
  something does ask it is there instead of being a kernel rebuild away.
- **yama** -- `kernel.yama.ptrace_scope`, default 1: one unprivileged process
  cannot `ptrace` another that is not its own child. That is the step between
  "a browser tab got code execution" and "it read the session keys out of every
  other process you are running". A debugger still attaches to processes it
  started itself, which is the case that matters in practice.

`capability` is not listed because it is `LSM_ORDER_FIRST` and always loads;
`integrity` is `LSM_ORDER_LAST` for the same reason. SELinux and AppArmor stay
*compiled in* deliberately rather than being switched off: `lsm=` on the kernel
command line replaces `CONFIG_LSM` entirely, so a machine that grows a policy
can turn one on without a rebuild.

**None of this is inspectable yet.** `/sys/kernel/security` is empty on the
running machine because nothing mounts securityfs -- `raven-init` knows the
name but the mount never happens -- so there is no `/sys/kernel/security/lsm`
to read the active list back from. That mount is the missing half of this
change and lives in another file.

## Hybrid scheduling: what ITMT is doing, and what it is not

The reference machine is a Meteor Lake Core Ultra 5 135U: 2 P-cores with SMT, 8
E-cores, 2 low-power E-cores on the SoC die, 14 threads.
`/sys/devices/cpu_core/cpus` is `0-3` and `/sys/devices/cpu_atom/cpus` is
`4-13`.

### `/proc/sys/kernel/sched_itmt_enabled` is missing because it moved

This looked like ITMT had never registered. It had not: the control moved from
sysctl to debugfs, and RavenLinux never mounts debugfs.

Evidence, all from the running 6.17.11-raven kernel:

```
$ grep -i itmt /proc/kallsyms
... sched_set_itmt_support
... sched_set_itmt_core_prio
... sched_itmt_enabled_write
... dfs_sched_itmt_fops          <- a file_operations
... dfs_sched_itmt               <- a dentry
... intel_pstste_sched_itmt_work_fn
... sysctl_sched_itmt_enabled

$ grep -iE 'itmt_kern_table|itmt_sysctl_header' /proc/kallsyms
(nothing)

$ grep -E 'debugfs|securityfs' /proc/mounts
(nothing)
```

`dfs_sched_itmt` and `dfs_sched_itmt_fops` are the debugfs form; the sysctl
table and its header, which is what a `register_sysctl()` build would have, do
not exist in this kernel at all. `sysctl_sched_itmt_enabled` survives as the
name of the *variable* the debugfs file is bound to, which is what makes the
symbol list look misleading at first glance. The file is created in
`debugfs_sched`, so its path is `/sys/kernel/debug/sched/sched_itmt_enabled`,
and `/sys/kernel/debug` is empty because nothing mounts debugfs. `DEBUG_FS=y`
and `DEBUG_FS_ALLOW_ALL=y`, so it is mountable -- nobody mounts it.

ITMT is therefore almost certainly *already on*: `sched_set_itmt_support()`
sets the enable flag itself when it succeeds, and everything it needs is
present. `intel_pstate` is the active driver in `active` mode with HWP enabled
(`intel_pstate: HWP enabled` at 1.18s in dmesg), `ACPI_CPPC_LIB=y`, and the
per-CPU CPPC ranking that feeds `sched_set_itmt_core_prio()` is populated and
non-uniform, which is the condition that triggers registration:

```
$ for c in /sys/devices/system/cpu/cpu*/acpi_cppc/highest_perf; do ...
cpu0-3  : 55     (P-cores)
cpu4-11 : 36     (E-cores)
cpu12-13: 21     (low-power E-cores)
```

Three distinct values, so `max_highest_perf > min_highest_perf` and
`intel_pstste_sched_itmt_work_fn` schedules `sched_set_itmt_support()`.

**This cannot be confirmed from userspace without mounting debugfs**, which
needs root and is another agent's file. The check is one line once it is
mounted:

```
mount -t debugfs none /sys/kernel/debug
cat /sys/kernel/debug/sched/sched_itmt_enabled          # expect 1
cat /sys/kernel/debug/sched/domains/cpu0/domain1/flags  # expect SD_ASYM_PACKING
```

`/proc/schedstat` is the part that *is* readable without debugfs, and it
confirms the topology the scheduler built: `cpu0` has `domain0 SMT 0003`,
`domain1 MC 0fff`, `domain2 PKG 3fff` -- note that the MC domain stops at CPU
11, so the two low-power E-cores on the SoC die are correctly in a domain of
their own. Domain flags are not printed there, which is why the ITMT check
needs debugfs.

### `INTEL_HFI_THERMAL`

Now on. It was `# CONFIG_INTEL_HFI_THERMAL is not set` -- present in the config
and disabled, not absent from it. Its Kconfig dependencies are already
satisfied: `CPU_SUP_INTEL=y`, `X86_THERMAL_VECTOR=y`, and it selects
`THERMAL_NETLINK`, which was already `y`. Nothing else had to move.

What it does: the firmware maintains a table in memory saying which core class
is currently the fastest and which is currently the most efficient, and
rewrites it as the package heats up, throttles, or has its power limit changed.
Without the driver the kernel never reads that table, and the only hybrid
signal anything gets is the static CPPC ranking above, which cannot notice a
P-core that is currently the slow one.

What it does **not** do, because this is easy to over-claim: mainline has no
in-kernel consumer of HFI. `intel_hfi.c` turns the firmware's updates into
thermal-netlink events for user space, which is what `thermald` and
`intel-lpmd` act on, and RavenLinux ships neither yet. The scheduler's own
hybrid awareness is ITMT asym-packing fed by CPPC through `intel_pstate`, which
is the mechanism described above and is independent of this option. So this
enables the data; a consumer for it is separate work.

## The checker

`scripts/check-kernel-hardening.sh` holds the required list and diffs it
against a config. It runs standalone with nothing but bash:

```
./scripts/check-kernel-hardening.sh                  # the checked-in config
./scripts/check-kernel-hardening.sh /proc/config.gz  # what actually got built
./scripts/check-kernel-hardening.sh build/sources/linux-6.17.11/.config
```

It is deliberately *not* a floor script in the shape of `kernel-ports.sh` and
`kernel-performance.sh`. Those edit a kernel tree's `.config` with the kernel's
own `scripts/config`, so they need a kernel tree and can only run during a
build. This one reads a file, so it runs in CI on a machine that will never
compile a kernel -- it is wired into the `shellcheck` job of
`.github/workflows/ci.yml`, next to the desktop-build contract test, without
`continue-on-error`. And it reports rather than repairs: a config that has
drifted is something a person should look at, not something a build should
quietly paper over.

Two options are listed as *deliberately off* (`MODULE_SIG`,
`INIT_ON_FREE_DEFAULT_ON`). Turning one on prints a warning and does not fail
the build, so that improving the kernel is never blocked by the checker -- but
it does not happen silently either, and the warning says to update the
reasoning here at the same time.

The checker also warns, when run against the checked-in config, if the
hardening comment blocks have vanished from it. That is the signature of a
`make menuconfig` session copied back over the file: `make olddefconfig`
carries the settings across and drops every comment.

## Built-in drivers: candidate list for a smaller vmlinuz

`vmlinuz` is 17 MB (zstd-compressed), with 686 entries in
`/lib/modules/6.17.11-raven/modules.builtin` against 361 loadable modules. The
list below is **candidates only**. Nothing here has been changed: an unbootable
kernel is a far worse outcome than a large one, and every line needs a human to
agree and a QEMU boot to confirm.

The constraint that decides every one of these is stated in
`etc/raven/init.toml` and restated in `kernel-ports.sh`: *anything that needs
firmware from `/lib/firmware` must be a module*, because a built-in driver asks
the firmware loader for its blobs at probe -- before any filesystem holding
firmware is mounted -- and never retries. Everything on the path to a console
or to the root filesystem must be built in, because
`scripts/build-initramfs.sh` puts no modules in the initramfs at all.

Sizes are text only, summed from next-symbol deltas in
`/lib/modules/6.17.11-raven/build/System.map`. They exclude rodata, data and
init sections, so they understate the object; the image is zstd-compressed, so
the saving on `vmlinuz` is smaller than the number. They are for ranking, not
for a budget.

| Candidate | ~text | Why it is a candidate | What to check first |
|---|---|---|---|
| `RT2800PCI`, `RT2800USB` and the `RT2X00_LIB*` set | ~157 KiB | **This one is a policy violation, not just size.** `CONFIG_RT2X00_LIB_FIRMWARE=y` -- these drivers load `rt2870.bin`/`rt2860.bin` from `/lib/firmware`, built in, at probe, which is exactly the failure `init.toml` documents. Ralink USB dongles are also not this machine's hardware. | That no supported machine ships a Ralink card as its only NIC. `=m` and let `raven-udev` coldplug it like every other wireless driver. |
| `DRM_VMWGFX` | ~178 KiB | VMware guest GPU. Cannot appear on a physical laptop. | Nothing boots RavenLinux under VMware in the test matrix. `=m` is safe; `=n` is probably right. |
| `DRM_VIRTIO_GPU` | ~38 KiB | QEMU guest GPU. | `scripts/test-qemu.sh` uses `virtio-vga-gl` for its desktop mode. As a module it loads after the root mount, which is before the Wayland session starts -- but the early console would fall back to `simpledrm` on the OVMF GOP. Boot the ISO in QEMU before believing this. |
| `DRM_QXL` | ~34 KiB | SPICE guest GPU; nothing in the test harness uses it. | Same QEMU check. |
| `DRM_BOCHS` | ~4.7 KiB | QEMU `bochs-display`, which `test-qemu.sh` uses for its large-resolution path. | Small enough that it is probably not worth the risk. Listed for completeness. |
| `DRM_CIRRUS_QEMU` | ~3.7 KiB | Obsolete QEMU VGA. | Same. |
| `DRM_VBOXVIDEO` | ~8.2 KiB | VirtualBox guest GPU. | Nothing in the test matrix uses VirtualBox. |
| The `CROS_EC*` / `CHROMEOS_*` set (15 built-in modules) | ~57 KiB | ChromeOS embedded controller. RavenLinux does not target Chromebooks and the installer has no Chromebook path. | Whether anyone intends to support Chromebooks. If not, `=n` rather than `=m`. |
| `SURFACE_*` (11 built-in modules) | ~41 KiB | Microsoft Surface aggregator, HID, DTX, GPE. Same argument. | Same. |
| `SND_HDA_CODEC_ALC*` + the other legacy HDA codecs | ~64 KiB (ALC only) | Every HDA codec family is built in: Analog, SigmaTel, VIA, Conexant, CA0132 *with its DSP*, C-Media, Si3054, Cirrus, and eleven separate Realtek families. A machine has one codec. | HDA codecs do not need firmware, so `=m` is safe on the firmware rule -- but the codec must be probed before the session wants audio, and `raven-udev` coldplugs after the root mount, which it is. Keep `SND_HDA_INTEL` and `SND_HDA_GENERIC` built in; modularise the per-vendor codecs. |
| The legacy USB-ethernet set: `catc`, `kaweth`, `pegasus`, `rtl8150`, `sr9700`, `sr9800`, `plusb`, `net1080`, `zaurus`, `cdc_subset`, `cdc_eem` | ~82 KiB | USB 1.1 and early USB 2 adapters, none of which anyone plugs into a 2024 laptop. | Keep `usbnet`, `cdc_ether`, `cdc_ncm`, `r8152`, `asix`, `ax88179_178a` built in -- a USB dock's ethernet is a plausible install-time network. Modularise the rest. |
| The bluetooth transport drivers (15 built-in modules) | ~111 KiB | `btusb` and friends. `btintel`, `btrtl`, `btbcm`, `btqca`, `btmtk` and `btnxpuart` **all load firmware**, which makes built-in the wrong answer by the project's own rule, not merely a large one. | Nothing needs bluetooth before the root mount. This looks like a straightforward `=m` for the whole directory, and it fixes a firmware-timing bug at the same time. Verify a Bluetooth mouse still pairs after the change. |
| The `HID_*` device drivers (52 built-in, ~227 KiB across the directory) | ~227 KiB | Wacom, UC-Logic, Waltop, Nintendo, PlayStation, Steam, Razer, Corsair, Stadia force feedback, and so on. | **Be careful here.** `HID_GENERIC`, `HID_MULTITOUCH`, `USB_HID` and `I2C_HID` must stay built in -- the installer runs on a keyboard and a touchpad, and there are no modules in the initramfs. The exotic gamepads and graphics tablets are the candidates; the input path is not. |
| The USB gadget functions (`usb_f_acm`, `usb_f_ncm`, `usb_f_ecm`, `usb_f_eem`, `usb_f_rndis`, `usb_f_mass_storage`, `usb_f_fs`, `libcomposite`) | ~32 KiB | These make the machine appear *as* a USB device to another host. A laptop with no UDC hardware can never use them. | Whether any target board has a USB device controller. If none does, `=n`. |

Not on the list, deliberately: `DRM_SIMPLEDRM` and the `DRM`/`DRM_KMS_HELPER`
core stay built in -- `simpledrm` on the EFI framebuffer is the console before
`i915`/`xe` load, and those two are already modules. `BTRFS_FS`, `EXT4`,
`DM_CRYPT`, `BLK_DEV_DM`, NVMe and AHCI stay built in because the initramfs has
no modules and they are the path to the root filesystem. The crypto directory
(44 built-in modules) stays built in for the same reason -- `DM_CRYPT` pulls on
it, and LUKS support is planned.
