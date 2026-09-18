# Kernel Ports and Peripherals

What the Raven kernel does with every physical port on a laptop, where that
support comes from, and what userspace does with the result.

Half of this document used to stop at the driver. That was the gap worth
closing: `USB_PRINTER` had been built in for years while the image carried
nothing that could print, and the USB storage stack was complete while plugging
in a drive produced a device node and no directory. A port is not supported
because the kernel enumerates it. The sections below say, for each one, both
halves -- and "What is still missing" at the end says where a half is genuinely
absent and why.

The option set lives in two places that must agree: `configs/kernel/config-6.17-raven`
is what the build uses, and `scripts/kernel-ports.sh` is the list of port and
peripheral options that config must never drop below. `build-kernel.sh` applies
the script every time it restores the saved config, so a `menuconfig` session or
a kernel version bump cannot silently lose a port. Run
`scripts/kernel-ports.sh build/sources/linux-<ver>` followed by `make olddefconfig`
to apply it by hand.

Module policy is the one the rest of the config follows: anything that loads
firmware is a module, because a built-in driver asks for its blobs before the
root filesystem exists and never asks again (`configs/raven-udev` has the
history); anything on the path to a console or the root filesystem is built in;
the rest is built in when small and a module when large.

## Ethernet

| Port | Driver | Option |
|---|---|---|
| Intel onboard | e1000e, igb, igc | `E1000E=y IGB=y IGC=y` |
| Realtek onboard (most laptops) | r8169 | `R8169=y` |
| Aquantia 2.5/5/10G (docks, workstations) | atlantic | `AQTION=y` |
| Atheros / Killer E2xxx (Asus, MSI) | alx, atl1c | `ALX=y ATL1C=y` |
| USB and dock adapters | r8152, ax88179, cdc_ether, cdc_ncm, lan78xx, smsc95xx | all `=y` |
| Thunderbolt/USB4 networking | thunderbolt-net | `USB4_NET=y` |

PHYs: Realtek, Marvell, Broadcom, Micrel and Aquantia are built in. A board with
an unlisted PHY falls back to the generic driver, which usually works.

Getting an address after the cable goes in is userspace's job. `raven-dhcp`
runs once at boot from `init.toml`; the `ports` service (`raven-ports watch
--react`) watches link state over rtnetlink and runs the same command when a
wired link comes up later, which is what makes a cable plugged in after boot,
or a dock's NIC, get a lease.

`raven-dhcp` is `configs/raven-dhcp`, a wrapper around `dhcpcd` rather than a
DHCP client of its own. What it owns is the two decisions above the protocol:
which links to run on -- ethernet with a real device behind it, never the
wireless one, which is `cawd`'s -- and not starting a second client on a link
that already has one, which is what stops a reseated cable leaving a pile of
daemons behind. `raven-dhcp --list` prints what it would serve and configures
nothing; it is the first thing to run when a cable is in and nothing happened.

## USB

USB 1.1 through 3.2 need only the host controllers, all built in: `USB_XHCI_HCD`
`USB_XHCI_PCI` `USB_EHCI_HCD` `USB_OHCI_HCD` `USB_UHCI_HCD`.

`USB_XHCI_PCI_RENESAS`, for the uPD720201/2 chips on add-in cards, is the one
exception and it is a **module**. It uploads `renesas_usb_fw.mem` at probe, and
a built-in driver probes during PCI enumeration -- before any root filesystem
exists -- so it asked, got nothing, and never asked again. The firmware is in
the image (`stage2-native.sh` copies `renesas/`) but not in the initramfs, which
carries only `regulatory.db`. As a module it loads after the real root is
mounted, which is the same rule every other firmware-loading driver here
follows. The consequence, and it is worth knowing: a machine that boots *from* a
drive behind one of these cards will not find its root. Built-in USB 3 ports are
unaffected -- they are the other symbols above.

