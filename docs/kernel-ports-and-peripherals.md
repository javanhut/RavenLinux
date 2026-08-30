# Kernel Ports and Peripherals

What the Raven kernel does with every physical port on a laptop, where that
support comes from, and what is deliberately left to userspace.

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

## USB

USB 1.1 through 3.2 need only the host controllers, all built in: `USB_XHCI_HCD`
`USB_XHCI_PCI` `USB_EHCI_HCD` `USB_OHCI_HCD` `USB_UHCI_HCD`, plus
`USB_XHCI_PCI_RENESAS` for the uPD720201/2 chips on add-in cards (firmware
under `renesas/`). Device classes built in: storage and UAS, HID, printers,
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
(authorised when the firmware says so), with no userspace `boltd`.

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

## Input and peripherals

Game controllers: `INPUT_JOYDEV`, `JOYSTICK_XPAD` (Xbox, wired and wireless
dongle), `HID_PLAYSTATION`, `HID_NINTENDO`, `HID_STEAM`, `HID_GOOGLE_STADIA_FF`,
all with force feedback. `INPUT_UINPUT=y` for virtual input devices. The HID
sensor hub with accelerometer and ambient-light drivers over IIO, for
auto-rotate and auto-brightness on convertibles. Card readers over USB
(`MISC_RTSX_USB`, `MMC_REALTEK_USB`) alongside the PCI ones. Webcams via
`MEDIA_SUPPORT` with UVC and the Intel IPU6 bridge for MIPI cameras.

Temperature: `SENSORS_CORETEMP` and `SENSORS_K10TEMP` so a peripherals view
can show what the machine is doing.

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
```
