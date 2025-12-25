# Kernel Input Device Configuration

This document describes the kernel configuration options for input device support in Raven Linux.

## Overview

Modern laptops use various input device controllers that require specific kernel drivers. The kernel is configured to support input devices connected via:

- PS/2 (legacy)
- USB HID
- I2C HID (most modern laptops)
- Intel Sensor Hub (ISH)
- AMD Sensor Fusion Hub (SFH)
- Intel Touch Host Controller (THC)

## Required Kernel Options

### Pin Controller Subsystem

Required for I2C controllers on Intel/AMD platforms:

```
CONFIG_PINCTRL=y
CONFIG_PINCTRL_INTEL=y
CONFIG_PINCTRL_ALDERLAKE=y
CONFIG_PINCTRL_BROXTON=y
CONFIG_PINCTRL_CANNONLAKE=y
CONFIG_PINCTRL_CEDARFORK=y
CONFIG_PINCTRL_DENVERTON=y
CONFIG_PINCTRL_ELKHARTLAKE=y
CONFIG_PINCTRL_EMMITSBURG=y
CONFIG_PINCTRL_GEMINILAKE=y
CONFIG_PINCTRL_ICELAKE=y
CONFIG_PINCTRL_JASPERLAKE=y
CONFIG_PINCTRL_LAKEFIELD=y
CONFIG_PINCTRL_LEWISBURG=y
CONFIG_PINCTRL_METEORLAKE=y
CONFIG_PINCTRL_SUNRISEPOINT=y
CONFIG_PINCTRL_TIGERLAKE=y
CONFIG_PINCTRL_AMD=y
```

### GPIO Support

```
CONFIG_GPIOLIB=y
CONFIG_GPIO_ACPI=y
CONFIG_GPIO_CDEV=y
```

### I2C Controller Drivers

Intel DesignWare I2C controllers (used on most Intel/AMD laptops):

```
CONFIG_I2C_DESIGNWARE_CORE=y
CONFIG_I2C_DESIGNWARE_PLATFORM=y
CONFIG_I2C_DESIGNWARE_PCI=y
```

### Intel Low Power Subsystem (LPSS)

Required for I2C on Intel platforms:

```
CONFIG_MFD_INTEL_LPSS=y
CONFIG_MFD_INTEL_LPSS_ACPI=y
CONFIG_MFD_INTEL_LPSS_PCI=y
```

### Serial Device Bus

```
CONFIG_SERIAL_DEV_BUS=y
CONFIG_SERIAL_DEV_CTRL_TTYPORT=y
```

### HID Drivers

Core HID support:

```
CONFIG_HID_MULTITOUCH=y
CONFIG_I2C_HID=y
CONFIG_I2C_HID_CORE=y
CONFIG_I2C_HID_ACPI=y
```

Platform-specific HID:

```
CONFIG_INTEL_ISH_HID=y
CONFIG_INTEL_ISH_FIRMWARE_DOWNLOADER=y
CONFIG_AMD_SFH_HID=y
CONFIG_INTEL_THC_HID=y
```

### Mouse/Touchpad Drivers

PS/2 touchpads:

```
CONFIG_MOUSE_PS2=y
CONFIG_MOUSE_PS2_ALPS=y
CONFIG_MOUSE_PS2_SYNAPTICS=y
CONFIG_MOUSE_PS2_SYNAPTICS_SMBUS=y
CONFIG_MOUSE_PS2_CYPRESS=y
CONFIG_MOUSE_PS2_ELANTECH=y
CONFIG_MOUSE_PS2_FOCALTECH=y
CONFIG_MOUSE_PS2_SMBUS=y
```

I2C touchpads:

```
CONFIG_MOUSE_ELAN_I2C=y
CONFIG_MOUSE_ELAN_I2C_I2C=y
CONFIG_MOUSE_ELAN_I2C_SMBUS=y
CONFIG_MOUSE_SYNAPTICS_I2C=y
```

## Troubleshooting

If input devices are not working on hardware:

1. Check if the device is detected:
   ```
   cat /proc/bus/input/devices
   ```

2. Check kernel messages for I2C/HID errors:
   ```
   dmesg | grep -i "i2c\|hid\|input\|touch"
   ```

3. Verify modules are loaded:
   ```
   lsmod | grep -E "hid|i2c|input"
   ```

4. Check libinput device list:
   ```
   libinput list-devices
   ```

## Notes

- Input devices may work in QEMU but not on hardware because QEMU emulates standard PS/2/USB HID devices, while real hardware often uses I2C-connected touchpads requiring specific drivers.
- Most modern laptops (2015+) use I2C HID for touchpads.
- Intel platforms require both pinctrl and LPSS drivers for I2C to function.
