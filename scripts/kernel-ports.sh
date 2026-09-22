#!/bin/bash
# =============================================================================
# kernel-ports.sh -- laptop port and peripheral support for the Raven kernel
# =============================================================================
#
# Applies the Kconfig options that make every port on a laptop usable: USB-C
# (PD, DisplayPort alt mode, role switching), USB4/Thunderbolt docks,
# DisplayLink adapters, USB serial/ACM, game controllers, webcams, audio
# codecs beyond Realtek, SOF audio on modern Intel/AMD laptops, more ethernet
# NICs and PHYs, and hybrid-graphics switching.
#
# Usage: scripts/kernel-ports.sh <kernel-source-dir>
#
# Edits <kernel-source-dir>/.config in place with the kernel's own
# scripts/config. Run `make olddefconfig` afterwards so dependencies resolve,
# then copy .config back over configs/kernel/config-6.17-raven.
# build-kernel.sh calls this automatically on --ports.
#
# Module vs built-in policy follows the rest of the config: anything that
# needs firmware from /lib/firmware is a module (it must load after the root
# is mounted -- see configs/raven-udev), anything on the path to a working
# console or root filesystem is built in, everything else is built in when
# small and a module when large.
# =============================================================================

set -euo pipefail

src="$(cd "${1:?usage: $0 <kernel-source-dir>}" && pwd)"
cfg="${src}/scripts/config"
[ -x "$cfg" ] || { echo "no scripts/config in $src" >&2; exit 1; }
cd "$src"

y() { for o in "$@"; do "$cfg" --enable "$o"; done; }
m() { for o in "$@"; do "$cfg" --module "$o"; done; }
n() { for o in "$@"; do "$cfg" --disable "$o"; done; }

# --- USB Type-C: connector class, port managers, alternate modes ------------
# Without TYPEC the port is a dumb USB-A behind a different plug: no PD
# negotiation, no DisplayPort over the port, no dock video.
y TYPEC TYPEC_UCSI UCSI_ACPI UCSI_CCG UCSI_STM32G0
y TYPEC_TCPM TYPEC_TCPCI TYPEC_FUSB302 TYPEC_TPS6598X TYPEC_ANX7411
y TYPEC_RT1719 TYPEC_HD3SS3220 TYPEC_STUSB160X TYPEC_WUSB3801
y TYPEC_DP_ALTMODE TYPEC_NVIDIA_ALTMODE TYPEC_TBT_ALTMODE
y TYPEC_MUX_PI3USB30532 TYPEC_MUX_INTEL_PMC TYPEC_MUX_FSA4480 TYPEC_MUX_IT5205
y TYPEC_MUX_NB7VPQ904M TYPEC_MUX_PTN36502
y INTEL_SCU_PCI                     # INTEL_SCU_IPC, which the PMC mux needs
y USB_ROLE_SWITCH USB_ROLES_INTEL_XHCI
# Renesas uPD720201/2 is the one host controller that uploads firmware at
# probe. Built in, it probes during PCI enumeration -- before any root
# filesystem -- asks for renesas_usb_fw.mem, gets nothing, and never asks
# again, so the card is dead for the rest of the boot. The firmware is in the
# image (stage2-native.sh copies renesas/) but not the initramfs, so the
# driver has to wait for the real root like every other firmware user.
m USB_XHCI_PCI_RENESAS

# --- USB4 / Thunderbolt: docks, eGPUs, DP tunnelling -------------------------
y USB4 USB4_NET INTEL_WMI_THUNDERBOLT
y HOTPLUG_PCI_PCIE HOTPLUG_PCI_ACPI    # TB devices arrive as PCIe hotplug

# --- DisplayLink and other USB displays ------------------------------------
# udl covers DL-1x5 (USB 2) adapters in tree. DL-3xxx/5xxx/6xxx need the
# out-of-tree evdi module, packaged separately (packages/raven/evdi).
m DRM_UDL DRM_GUD DRM_GM12U320
y DRM_DISPLAY_DP_AUX_CHARDEV          # /dev/drm_dp_auxN for dock/monitor tooling

# --- Graphics: newer Intel, hybrid switching ---------------------------------
m DRM_XE                              # Lunar Lake / Battlemage and later
y DRM_XE_DISPLAY
y VGA_SWITCHEROO                      # power down the idle dGPU on hybrid laptops

