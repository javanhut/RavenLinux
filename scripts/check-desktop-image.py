#!/usr/bin/env python3
"""Check the staged desktop without executing any target binaries; emit provenance."""
import argparse
from collections import deque
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tomllib

RUNTIME = ['pipewire', 'pipewire-pulse', 'wireplumber', 'wpctl', 'bluetoothctl', 'Xwayland', 'seatd']
DATA = [
    'boot/vmlinuz', 'boot/initramfs.img',
    'usr/share/pipewire/pipewire.conf', 'usr/share/pipewire/pipewire-pulse.conf',
    'usr/share/wireplumber/wireplumber.conf', 'usr/share/alsa/ucm2/ucm.conf',
    'etc/raven/init.toml', 'etc/raven/init.d/bluetoothd.toml',
    'etc/raven/session.d/50-roostbar', 'usr/share/wayland-sessions/huginn.desktop',
    'etc/skel/.config/raven/desktop.toml', 'etc/skel/.config/raven/config.toml',
    'etc/skel/.config/roostbar/config.toml',
    'usr/share/fonts/JetBrainsMonoNerdFontMono-Regular.ttf',
    'usr/share/wallpaper/set/wallpaper.jpg', 'usr/share/glib-2.0/schemas/gschemas.compiled',
    'usr/share/mime/mime.cache',
]


def target_path(root, name):
    """Resolve absolute symlinks inside the image, never against the build host."""
    pending = deque(PurePosixPath('/' + str(name).lstrip('/')).parts[1:])
    parts = []
    links = 0
    while pending:
        part = pending.popleft()
        if part in ('', '.'):
            continue
        if part == '..':
            if parts:
                parts.pop()
            continue
        candidate = root.joinpath(*parts, part)
        if candidate.is_symlink():
            links += 1
            if links > 40:
                raise ValueError(f'Symlink loop in {name}')
            link = PurePosixPath(os.readlink(candidate))
            if link.is_absolute():
                parts = []
                pending.extendleft(reversed(link.parts[1:]))
            else:
                pending.extendleft(reversed(link.parts))
        else:
            parts.append(part)
    return root.joinpath(*parts)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def validate(root, binaries, desktop=True, sources=()):
    errors = []
    def exists(name, executable=False):
        try:
            p = target_path(root, name)
            return p.is_file() and (not executable or bool(p.stat().st_mode & 0o111))
        except ValueError as error:
            errors.append(str(error))
            return False
    for name in binaries + (RUNTIME if desktop else []):
        if not any(exists(f'{directory}/{name}', True) for directory in ['usr/bin', 'usr/sbin', 'bin', 'sbin']):
            errors.append(f'Missing executable in system PATH: {name}')
    for repository in sources:
        record = root / f'usr/share/raven/build/sources/{repository}.tsv'
        fields = record.read_text().strip().split('\t') if record.is_file() else []
        if len(fields) != 3 or fields[0] != repository or not re.fullmatch(r'[0-9a-f]{40}', fields[2]):
            errors.append(f'Missing or invalid source provenance: {repository}; rebuild its stage')
    if desktop:
        for name in DATA:
            if not exists(name):
                errors.append(f'Missing desktop data: /{name}')
        if not exists('etc/raven/session.d/50-roostbar', True):
            errors.append('RoostBar session hook is not executable')
        for pattern in ['usr/lib/spa-0.2/alsa/*.so', 'usr/lib/pipewire-0.3/*.so',
                        'usr/lib/modules/*/modules.dep', 'usr/lib/modules/*/modules.alias']:
            if not list(root.glob(pattern)):
                errors.append(f'Missing runtime plugins: {pattern}')
        for name in ['etc/skel/.config/raven/desktop.toml', 'etc/skel/.config/raven/config.toml', 'etc/skel/.config/roostbar/config.toml']:
            if exists(name):
                try:
                    tomllib.loads(target_path(root, name).read_text())
                except tomllib.TOMLDecodeError as error:
                    errors.append(f'{name}: {error}')
        files = [root / 'etc/raven/init.toml', *sorted(root.glob('etc/raven/init.d/*.toml'))]
        known = set()
        enabled = set()
        for config in files:
            if not config.is_file():
                continue
            try:
                services = tomllib.loads(config.read_text()).get('services', [])
            except tomllib.TOMLDecodeError as error:
                errors.append(f'{config.relative_to(root)}: {error}')
                continue
            for service in services:
                name = service['name']
                if name in known:
                    continue  # init.toml wins, matching raven-init
                known.add(name)
                if not service.get('enabled', True):
                    continue
                enabled.add(name)
                commands = [service.get('exec', ''), service.get('stop_exec', '')]
                pre = service.get('pre_exec', [])
                if pre:
                    commands.append(pre[0])
                for command in filter(None, commands):
                    if not exists(command, True):
                        errors.append(f'{name}: missing service executable {command}')
        for name in ['cawd', 'powerd', 'controlsd', 'timed', 'ports', 'bluetoothd']:
            if name not in enabled:
                errors.append(f'Missing enabled service definition: {name}')
        passwd = root / 'etc/passwd'
        names = {line.split(':')[0] for line in passwd.read_text().splitlines()} if passwd.exists() else set()
        for name in ['dbus', 'raven-greeter']:
            if name not in names:
                errors.append(f'Missing daemon account: {name}')
    # Inspect ELF metadata only: do not execute programs or resolve libraries
    # against the builder's root. Check plugins as well as launchable programs.
    #
    # /usr/lib/firmware is skipped: some blobs there (the ath10k WCN3990
    # wlanmdsp.mbn images, for one) are ELF files for the device's own DSP,
    # with NEEDED entries naming that firmware's libraries. Nothing on the
    # host loads them, so their link graph says nothing about the image.
    firmware = root / 'usr/lib/firmware'
    walked = set()
    for directory in ['usr/bin', 'usr/lib', 'usr/lib64']:
        top = root / directory
        # /usr/lib64 is a symlink onto /usr/lib under the usr-merge; walking it
        # again would report every problem twice under a second name.
        try:
            real = top.resolve(strict=True)
        except OSError:
            continue
        if real in walked:
            continue
        walked.add(real)
        for path in top.rglob('*'):
            if path.is_symlink() or not path.is_file():
                continue
            if firmware in path.parents:
                continue
            with path.open('rb') as stream:
                if stream.read(4) != b'\x7fELF':
                    continue
            metadata = subprocess.run(['readelf', '-l', '-d', str(path)], capture_output=True, text=True)
            if metadata.returncode:
                errors.append(f'Invalid ELF: {path.relative_to(root)}')
                continue
            interpreter = re.search(r'Requesting program interpreter: ([^\]]+)', metadata.stdout)
            if interpreter and not exists(interpreter[1]):
                errors.append(f'{path.relative_to(root)}: missing loader {interpreter[1]}')
            origin = '/' + str(path.parent.relative_to(root))
            search = []
            for runpath in re.findall(r'\((?:RUNPATH|RPATH)\).*?\[([^\]]+)\]', metadata.stdout):
                search.extend(runpath.replace('${ORIGIN}', origin).replace('$ORIGIN', origin).split(':'))
            search += ['/usr/lib', '/usr/lib64', '/lib', '/lib64']
            for needed in re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', metadata.stdout):
                if '/' in needed:
                    found = needed.startswith('/') and exists(needed)
                else:
                    found = any(exists(f'{folder}/{needed}') for folder in search if folder.startswith('/'))
                if not found:
                    errors.append(f'{path.relative_to(root)}: missing library {needed}')
    return errors


