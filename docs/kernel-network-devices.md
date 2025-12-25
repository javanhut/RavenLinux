# Kernel Network Device Configuration

This document describes the kernel configuration options for network device support in Raven Linux.

## Ethernet Drivers

### Intel Ethernet

```
CONFIG_E1000=y          # Intel PRO/1000 (legacy)
CONFIG_E1000E=y         # Intel PRO/1000 PCI-Express
CONFIG_IGB=y            # Intel 82575/82576/I350 Gigabit
CONFIG_IGC=y            # Intel I225/I226 2.5G Ethernet
```

### Realtek Ethernet

```
CONFIG_R8169=y          # Realtek 8169/8168/8101/8125 Gigabit
```

### Marvell Ethernet

```
CONFIG_SKY2=y           # Marvell Yukon 2 Gigabit
```

## WiFi Drivers

### Intel WiFi

```
CONFIG_IWLWIFI=y        # Intel Wireless WiFi core
CONFIG_IWLDVM=y         # Intel DVM firmware support (older cards)
CONFIG_IWLMVM=y         # Intel MVM firmware support (newer cards)
```

### Qualcomm/Atheros WiFi

```
CONFIG_ATH9K=y          # Atheros 802.11n (AR9xxx)
CONFIG_ATH10K=y         # Atheros 802.11ac (QCA98xx)
CONFIG_ATH11K=y         # Qualcomm WiFi 6 (QCA6390, WCN6855)
CONFIG_ATH12K=y         # Qualcomm WiFi 7 (WCN7850, QCA2066)
```

### Broadcom WiFi

```
CONFIG_BRCMSMAC=y       # Broadcom IEEE802.11n SoftMAC
CONFIG_BRCMFMAC=y       # Broadcom FullMAC WLAN (USB/PCI/SDIO)
```

### MediaTek WiFi

```
CONFIG_MT7601U=y        # MediaTek MT7601U (USB)
CONFIG_MT7615E=y        # MediaTek MT7615E (PCIe)
CONFIG_MT7663U=y        # MediaTek MT7663U (USB)
CONFIG_MT7921E=y        # MediaTek MT7921 WiFi 6 (PCIe)
CONFIG_MT7921U=y        # MediaTek MT7921 WiFi 6 (USB)
CONFIG_MT7925E=y        # MediaTek MT7925 WiFi 7 (PCIe)
CONFIG_MT7925U=y        # MediaTek MT7925 WiFi 7 (USB)
```

### Realtek WiFi

RTW88 (WiFi 5/6):

```
CONFIG_RTW88=y
CONFIG_RTW88_CORE=y
CONFIG_RTW88_PCI=y
CONFIG_RTW88_USB=y
CONFIG_RTW88_8822BE=y   # RTL8822BE (PCIe)
CONFIG_RTW88_8822BU=y   # RTL8822BU (USB)
CONFIG_RTW88_8822CE=y   # RTL8822CE (PCIe)
CONFIG_RTW88_8822CU=y   # RTL8822CU (USB)
CONFIG_RTW88_8821CE=y   # RTL8821CE (PCIe)
CONFIG_RTW88_8821CU=y   # RTL8821CU (USB)
CONFIG_RTW88_8821AU=y   # RTL8821AU (USB)
CONFIG_RTW88_8812AU=y   # RTL8812AU (USB)
```

RTW89 (WiFi 6E/7):

```
CONFIG_RTW89=y
CONFIG_RTW89_CORE=y
CONFIG_RTW89_PCI=y
CONFIG_RTW89_USB=y
CONFIG_RTW89_8852AE=y   # RTL8852AE (PCIe)
CONFIG_RTW89_8852BE=y   # RTL8852BE (PCIe)
CONFIG_RTW89_8852BU=y   # RTL8852BU (USB)
CONFIG_RTW89_8852CE=y   # RTL8852CE (PCIe)
CONFIG_RTW89_8851BE=y   # RTL8851BE (PCIe)
CONFIG_RTW89_8851BU=y   # RTL8851BU (USB)
CONFIG_RTW89_8922AE=y   # RTL8922AE WiFi 7 (PCIe)
```

## USB Network Adapters

### USB Ethernet

```
CONFIG_USB_NET_DRIVERS=y
CONFIG_USB_USBNET=y           # USB Networking core
CONFIG_USB_RTL8150=y          # Realtek RTL8150 USB 1.1
CONFIG_USB_RTL8152=y          # Realtek RTL8152/RTL8153 USB 2.0/3.0
CONFIG_USB_LAN78XX=y          # Microchip LAN78XX USB 3.0
CONFIG_USB_NET_AX8817X=y      # ASIX AX88xxx USB 2.0
CONFIG_USB_NET_AX88179_178A=y # ASIX AX88179/178A USB 3.0
CONFIG_USB_NET_SMSC75XX=y     # SMSC LAN75XX USB 2.0
CONFIG_USB_NET_SMSC95XX=y     # SMSC LAN95XX USB 2.0
```