Device classes built in: storage and UAS, HID, printers,
audio (`SND_USB_AUDIO`), serial (`USB_ACM` for modems and Arduino-class boards,
`USB_SERIAL` with FTDI, CP210x, PL2303, CH341, and the WWAN set), USB ethernet
(above), and video (`USB_VIDEO_CLASS=m`, the webcam class driver).

### USB-C

A USB-C port with no Type-C support is a USB-A port with a different plug.
Power delivery, alternate modes and role switching all need the connector
class:

```
CONFIG_TYPEC=y
CONFIG_TYPEC_UCSI=y            # the ACPI port-manager interface most laptops use
CONFIG_UCSI_ACPI=y
CONFIG_UCSI_CCG=y              # Cypress CCGx, including the one on NVIDIA GPUs
CONFIG_UCSI_STM32G0=y
CONFIG_TYPEC_TCPM=y            # discrete port controllers (Chromebooks, some Dell)
CONFIG_TYPEC_TCPCI=y  FUSB302  TPS6598X  ANX7411  RT1719  HD3SS3220  STUSB160X  WUSB3801
CONFIG_TYPEC_DP_ALTMODE=y      # DisplayPort over USB-C
CONFIG_TYPEC_TBT_ALTMODE=y     # Thunderbolt over USB-C
CONFIG_TYPEC_NVIDIA_ALTMODE=y  # VirtualLink
CONFIG_TYPEC_MUX_*             # the orientation/mode muxes those need
CONFIG_USB_ROLE_SWITCH=y
CONFIG_USB_ROLES_INTEL_XHCI=y
CONFIG_INTEL_SCU_PCI=y         # for TYPEC_MUX_INTEL_PMC
```

With these, `/sys/class/typec/` lists each port and its partner, and a
DisplayPort alt-mode monitor on a USB-C port appears as a normal DRM connector
on whichever GPU the port is wired to.

### USB4 / Thunderbolt

```
CONFIG_USB4=y
CONFIG_USB4_NET=y
CONFIG_INTEL_WMI_THUNDERBOLT=y
CONFIG_HOTPLUG_PCI_PCIE=y      # Thunderbolt devices arrive as PCIe hotplug
CONFIG_HOTPLUG_PCI_ACPI=y
```

Docks, eGPUs and DisplayPort tunnelling all go through this. Devices show
under `/sys/bus/thunderbolt/`; authorisation is left at the kernel default
(authorised when the firmware says so), with no userspace `boltd`. On a machine
whose firmware sets the security level to `user`, that means a device arrives
unauthorised and has to be let in by hand:

```
cat /sys/bus/thunderbolt/devices/domain0/security   # none, user, secure, dponly
echo 1 > /sys/bus/thunderbolt/devices/0-1/authorized
```

### Dual role: being the device on the cable

`USB_ROLE_SWITCH` above only flips the port. Actually being the device at the
other end of the cable -- tethering out to a phone, `g_ether` to a second
machine, a serial console over USB -- needs a gadget stack and a UDC, and on a
laptop the UDC is dwc3:

```
CONFIG_USB_GADGET=y
CONFIG_USB_CONFIGFS=y          # plus NCM, ECM, RNDIS, EEM, MASS_STORAGE, ACM, F_FS
CONFIG_USB_DWC3=m  CONFIG_USB_DWC3_PCI=m
CONFIG_USB_DWC3_DUAL_ROLE=y
```

Gadgets are assembled through configfs under `/sys/kernel/config/usb_gadget/`.
Nothing in the image does this for you; the point is that the kernel can.

## Mobile broadband

The WWAN card in a business laptop needs the WWAN class, and without it the
modem enumerates and stops there -- no `/dev/wwan*`, no control port, nothing
for a dialler to talk to.

| Modem | Path | Option |
|---|---|---|
| Qualcomm SDX over PCIe (Dell, Lenovo, HP WWAN SKUs) | MHI | `WWAN=y MHI_BUS_PCI_GENERIC=m MHI_WWAN_CTRL=m MHI_WWAN_MBIM=m MHI_NET=m` |
| USB MBIM | cdc_mbim | `USB_NET_CDC_MBIM=y` |
| USB QMI (older Sierra, Quectel) | qmi_wwan | `USB_NET_QMI_WWAN=m` |
| USB serial control ports | option, sierra | `USB_SERIAL_OPTION=y` and the WWAN set |

