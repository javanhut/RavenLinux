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

PACKAGES = (
    'pipewire', 'libpipewire', 'pipewire-audio', 'pipewire-pulse',
    'wireplumber', 'libwireplumber', 'bluez', 'bluez-utils', 'alsa-lib',
    'alsa-card-profiles', 'alsa-ucm-conf', 'alsa-topology-conf', 'polkit',
)


def stage(root):
    root = Path(root).absolute()
    files = set()
    versions = subprocess.check_output(['pacman', '-Q', *PACKAGES], text=True)
    for line in subprocess.check_output(['pacman', '-Qlq', *PACKAGES], text=True).splitlines():
        if not line.endswith('/') and Path(line).is_file():
            files.add(line)
    # The firmware stage already handles SOF; these packages carry audio
    # profiles, SPA codecs, WirePlumber policy, D-Bus policy, and polkit rules.
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
    # polkit drops privilege to this account. Do not import the builder's users.
    passwd = root / 'etc/passwd'
    group = root / 'etc/group'
    rows = [line.split(':') for line in passwd.read_text().splitlines()]
    groups = [line.split(':') for line in group.read_text().splitlines()]
    existing = next((r for r in rows if r[0] == 'polkitd'), None)
    if existing:
        uid, gid = int(existing[2]), int(existing[3])
    else:
        used = {int(r[2]) for r in rows + groups}
        uid = next(i for i in range(900, 970) if i not in used)
        gid = next((int(r[2]) for r in groups if r[0] == 'polkitd'), uid)
        with passwd.open('a') as stream:
            stream.write(f'polkitd:x:{uid}:{gid}:PolicyKit:/:/usr/bin/nologin\n')
        if not any(r[0] == 'polkitd' for r in groups):
            with group.open('a') as stream:
                stream.write(f'polkitd:x:{gid}:\n')
        with (root / 'etc/shadow').open('a') as stream:
            stream.write('polkitd:!:19000:0:99999:7:::\n')
    for directory in ['etc/polkit-1/rules.d', 'usr/share/polkit-1/rules.d']:
        path = root / directory
        path.mkdir(parents=True, exist_ok=True)
        os.chmod(path, 0o750)
        os.chown(path, uid, gid)
    print(f'Staged desktop runtime: {len(seen)} files and shared libraries')


if __name__ == '__main__':
    stage(sys.argv[1])