# --- Audio: every codec family, SOF for post-2019 laptops ---------------------
y SND_HDA_CODEC_CONEXANT SND_HDA_CODEC_CIRRUS SND_HDA_CODEC_ANALOG
y SND_HDA_CODEC_SIGMATEL SND_HDA_CODEC_VIA SND_HDA_CODEC_CA0132 SND_HDA_CODEC_CMEDIA
y SND_HDA_CODEC_CS8409 SND_HDA_CODEC_SI3054
y SND_HDA_RECONFIG SND_HDA_PATCH_LOADER SND_HDA_INPUT_BEEP
y SPI SPI_MASTER SPI_MEM SPI_PXA2XX SPI_AMD    # LPSS/AMD SPI: the bus the amps below sit on
m SND_HDA_SCODEC_CS35L41_I2C SND_HDA_SCODEC_CS35L41_SPI   # amps on many 2022+ laptops
m SND_HDA_SCODEC_CS35L56_I2C SND_HDA_SCODEC_CS35L56_SPI
m SND_HDA_SCODEC_TAS2781_I2C
y SND_USB_AUDIO_MIDI_V2
m SND_ALOOP
# ASoC/SOF: modules -- every SOF platform loads firmware and a topology.
m SND_SOC
y SND_SOC_SOF_TOPLEVEL                # bool: the menu gate, not a module
m SND_SOC_SOF_PCI SND_SOC_SOF_ACPI
y SND_SOC_SOF_INTEL_TOPLEVEL SND_SOC_SOF_HDA_LINK SND_SOC_SOF_HDA_AUDIO_CODEC
m SND_SOC_SOF_APOLLOLAKE SND_SOC_SOF_GEMINILAKE SND_SOC_SOF_CANNONLAKE
m SND_SOC_SOF_COFFEELAKE SND_SOC_SOF_ICELAKE SND_SOC_SOF_COMETLAKE
m SND_SOC_SOF_TIGERLAKE SND_SOC_SOF_ALDERLAKE SND_SOC_SOF_METEORLAKE
m SND_SOC_SOF_LUNARLAKE SND_SOC_SOF_PANTHERLAKE SND_SOC_SOF_ELKHARTLAKE
m SND_SOC_SOF_JASPERLAKE
m SND_SOC_SOF_INTEL_SOUNDWIRE
m SND_SOC_SOF_AMD_TOPLEVEL SND_SOC_SOF_AMD_RENOIR SND_SOC_SOF_AMD_REMBRANDT
m SND_SOC_SOF_AMD_VANGOGH SND_SOC_SOF_AMD_ACP63 SND_SOC_SOF_AMD_ACP70
m SND_SOC_AMD_ACP_COMMON SND_SOC_AMD_ACP_PCI
m SND_SOC_AMD_RENOIR
# scripts/config upper-cases symbol names, so the three mixed-case ACP
# symbols are set by hand. ACP6x is the DMIC path on Ryzen 6000+ laptops.
for o in SND_SOC_AMD_ACP3x SND_SOC_AMD_ACP5x SND_SOC_AMD_ACP6x; do
    sed -i "/^# CONFIG_${o} is not set\$/d; /^CONFIG_${o}=/d" .config
    echo "CONFIG_${o}=m" >> .config
done
m SND_SOC_AMD_YC_MACH SND_SOC_AMD_ACP63_TOPLEVEL SND_AMD_ASOC_ACP70
# SST_TOPLEVEL and INTEL_MACH are bool menu gates, not drivers; asking for
# =m on either is silently corrected to =y by olddefconfig.
y SND_SOC_INTEL_SST_TOPLEVEL SND_SOC_INTEL_MACH
m SND_SOC_INTEL_SOUNDWIRE_SOF_MACH
m SND_SOC_INTEL_SOF_RT5682_MACH SND_SOC_INTEL_SOF_DA7219_MACH SND_SOC_INTEL_SOF_NAU8825_MACH
m SND_SOC_INTEL_SOF_CS42L42_MACH SND_SOC_INTEL_SOF_ES8336_MACH SND_SOC_INTEL_SOF_SSP_AMP_MACH
m SND_SOC_INTEL_SKL_HDA_DSP_GENERIC_MACH SND_SOC_INTEL_AVS
y SND_SOC_INTEL_USER_FRIENDLY_LONG_NAMES
m SND_SOC_DMIC
m SOUNDWIRE SOUNDWIRE_INTEL SOUNDWIRE_AMD

