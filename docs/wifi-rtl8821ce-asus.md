# RTL8821CE wifi on the ASUS ROG laptop: issues found, fixes, and TODOs

Status as of 2026-08-26: **working** on the build from seal `c1642e2e`
(box seal `98a311ae`). Do not touch the items under "TODO" until there is
time to re-test; the machine only just came back on the air.

## Symptom

Card: Realtek RTL8821CE, PCI `0000:03:00.0`, driver `rtw88_8821ce`, interface
`wlan0`. Firmware loads (`Firmware version 24.11.0, H2C version 12`). Every
attempt to bring the interface up fails inside the driver's MAC power-on:

```
rtw88_8821ce 0000:03:00.0: failed to poll offset=0x6 mask=0x2 value=0x2
rtw88_8821ce 0000:03:00.0: mac power on failed
rtw88_8821ce 0000:03:00.0: failed to power on mac
```

`caw port up wlan0` reports the kernel's errno for that:
`kernel returned Resource busy (os error 16)`. Once wedged, a driver reload,
`remove`/`rescan`, or a PCI bus reset makes even the *probe* fail
(`probe with driver rtw88_8821ce failed with error -16`). Only a cold boot
recovers the chip.

The precise pattern: the driver powers the MAC on once at probe (to read the
efuse) and that works; it powers off; every power-on after that fails.

## Root cause

The kernel config changed in seal `1cd6b98f` (2026-08-24). Compared with the
last build on which the card worked (June 2026, GitHub `4871f43f`), the kernel
*source* was byte-identical (`linux-6.17.11`) but the config gained:

| option | June (worked) | after 1cd6b98f |
|---|---|---|
| `CONFIG_ASUS_WMI`, `CONFIG_ASUS_NB_WMI` | not set | `=m` |
| `CONFIG_AMD_PMC` | not set | `=m` |
| `CONFIG_RTW88*` | `=y` | `=m` |

`asus_nb_wmi` is loaded by udev a few seconds into userspace -- between the
card's probe and cawd's first `port up`. At load it talks to the embedded
controller about wireless (`INIT`, `WAPF`/`CWAP` WMI calls), and on this board
that leaves the card in a state the driver's power sequence cannot get out of.
Same family as the long-known "ASUS laptop, wifi dead, add `asus_nb_wmi
wapf=4`" problem.

## Fix that shipped

`scripts/stages/stage2-native.sh` writes `/etc/modprobe.d/asus-wmi.conf`:

```
blacklist asus_nb_wmi
blacklist asus_wmi
```

Cost: ASUS hotkeys and fan/platform-profile WMI. Nothing the console system
uses.

## Also in the image from the same investigation

These went in while the cause was still unknown. None of them fixed the
problem on its own, and the build that works has all of them. Sorting out
which can go is the TODO below.

- `/etc/modprobe.d/pcie-aspm.conf`: `options pcie_aspm policy=performance`.
  Disables PCIe ASPM on **every** link in the machine -- costs idle power.
  Added in the same build as the blacklist, so not yet separated from it.
- `/etc/modprobe.d/rtw88.conf`, `rtw89.conf`: `disable_aspm=1`. Proven
  insufficient on its own (verified `Y` in `/sys/module`, still failed).
- raven-init `apply_builtin_module_options()`: applies `options` lines from
  `/etc/modprobe.d` to built-in drivers via `/sys/module/<mod>/parameters`
  before its PCI re-probe. Needed because `rtw88` was built in (`=y`) at the
  time and modprobe.d does nothing for built-in drivers. Keep: it is the only
  thing that makes modprobe.d honest for `=y` drivers.

## Ruled out (so nobody re-runs these)

- **caw / cawd**: `port up` is one `RTM_NEWLINK` with `IFF_UP`; cawd sends
  nothing else before scanning and never touches rfkill. The kernel's own
  re-probe fails with no userspace involved. cawd's reconnect backoff is why
  the failure repeats in dmesg; it is the messenger.
- **iwd**: was still installed and *enabled* by stage2's fallback init.toml,
  fought cawd for the wiphy, and exited 1 at every start -- a real mess, but
  not this bug (it exited before touching the card). Removed entirely along
  with iwctl, iwmon, ELL, wpa_supplicant, iw and libnl3.
- **Driver-side ASPM** (`rtw88_pci.disable_aspm=1`): no effect.
- **Host-side ASPM at runtime** (`pcie_aspm` policy, L1 substates off): no
  effect on an already-wedged chip.
- **Runtime PM**: device was `active` / `control=on`.
- **Power sequences**: kernel 6.17's `rtw8821c` on/off sequences match
  Realtek's vendor driver step for step.
- **Kernel version**: identical source between working and broken builds.

Every runtime test after the first failure in a boot is inconclusive: the
chip needs a cold boot, and a workaround only counts if it is in effect
before the first power-off (i.e. at boot).

## TODO -- one variable per round, cold boot between rounds

1. **Drop `pcie-aspm.conf`** and rebuild. If wifi survives, also drop
   `rtw88.conf` / `rtw89.conf`. Expected outcome: all three go.
2. **Replace the blacklist with `options asus_nb_wmi wapf=4`** in
   `asus-wmi.conf`. Same effect on wireless, but restores ASUS hotkeys and
   platform profiles. If it fails, go back to the blacklist.
3. Decide whether `CONFIG_RTW88*` should return to `=y` (the June state). Not
   needed for wifi; only matters for boot-time probe ordering. Note that
   `build/sources/linux-6.17.11/.config` on the build box is reused as-is, so
   a saved-config edit needs that file deleted or the kernel rebuilds
   unchanged.
4. Unrelated but seen in the build log: `fish build failed` -- CMake error
   `set_property could not find TARGET tests_buildroot_target` (fish 3.7
   tarball vs CMake 4.x on the Arch build host). Build falls back to bash.

## Test recipe

Cold boot (unplug, hold power 15 s -- a warm reboot can leave the chip
wedged), then:

```
lsmod | grep asus                 # empty while the blacklist is in
dmesg | grep rtw88                # no "mac power on failed"
sudo caw port up wlan0
```

## Build box notes

Builds run on the Arch box (`ssh raven`, 192.168.1.78) as `sudo imlazy build`;
`build/` is root-owned, so a rootless run dies at its first log write. Logs:
`build/logs/build_<timestamp>.log`. `ivaldi sync` needs `--yes` when
non-interactive. Before flashing, confirm the sysroot has the file:
`ls build/sysroot/etc/modprobe.d/asus-wmi.conf`.