### USB CDC (USB Tethering)

```
CONFIG_USB_NET_CDCETHER=y     # CDC Ethernet
CONFIG_USB_NET_CDC_NCM=y      # CDC NCM (faster)
CONFIG_USB_NET_CDC_MBIM=y     # CDC MBIM (mobile broadband)
CONFIG_USB_NET_RNDIS_HOST=y   # RNDIS (Windows Mobile)
CONFIG_USB_IPHETH=y           # iPhone USB tethering
```

## Bluetooth

### Core Bluetooth Stack

```
CONFIG_BT=y              # Bluetooth subsystem
CONFIG_BT_BREDR=y        # Bluetooth Classic (BR/EDR)
CONFIG_BT_LE=y           # Bluetooth Low Energy (BLE)
CONFIG_BT_RFCOMM=y       # RFCOMM protocol (serial)
CONFIG_BT_BNEP=y         # BNEP protocol (networking)
CONFIG_BT_HIDP=y         # HIDP protocol (HID devices)
CONFIG_BT_HS=y           # High Speed (802.11 PAL)
CONFIG_BT_LEDS=y         # LED triggers
```

### USB Bluetooth Adapters

```
CONFIG_BT_HCIBTUSB=y           # HCI USB driver (most USB dongles)
CONFIG_BT_HCIBTUSB_BCM=y       # Broadcom protocol support
CONFIG_BT_HCIBTUSB_MTK=y       # MediaTek protocol support
CONFIG_BT_HCIBTUSB_RTL=y       # Realtek protocol support
```

### UART Bluetooth (Integrated)

```
CONFIG_BT_HCIUART=y            # HCI UART driver
CONFIG_BT_HCIUART_INTEL=y      # Intel (integrated in laptops)
CONFIG_BT_HCIUART_BCM=y        # Broadcom
CONFIG_BT_HCIUART_RTL=y        # Realtek
CONFIG_BT_HCIUART_QCA=y        # Qualcomm Atheros
CONFIG_BT_HCIUART_ATH3K=y      # Atheros AR3K
CONFIG_BT_HCIUART_MRVL=y       # Marvell
```

### Platform-Specific Bluetooth

```
CONFIG_BT_INTEL_PCIE=y         # Intel PCIe Bluetooth (newer laptops)
CONFIG_BT_MTKSDIO=y            # MediaTek SDIO Bluetooth
CONFIG_BT_MTKUART=y            # MediaTek UART Bluetooth
CONFIG_BT_HCIBCM203X=y         # Broadcom BCM203x USB
CONFIG_BT_NXPUART=y            # NXP Bluetooth
```

## Troubleshooting

### Bluetooth not working

1. Check if adapter is detected:
   ```
   lsusb | grep -i bluetooth
   hciconfig -a
   ```

2. Check kernel messages:
   ```
   dmesg | grep -i bluetooth
   dmesg | grep -i hci
   ```

3. Check if firmware is loaded:
   ```
   dmesg | grep -i "firmware.*bluetooth"
   ```

4. Verify Bluetooth service:
   ```
   bluetoothctl show
   ```

### WiFi not working

1. Check if the device is detected:
   ```
   lspci | grep -i network
   lsusb | grep -i wireless
   ```

2. Check kernel messages:
   ```
   dmesg | grep -i "wifi\|wlan\|802.11"
   ```

3. Check if firmware is loaded:
   ```
   dmesg | grep -i firmware
   ```

4. Verify interface exists:
   ```
   ip link show
   iw dev
   ```

### Ethernet not working

1. Check if the device is detected:
   ```
   lspci | grep -i ethernet
   ```

2. Check kernel messages:
   ```
   dmesg | grep -i "eth\|enp\|eno"
   ```

3. Check link status:
   ```
   ip link show
   ethtool <interface>
   ```

## Firmware Requirements

Many WiFi and Bluetooth adapters require firmware blobs. Ensure the following firmware packages are available:

### WiFi Firmware
- Intel: `linux-firmware` (iwlwifi-*.ucode)
- Qualcomm: `linux-firmware` (ath10k/*, ath11k/*, ath12k/*)
- Realtek: `linux-firmware` (rtw88/*, rtw89/*)
- MediaTek: `linux-firmware` (mediatek/*)

### Bluetooth Firmware
- Intel: `linux-firmware` (intel/ibt-*.sfi, intel/ibt-*.ddc)
- Realtek: `linux-firmware` (rtl_bt/*)
- Qualcomm: `linux-firmware` (qca/*)
- MediaTek: `linux-firmware` (mediatek/BT*)
- Broadcom: `linux-firmware` (brcm/*.hcd)

Firmware files should be placed in `/lib/firmware/`.