# --- Webcams -----------------------------------------------------------------
y MEDIA_SUPPORT MEDIA_CAMERA_SUPPORT MEDIA_USB_SUPPORT MEDIA_PCI_SUPPORT VIDEO_DEV
m USB_VIDEO_CLASS USB_GSPCA
y USB_VIDEO_CLASS_INPUT_EVDEV
m VIDEO_INTEL_IPU6 IPU_BRIDGE VIDEO_IPU3_CIO2          # MIPI webcams on 2021+ Intel laptops

# --- Serial-over-USB and misc USB device classes ------------------------------
y USB_ACM USB_SERIAL USB_SERIAL_GENERIC
y USB_SERIAL_FTDI_SIO USB_SERIAL_CP210X USB_SERIAL_PL2303 USB_SERIAL_CH341
y USB_SERIAL_OPTION USB_SERIAL_QUALCOMM USB_SERIAL_SIERRAWIRELESS USB_SERIAL_WWAN
m MISC_RTSX_USB MMC_REALTEK_USB       # USB card readers in docks

# --- Input: controllers, virtual devices, sensors ------------------------------
y INPUT_JOYDEV INPUT_UINPUT
y HID_PLAYSTATION HID_NINTENDO HID_STEAM HID_GOOGLE_STADIA_FF
y PLAYSTATION_FF NINTENDO_FF
y JOYSTICK_XPAD JOYSTICK_XPAD_FF JOYSTICK_XPAD_LEDS INPUT_JOYSTICK
y HID_SENSOR_HUB HID_SENSOR_ACCEL_3D HID_SENSOR_ALS HID_SENSOR_IIO_COMMON HID_SENSOR_IIO_TRIGGER
y IIO

# --- Ethernet: NICs and PHYs missing from the vendor sweep --------------------
y NET_VENDOR_AQUANTIA AQTION          # Aquantia 2.5/5/10G, common in TB docks
y NET_VENDOR_ATHEROS ALX ATL1 ATL1C ATL1E   # Killer / Atheros on Asus and MSI
y MARVELL_PHY BROADCOM_PHY MICREL_PHY AQUANTIA_PHY

# --- Bluetooth: keep the USB radio awake ---------------------------------------
# btusb's runtime suspend parks the radio two seconds after the last URB and
# wakes it on the next one. Realtek and Intel parts lose ACL fragments across
# that resume -- L2CAP logs "Unexpected start frame" and an A2DP headset
# drops -- and the saving is a few milliwatts on a part that sleeps on its
# own between packets. Off by default; btusb.enable_autosuspend=1 on the
# command line turns it back on for a machine that wants it.
n BT_HCIBTUSB_AUTOSUSPEND

# --- Bluetooth: radio drivers are modules ------------------------------------
# Every Bluetooth radio worth having uploads firmware at probe: Intel wants
# intel/ibt-*.sfi, Realtek rtl_bt/, Broadcom brcm/*.hcd, MediaTek mediatek/.
# Built in, btusb probes during USB enumeration inside the initramfs, where
# none of that is, and the vendor setup gives up for good. hci0 exists in
# sysfs but never registers with mgmt, so bluetoothd sees no controller and
# the settings panel reports "no Bluetooth adapter" on a laptop that has one
# (an AX211, 8087:0033, was the one that showed it). As modules they load
# from the mounted root like iwlwifi does.
#
# The vendor helpers have to go too: a =y driver that selects BT_INTEL (or
# BCM/RTL/QCA/MTK) forces the helper back to =y, so every selector is listed.
# The BT core stays built in; it asks for no firmware.
m BT_HCIBTUSB BT_HCIUART BT_INTEL_PCIE BT_MTKUART BT_NXPUART
m BT_HCIBCM203X BT_HCIBFUSB
m BT_INTEL BT_BCM BT_RTL BT_QCA BT_MTK

# --- Mobile broadband: the WWAN card in a business laptop --------------------
# Without the WWAN class the modem enumerates and stops there: no /dev/wwan*,
# no control port, nothing for a userspace dialler to talk to. The USB side
# of this was already on (USB_SERIAL_WWAN, CDC_MBIM) but the PCIe side --
# every Qualcomm SDX card Dell, Lenovo and HP fit -- arrives over MHI, which
# needs the class and the generic MHI PCI driver.
y WWAN
m MHI_WWAN_CTRL MHI_WWAN_MBIM MHI_NET MHI_BUS_PCI_GENERIC
m USB_NET_QMI_WWAN