The USB half of this was on for years while the class itself was off, so the
PCIe modems -- which is what is actually fitted to laptops now -- had a driver
and no interface to expose it through.

Getting an address is `cawd`'s, not `raven-dhcp`'s; this is the layer below it.

## Displays

| Port | Driver |
|---|---|
| Laptop panel, HDMI, DisplayPort, DVI on the GPU | amdgpu, i915, xe, nouveau -- all modules |
| DisplayPort over USB-C | same GPU driver, via `TYPEC_DP_ALTMODE` |
| DisplayPort over USB4/Thunderbolt | same GPU driver, via `USB4` |
| DisplayLink DL-1x5 (USB 2) | udl (`DRM_UDL=m`) |
| DisplayLink DL-3xxx/5xxx/6xxx (USB 3, USB-C) | evdi, out of tree -- see below |
| Generic USB display class | gud (`DRM_GUD=m`) |

`VGA_SWITCHEROO=y` lets a hybrid-graphics laptop power its discrete GPU down when
idle. `DRM_XE=m` covers Intel Lunar Lake, Battlemage and later, which i915 does
not. `DRM_DISPLAY_DP_AUX_CHARDEV=y` exposes `/dev/drm_dp_auxN` for dock and
monitor tooling.

Which port belongs to which GPU is a hardware fact the kernel reports and the
compositor has to respect: on most gaming laptops the HDMI and USB-C outputs are
wired to the discrete GPU while the panel is on the integrated one, so driving
every port means driving both GPUs. That is why the nouveau firmware is shipped
by default (`RAVEN_FW_NVIDIA=0` in the build drops it) and why huginn treats
every DRM node as an output source rather than only the primary.

### DisplayLink

`scripts/build-evdi.sh` builds DisplayLink's `evdi` module against the Raven
kernel and installs it under `extra/`; stage1 runs it after the kernel. evdi
is only the kernel half: it creates a DRM device per attached screen and
receives frames from DisplayLink's proprietary `DisplayLinkManager`, which the
EULA does not let the image carry. On a machine with a DisplayLink dock, install
DisplayLink's Ubuntu package (the daemon is a single static binary) and the
dock's screens appear as `/dev/dri/cardN`.

## Audio

Every HDA codec family is built in, not only Realtek: Conexant (ThinkPads),
Cirrus (Dell, HP, Apple), Analog Devices, Sigmatel/IDT, VIA, C-Media, CA0132,
CS8409, plus `SND_HDA_RECONFIG` and `SND_HDA_PATCH_LOADER` for pin quirks from
userspace. HDMI/DP audio is `SND_HDA_CODEC_HDMI` with the Intel, ATI and NVIDIA
variants.

Laptops from roughly 2019 on do not route their speakers and microphones
through HDA alone; they use the Sound Open Firmware DSP (Intel) or the ACP
(AMD), and the internal microphone in particular is only reachable that way.
Those are the ASoC/SOF modules: `SND_SOC=m`, `SND_SOC_SOF_TOPLEVEL=y` with every
Intel platform from Apollo Lake to Panther Lake, `SND_SOC_SOF_AMD_*` from Renoir
to ACP 7.0, `SND_SOC_AMD_ACP6x` and `SND_SOC_AMD_YC_MACH` for the Ryzen 6000+
DMIC, SoundWire, and the machine drivers for the common codec pairings. The
smart amplifiers those laptops hang off HDA (Cirrus CS35L41/CS35L56, TI
TAS2781) are modules on I2C and SPI, which is why `SPI`, `SPI_PXA2XX` and
`SPI_AMD` are on.

All of this needs firmware, which `stage2-native.sh` now copies: `intel/sof*`
(the DSP images and topologies), `amd/`, `cirrus/`, `ti/`.