def manifest(root, project, binaries, errors):
    files = {}
    for directory in ['usr/bin', 'boot', 'etc/raven', 'etc/modprobe.d', 'etc/skel/.config', 'usr/share/wallpaper/set']:
        for path in sorted((root / directory).rglob('*')):
            if path.is_file() and not path.is_symlink():
                files['/' + str(path.relative_to(root))] = digest(path)
    sources = []
    for path in sorted(root.glob('usr/share/raven/build/sources/*.tsv')):
        fields = path.read_text().strip().split('\t')
        if len(fields) == 3:
            repo, url, commit = fields
            sources.append({'repository': repo, 'url': url, 'commit': commit})
    recipes = {}
    for directory in ['scripts', 'configs', 'packages', 'init', 'installer-ui', 'fonts', 'bootloader', 'etc']:
        for base, directories, names in os.walk(project / directory):
            directories[:] = [name for name in directories
                              if name not in ['target', '.git', '.ivaldi', '__pycache__']]
            for name in sorted(names):
                path = Path(base) / name
                if path.is_file() and not path.is_symlink():
                    recipes[str(path.relative_to(project))] = digest(path)
    for name in ['Dockerfile', 'lazy.toml']:
        if (project / name).is_file():
            recipes[name] = digest(project / name)
    destination = root / 'usr/share/raven/build' 
    destination.mkdir(parents=True, exist_ok=True)
    data = {'schema': 1, 'complete': not errors, 'validation_errors': errors,
            'expected_binaries': binaries, 'sources': sources,
            'files_sha256': files, 'recipe_sha256': dict(sorted(recipes.items()))}
    runtime = destination / 'desktop-packages.txt'
    data['desktop_packages'] = runtime.read_text().splitlines() if runtime.is_file() else []
    (destination / 'manifest.json').write_text(json.dumps(data, indent=2) + '\n')
    (destination / 'sources.tsv').write_text(''.join(f"{s['repository']}\t{s['url']}\t{s['commit']}\n" for s in sources))
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--project', type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument('--allow-incomplete', action='store_true')
    parser.add_argument('--binary', action='append', default=[])
    parser.add_argument('--source', action='append', default=[])
    args = parser.parse_args()
    root = args.root.absolute()
    errors = validate(root, args.binary, sources=args.source)
    for error in errors:
        print('ERROR: ' + error, file=sys.stderr)
    if errors and not args.allow_incomplete:
        return 1
    destination = manifest(root, args.project, args.binary, errors)
    print(f'Desktop validation: {len(errors)} error(s); provenance: {destination}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