# --- USB dual role: the device side of a USB-C port --------------------------
# USB_ROLE_SWITCH above only flips the port. Being the device on the other end
# of the cable -- tethering out, g_ether to a second machine, a serial console
# over USB -- needs a gadget stack and a UDC, and on a laptop the UDC is dwc3.
y USB_GADGET USB_CONFIGFS
y USB_CONFIGFS_NCM USB_CONFIGFS_ECM USB_CONFIGFS_RNDIS USB_CONFIGFS_EEM
y USB_CONFIGFS_MASS_STORAGE USB_CONFIGFS_SERIAL USB_CONFIGFS_ACM USB_CONFIGFS_F_FS
m USB_DWC3 USB_DWC3_PCI
y USB_DWC3_DUAL_ROLE

# --- Microsoft Surface -------------------------------------------------------
# Surface Laptop and Pro put the keyboard, touchpad, battery, thermal profile
# and the detach button behind the Surface Aggregator Module, a controller on
# a serial port. Without it the machine boots to a desktop with no input
# devices at all, which reads as a dead image rather than a missing driver.
y SERIAL_8250_DW                      # the UART the aggregator sits on
y SURFACE_AGGREGATOR SURFACE_AGGREGATOR_BUS SURFACE_AGGREGATOR_REGISTRY
y SURFACE_AGGREGATOR_HUB SURFACE_AGGREGATOR_CDEV SURFACE_AGGREGATOR_TABLET_SWITCH
y SURFACE_ACPI_NOTIFY SURFACE_DTX SURFACE_GPE SURFACE_HOTPLUG
y SURFACE_PRO3_BUTTON SURFACE3_WMI SURFACE_3_POWER_OPREGION
y SURFACE_HID SURFACE_KBD

# --- ChromeOS EC: Chromebooks, and every Framework laptop --------------------
# Framework ships a ChromeOS EC, which is why it lands in this section rather
# than one of its own: charge thresholds, fan control, the ambient light
# sensor and the privacy switches all come through cros_ec_lpc.
y CHROME_PLATFORMS
y CROS_EC CROS_EC_LPC CROS_EC_I2C CROS_EC_SPI CROS_EC_ISHTP
y CROS_EC_CHARDEV CROS_EC_SYSFS CROS_EC_DEBUGFS CROS_EC_SENSORHUB
y CROS_KBD_LED_BACKLIGHT CROS_USBPD_LOGGER
m SENSORS_CROS_EC

# --- AMD platform: thermal and power profile on Ryzen laptops ----------------
m AMD_PMF                             # needs AMD_PMC and ACPI_PLATFORM_PROFILE, both on
y I2C_AMD_MP2                         # the I2C controller AMD laptop touchpads hang off

# --- HID: peripherals the generic driver only half drives --------------------
# Generic HID gets a mouse moving; these are what make the extra buttons,
# the tablet pressure axis and the per-key LEDs work.
y HID_ELAN HID_ALPS HID_UCLOGIC HID_KYE HID_WALTOP HID_VIEWSONIC
y HID_CORSAIR HID_RAZER HID_HOLTEK HID_ELECOM HID_CMEDIA HID_LED HID_MCP2221
m TOUCHSCREEN_USB_COMPOSITE           # touchscreens in monitors and KVMs

# --- USB audio interfaces the class driver does not cover --------------------
m SND_USB_UA101 SND_USB_USX2Y SND_USB_CAIAQ SND_USB_US122L
m SND_USB_6FIRE SND_USB_HIFACE
m SND_USB_POD SND_USB_PODHD SND_USB_TONEPORT SND_USB_VARIAX   # the Line 6 family
y SND_USB_CAIAQ_INPUT

# --- Removable-media filesystems ---------------------------------------------
# A drive formatted somewhere else is the common case for external storage.
# vfat, exfat, ntfs3, udf and iso9660 were already in; these are the two that
# were not, and both turn up on a desk: F2FS on anything an Android phone
# formatted, HFS+ on a drive that came off a Mac.
y F2FS_FS F2FS_FS_XATTR F2FS_FS_POSIX_ACL
m HFSPLUS_FS HFS_FS

# --- Thermal readouts, for the peripherals utility ---------------------------
# CORETEMP and K10TEMP are the CPU package only. A peripherals view that
# claims to show what the machine is doing needs the board too: fans, chassis
# temperatures and voltages live on a Super I/O chip or behind vendor WMI.
y SENSORS_CORETEMP SENSORS_K10TEMP
m SENSORS_NCT6775 SENSORS_NCT6683 SENSORS_IT87
m SENSORS_DELL_SMM SENSORS_ASUS_WMI SENSORS_ASUS_EC