Jack detection is a kernel event (an input device named `HDA ... Headphone`)
and a PipeWire/WirePlumber policy; nothing in the kernel config needs to change
for it.

### An internal microphone that records a roar

Not every laptop digital mic goes through SOF. Many still hang a DMIC off the
HDA codec itself -- on Realtek parts it is pin 0x12, which
`/proc/asound/card0/codec#0` shows as `[Fixed] Mic at Oth Mobile-In`,
`Conn = Digital` -- and the generic parser gives that pin an
`Internal Mic Boost` of up to +30 dB. PipeWire folds the boost into the source
volume, so an input at 100% is the boost at maximum on top of the capture
volume at maximum: the DMIC's samples, already full scale, pin there, and a
recording of a quiet room is a roar. The telltale is a raw capture that is
mostly ±32767 with the two channels in opposite polarity.

Two things keep that from happening:

* `configs/wireplumber/wireplumber.conf.d/50-raven-input-volume.conf`, staged
  to `/etc/wireplumber`, starts every input WirePlumber has not seen before
  at 40% rather than 100%. That is below the first boost step, and measured
  clean on the machine that found this.
* `configs/kernel/patches/0002-*` removes the boost outright on the machine
  that found it (HP, PCI SSID `103c:883e`), so no slider position brings the
  roar back. Another model with the same symptom wants its SSID
  (`/sys/bus/pci/devices/0000:00:0e.0/subsystem_{vendor,device}`) added to
  the same quirk -- `ALC236_FIXUP_HP_NO_INT_MIC_BOOST`, or upstream's
  `LIMIT_INT_MIC_BOOST` fixups where one +10 dB step is tolerable.

An input someone already set to 100% keeps what they chose; `wpctl set-volume
@DEFAULT_AUDIO_SOURCE@ 0.4` puts it back.

## Input and peripherals

Game controllers: `INPUT_JOYDEV`, `JOYSTICK_XPAD` (Xbox, wired and wireless
dongle), `HID_PLAYSTATION`, `HID_NINTENDO`, `HID_STEAM`, `HID_GOOGLE_STADIA_FF`,
all with force feedback. `INPUT_UINPUT=y` for virtual input devices. The HID
sensor hub with accelerometer and ambient-light drivers over IIO, for
auto-rotate and auto-brightness on convertibles. Card readers over USB
(`MISC_RTSX_USB`, `MMC_REALTEK_USB`) alongside the PCI ones. Webcams via
`MEDIA_SUPPORT` with UVC and the Intel IPU6 bridge for MIPI cameras.

Peripherals that the generic HID driver only half drives have their own:
`HID_ELAN` and `HID_ALPS` for I2C touchpads, `HID_UCLOGIC`, `HID_KYE`,
`HID_WALTOP` and `HID_VIEWSONIC` for drawing tablets, `HID_CORSAIR`,
`HID_RAZER` and `HID_HOLTEK` for gaming mice and keyboards, `HID_MCP2221` for
the USB-to-I2C bridge on a lot of dev hardware. Generic HID gets a mouse
moving; these are what make the extra buttons, the pressure axis and the
per-key LEDs work. `I2C_AMD_MP2=y` is the controller AMD laptop touchpads hang
off, which is a blank touchpad rather than a missing feature when it is absent.

USB audio interfaces beyond the class driver -- Native Instruments, Line 6,
Tascam, M-Audio -- are the `SND_USB_*` modules. Anything class-compliant needed
nothing new.

Temperature and fans: `SENSORS_CORETEMP` and `SENSORS_K10TEMP` are the CPU
package only, which is not enough for a peripherals view that claims to show
what the machine is doing. The board is a Super I/O chip or vendor WMI:
`SENSORS_NCT6775`, `SENSORS_NCT6683`, `SENSORS_IT87`, `SENSORS_DELL_SMM`,
`SENSORS_ASUS_WMI`, `SENSORS_ASUS_EC`, all modules.

## Platform controllers

