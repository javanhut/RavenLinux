#!/usr/bin/env python3
"""Stage package-owned desktop runtime/data and their ELF library closure.

This runs on the Arch build container, never on the machine being imaged.
Systemd units are not activated: Raven's service definitions own startup.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

# Audio and Bluetooth. A desktop image without these is broken in a way worth
# failing the build over, so a missing one here is an error.
REQUIRED = (
    'pipewire', 'libpipewire', 'pipewire-audio', 'pipewire-pulse',
    'wireplumber', 'libwireplumber', 'bluez', 'bluez-utils', 'alsa-lib',
    'alsa-card-profiles', 'alsa-ucm-conf', 'alsa-topology-conf',
)

# Everything a peripheral needs above the kernel driver. These are separated
# from REQUIRED for two reasons: an image built without them is diminished
# rather than broken, and the names drift -- Arch has split and renamed the
# printing stack more than once -- so a rename upstream should cost a warning
# and a missing printer dialog, not a failed ISO build at the last stage.
#
# Driverless is the whole strategy here. IPP Everywhere and AirPrint cover
# every printer sold in about a decade without a vendor PPD, eSCL does the
# same for scanners, and ipp-usb is what makes a printer on a USB cable look
# like one of those rather than something needing a driver. The alternative --
# ghostscript, gutenprint and foomatic -- is most of a gigabyte to support
# hardware from before that, and is one `rvn install` away for whoever has it.
OPTIONAL = (
    # Printing, and the mDNS that finds a printer on the network.
    'cups', 'cups-filters', 'libcupsfilters', 'libppd', 'ipp-usb',
    'avahi', 'nss-mdns',
    # Scanning, the same way: eSCL and WSD over the network or USB.
    'sane', 'sane-airscan',
    # Phones and cameras. These are the device databases and the transfer
    # tools; browsing a phone in the file manager needs a gvfs backend, which
    # is not shipped -- see docs/kernel-ports-and-peripherals.md.
    'libmtp', 'libgphoto2',
    # Bluetooth file transfer -- the receiving half of "send to device".
    'bluez-obex',
    # `lsusb`, and the USB id database that makes its output readable.
    'usbutils', 'hwdata',
    # Firmware updates for the machine and what is plugged into it. Driven by
    # /usr/bin/raven-firmware through fwupdtool, which needs no daemon and no
    # polkit -- see configs/raven-firmware. `fwupd` carries the plugins, the
    # device quirks and the LVFS remote definitions, none of which can be
    # reconstructed from the binary; `fwupd-efi` is the capsule loader that
    # system-firmware updates stage onto the ESP.
    'fwupd', 'fwupd-efi',
)


def installed(names):
    """The subset of `names` this container actually has, one query per name.

    Queried individually because `pacman -Q a b c` fails the whole call on the
    first name it does not know, which would make one renamed optional package
    take the printing stack, the scanner stack and lsusb down with it.
    """
    present = []
    for name in names:
        if subprocess.run(['pacman', '-Q', name], capture_output=True).returncode == 0:
            present.append(name)
    return present


def stage(root):
    root = Path(root).absolute()
    files = set()

    have = installed(REQUIRED)
    missing = [n for n in REQUIRED if n not in have]
    if missing:
        raise RuntimeError(
            'build container is missing required desktop packages: '
            + ' '.join(missing)
            + '\nInstall them in the Dockerfile and rebuild the container.'
        )
    optional = installed(OPTIONAL)
    for name in OPTIONAL:
        if name not in optional:
            print(f'  not in this container, skipping: {name}')
    packages = list(REQUIRED) + optional

    versions = subprocess.check_output(['pacman', '-Q', *packages], text=True)
    for line in subprocess.check_output(['pacman', '-Qlq', *packages], text=True).splitlines():
        if not line.endswith('/') and Path(line).is_file():
            files.add(line)
    # The firmware stage already handles SOF. What these packages carry beyond
    # their binaries is the part that cannot be reconstructed: audio profiles,
    # SPA codecs, WirePlumber policy, D-Bus policy, the CUPS filter chain and
    # its backends, SANE's device tables, and the USB id database. Directory
    # entries are skipped here, so anything that needs an *empty* directory to
    # exist has it created by scripts/lib/skeleton.sh instead.
    pending = list(files)
    seen = set()
    while pending:
        name = pending.pop()
        src = Path(name)
        if name in seen:
            continue
        seen.add(name)
        dest = root / name.lstrip('/')
        dest.parent.mkdir(parents=True, exist_ok=True)
        # Dereference package file symlinks: a host absolute link must never
        # point outside the target tree during staging or at boot.
        if dest.is_symlink():
            dest.unlink()
        shutil.copy2(src, dest)
        with src.open('rb') as stream:
            elf = stream.read(4) == b'\x7fELF'
        if not elf:
            continue
        result = subprocess.run(['ldd', str(src)], capture_output=True, text=True)
        if '=> not found' in result.stdout:
            raise RuntimeError(f'{src}: unresolved build-host libraries:\n{result.stdout}')
        for lib in re.findall(r'(?:=>\s+|^\s*)(/[^\s]+)', result.stdout, re.M):
            if Path(lib).is_file():
                # The sysroot owns loader paths; dependencies belong in /usr/lib.
                if Path(lib).name.startswith(('ld-linux-', 'ld-musl-')):
                    continue
                target = '/usr/lib/' + Path(lib).name
                if lib != target:
                    dst = root / target.lstrip('/')
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    if dst.is_symlink():
                        dst.unlink()
                    shutil.copy2(lib, dst)
                pending.append(lib)
    record = root / 'usr/share/raven/build/desktop-packages.txt'
    record.parent.mkdir(parents=True, exist_ok=True)
    record.write_text(versions)
    # polkit is not staged (see the polkitd note in etc/raven/init.toml), so
    # neither is its account nor its rules directories.
    print(f'Staged desktop runtime: {len(seen)} files and shared libraries')


if __name__ == '__main__':
    stage(sys.argv[1])