Some machines put their keyboard, battery and thermal controls behind a vendor
controller rather than on the buses above. Without its driver the laptop does
not look like it is missing a feature; it looks broken.

| Machine | Behind | Option |
|---|---|---|
| Surface Laptop, Surface Pro | Surface Aggregator Module, on a UART | `SURFACE_AGGREGATOR=y` and the `SURFACE_*` set, plus `SERIAL_8250_DW=y` |
| Framework 13 / 16 | a ChromeOS EC | `CHROME_PLATFORMS=y CROS_EC=y CROS_EC_LPC=y` |
| Chromebooks | the same EC | as above, plus `CROS_EC_SPI`, `CROS_EC_ISHTP` |
| Ryzen laptops | AMD PMF | `AMD_PMF=m` (needs `AMD_PMC` and `ACPI_PLATFORM_PROFILE`, both already on) |
| Intel laptops | hotkeys over ACPI | `INTEL_HID_EVENT=m`, `INTEL_VBTN=m` |
| Dell, ThinkPad, Asus, HP, MSI, Acer | vendor WMI | `DELL_LAPTOP`, `THINKPAD_ACPI`, `ASUS_WMI`, `HP_WMI`, `MSI_WMI`, `ACER_WMI`, all modules |

Surface is the one where the difference is total: the keyboard, the touchpad,
the battery, the thermal profile and the detach button are all on the
aggregator, so a Surface booting a kernel without it reaches a desktop with no
input devices at all. Framework lands in the ChromeOS row rather than one of
its own because that is genuinely what it ships -- charge thresholds, fan
control and the privacy switches all come through `cros_ec_lpc`.

## Removable storage

The kernel side was never the problem: `USB_STORAGE`, `USB_UAS`, the card
readers, and `vfat`, `exfat`, `ntfs3`, `udf` and `iso9660` have all been built
in. What was missing was anything in userspace to mount the result, so plugging
in a USB drive produced a `/dev/sdb1` and a file manager showing nothing.

`raven-mount` (`init/src/mount.rs`, built with raven-init) is that piece.
`init.toml` runs `raven-mount watch --automount` as the `mount` service; it
watches the same uevent socket `raven-ports` does and mounts what it
recognises under `/media/<label>`.

```
raven-mount                     # every removable volume and where it is
raven-mount mount /dev/sdb1     # by hand
raven-mount unmount /media/TRAVEL
raven-mount eject /dev/sdb1     # unmount, then spin the device down
```

The decisions it owns, which are the whole of what it is:

* **Eligible** means removable -- the disk says so, or it is reached over USB,
  MMC or FireWire, because a disk in a USB enclosure usually reports that it is
  fixed -- and not on a disk the running system is using. That second test
  follows loop devices back to their backing file, which is what stops a live
  USB from mounting and then unmounting the stick it booted from.
* **Where** is `/media/<label>`, with the label sanitised to one safe path
  component, falling back to the filesystem type and a UUID fragment. A
  collision gets a `-2`.
* **How** is always `nosuid,nodev,noatime`. Filesystems that store no ownership
  -- vfat, exfat, ntfs3, hfs+ -- are mounted `uid=`/`gid=` of whoever the
  graphical session runs as, resolved by the same rule `overrides.rs` uses
  (`raven.user=` on the command line, else the lowest-uid regular account), so
  the two cannot disagree. ext4, btrfs, xfs and f2fs keep their own ownership.
  A filesystem that rejects an option or is unclean gives ground one step at a
  time down to a read-only mount, rather than failing outright.
* **Encrypted volumes are listed, not opened.** There is no passphrase prompt
  here. `crypto_LUKS`, `LVM2_member` and swap are reported and left alone.
* **A drive pulled without being unmounted** is detached rather than unmounted,
  because a clean unmount of a device that no longer exists can block forever.

It is not udisks2. udisks2 answers to polkit, and `etc/raven/init.toml` records
why polkitd is not here -- nothing consults it. RavenFileManager needs nothing
new either way: its sidebar is GIO's `VolumeMonitor`, whose unix backend reads
`/proc/mounts`, so a volume mounted here appears in Devices on its own.

Filesystems on media formatted elsewhere: `F2FS_FS=y` for anything an Android
phone formatted, `HFSPLUS_FS=m` and `HFS_FS=m` for a drive that came off a Mac.

## Printing and scanning

Driverless, deliberately. IPP Everywhere and AirPrint cover every printer sold
in roughly the last decade with no vendor PPD, and eSCL does the same for
scanners. The alternative -- ghostscript, gutenprint and foomatic-db -- is most
of a gigabyte to support hardware older than that, and is one `rvn install`
away for whoever has it.

| Piece | Package | Service |
|---|---|---|
| The scheduler | `cups` | `cupsd`, `/usr/bin/cupsd -f` |
| The filter chain | `cups-filters`, `libcupsfilters`, `libppd` | -- |
| A USB printer, made to look like a network one | `ipp-usb` | `ipp-usb standalone` |
| Finding a printer or scanner on the network | `avahi`, `nss-mdns` | `avahi-daemon` |
| Scanning | `sane`, `sane-airscan` | -- |

`ipp-usb` is the piece that does the work for a printer on a cable: a modern
printer speaks IPP over USB exactly as it does over the network, but nothing in
CUPS reaches that interface directly. ipp-usb binds it and re-exports it on
localhost, so the printer arrives as an ordinary IPP device needing no driver.
Without it the same printer falls back to the raw USB backend and wants a PPD
the image does not carry.

All of these are staged by `scripts/lib/stage-desktop-runtime.py` as
**optional**: a package whose name moves upstream costs a warning and a missing
printer dialog rather than a failed ISO at the last stage. `stage-gui.sh`
installs each service drop-in only when the binary it names actually arrived,
because `check-desktop-image.py` treats an enabled service with a missing
executable as an error -- and because a service pointing at a path that does not
exist fails silently at every boot, which is the failure mode
`scripts/lib/components.sh` exists to stop.

cupsd and avahi-daemon start as root, bind their ports, and drop to the `cups`
and `avahi` accounts. Those accounts and their state directories are in
`scripts/lib/skeleton.sh`: there is no systemd-sysusers here, so an account that
is not in the shipped `/etc/passwd` never exists, and a daemon that cannot drop
privilege refuses to start.

## Phones, cameras and Bluetooth transfer

`libmtp` and `libgphoto2` are staged, which is the device databases and the
command-line transfer tools (`mtp-detect`, `mtp-files`, `gphoto2`). Browsing a
phone in the file manager is a gvfs backend (`gvfs-mtp`, `gvfs-gphoto2`) and
gvfs is not shipped -- see "What is still missing" below.

`bluez-obex` is staged and `obexd` runs as a service, which is the receiving
half of "send this file to that device"; it drops into
`/var/spool/bluetooth`. Pairing and audio never needed it.

## Firmware updates

A Thunderbolt dock two firmware revisions behind drops its displays. An NVMe
drive ships from the factory with a known data-loss bug. A USB-C hub
renegotiates power wrongly until it is updated. None of these look like
firmware from the desktop -- they look like Raven is broken.

`raven-firmware` (`configs/raven-firmware`, installed to `/usr/bin` by stage2)
is the entry point:

```
raven-firmware              # devices with updatable firmware, and their versions
raven-firmware check        # refresh LVFS metadata, list what is available
raven-firmware update       # apply them; asks first
raven-firmware history      # what has been updated here before
raven-firmware security     # the host security (HSI) report
```

### Why fwupdtool and not fwupdmgr

This is the decision worth knowing about. `fwupdmgr` talks over D-Bus to
`fwupd`, a root daemon, which authorises every call through polkit -- and Raven
does not run polkitd, for the reason `etc/raven/init.toml` gives: nothing on
this system consults it. On a stock image that path ends in a denial, not an
update.

`fwupdtool` is the same engine with no daemon, no D-Bus and no polkit. It loads
the plugins in-process and expects to be run as root, which is the
authorisation. It is what fwupd's own documentation points at for this case. So
firmware stays current without an extra always-on root process, which is what
made adding fwupd worth it rather than a second polkit argument.

The daemon is still available for anyone who wants `fwupdmgr` or a graphical
updater: `configs/raven/services/fwupd.toml` ships `enabled = false`, and
turning it on means `rvn install polkit` and a polkitd drop-in as well.

### What it will not do

Update anything on its own. There is no timer and no service, deliberately: a
firmware write is not something to do to someone's dock while they are using
it, and an interrupted one can leave hardware that does not come back. `check`
is read-only; `update` states what it is about to do and waits for a yes.

System firmware updates -- and the Thunderbolt controllers on many laptops --
are UEFI capsules, staged by writing an EFI variable and a file to the ESP.
That needs efivarfs, which init mounts (`EARLY_FILESYSTEMS` in
`init/src/main.rs`). On a machine booted in BIOS mode it is simply absent, and
`raven-firmware` says so; devices updated over USB or Thunderbolt are
unaffected.

`fwupd` and `fwupd-efi` are staged by `stage-desktop-runtime.py` as optional,
like the printing stack. `raven-firmware` itself comes from stage2, so a
console-only image has it too -- it checks for `fwupdtool` at run time and
names the package when it is missing.

## Checking a machine

`raven-ports` (in `init/src/ports.rs`, built with raven-init) prints every
port and what is in it, section by section:

```
raven-ports                 # displays, USB-C, USB4, USB, network, audio and
                            # jacks, bluetooth, input, sensors
raven-ports displays        # one section
raven-ports watch           # devices as they are plugged and unplugged (root)
```

A section that says the class is missing -- "no Type-C class", "no
Thunderbolt bus" -- is the kernel config, not the cable. The same facts by
hand:

```
ls /sys/class/typec/                 # USB-C ports and partners
ls /sys/bus/thunderbolt/devices/     # USB4 devices
for c in /sys/class/drm/card*-*; do echo "$c: $(cat $c/status)"; done
cat /proc/asound/cards               # HDA, HDMI, USB and SOF cards
ls /sys/class/net/                   # every NIC, including dock ones
ls /sys/class/wwan/                  # mobile broadband, if fitted
```

For storage specifically, `raven-mount` answers the same kind of question --
what is plugged in, what filesystem is on it, and why it is or is not mounted:

```
raven-mount                          # every removable volume and its state
lsusb                                # the USB tree, with readable names
lpstat -p                            # printers CUPS knows about
avahi-browse -art                    # printers and scanners on the network
raven-firmware                       # devices whose firmware can be updated
```

A volume that says `system disk` is on a disk the running system is using and
will never be automounted. One that says `crypto_LUKS, not opened here` needs
`cryptsetup` by hand; there is no passphrase prompt in the daemon.

## What is still missing

Deliberately, with the reason, so nobody has to rediscover it:

* **DisplayLink DL-3xxx/5xxx/6xxx.** `evdi` is built and installed; the
  userspace half, `DisplayLinkManager`, is under a EULA that does not permit
  redistribution. Install DisplayLink's own package on a machine with such a
  dock. DL-1x5 (USB 2) adapters need nothing -- `udl` is in tree.
* **Browsing a phone or camera in the file manager.** Needs `gvfs-mtp` and
  `gvfs-gphoto2`, and gvfs is not shipped. The devices are recognised and the
  command-line tools work. Mounting one under `/media` would need a FUSE MTP
  helper, and the ones that exist are not in the repositories this image builds
  from.
* **Thunderbolt authorisation with a prompt.** No `boltd`; see the sysfs
  commands in the USB4 section. The kernel default authorises whatever the
  firmware vouches for, which is every machine whose security level is not
  `user`.
* **Printers older than IPP Everywhere.** The driverless stack is what is
  shipped; `rvn install ghostscript gutenprint foomatic-db` covers the rest and
  is why that was not made the default.
* **LUKS volumes on removable media.** Listed by `raven-mount`, not opened.
