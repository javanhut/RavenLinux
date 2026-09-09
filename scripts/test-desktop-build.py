#!/usr/bin/env python3
"""Offline regression tests for image completeness and source selection."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

PROJECT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('contract', PROJECT / 'scripts/check-desktop-image.py')
contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(contract)


class ImageContract(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def put(self, name, text='', executable=False):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        if executable:
            path.chmod(0o755)
        return path

    def desktop(self):
        for name in contract.RUNTIME + ['huginn', 'roostbar', 'raven-output']:
            self.put('usr/bin/' + name, '#!/bin/sh\n', True)
        for name in contract.DATA:
            self.put(name)
        self.put('etc/raven/session.d/50-roostbar', '#!/bin/sh\nexec /usr/bin/roostbar\n', True)
        shutil.copyfile(PROJECT / 'init/config/init.toml', self.root / 'etc/raven/init.toml')
        shutil.copyfile(PROJECT / 'configs/raven/services/bluetoothd.toml', self.root / 'etc/raven/init.d/bluetoothd.toml')
        for path in [self.root / 'etc/raven/init.toml', self.root / 'etc/raven/init.d/bluetoothd.toml']:
            for service in contract.tomllib.loads(path.read_text())['services']:
                for executable in [service['exec'], service.get('stop_exec', '')]:
                    if executable:
                        self.put(executable.lstrip('/'), '#!/bin/sh\n', True)
        self.put('usr/lib/modules/test/modules.dep')
        self.put('usr/lib/modules/test/modules.alias')
        self.put('usr/lib/spa-0.2/alsa/libspa-alsa.so')
        self.put('usr/lib/pipewire-0.3/libpipewire-module-protocol-native.so')
        self.put('etc/passwd', 'dbus:x:81:81::/:/bin/false\npolkitd:x:900:900::/:/bin/false\nraven-greeter:x:971:971::/:/bin/false\n')

    def test_complete_desktop(self):
        self.desktop()
        self.assertEqual(contract.validate(self.root, ['huginn', 'raven-output']), [])

    def test_missing_source_record_rejects_old_sysroot(self):
        self.desktop()
        errors = contract.validate(self.root, ['huginn'], sources=['RavenGUI'])
        self.assertTrue(any('source provenance: RavenGUI' in error for error in errors))

    def test_disabled_daemon_is_not_complete(self):
        self.desktop()
        config = self.root / 'etc/raven/init.d/bluetoothd.toml'
        config.write_text(config.read_text().replace('enabled = true', 'enabled = false'))
        self.assertTrue(any('enabled service definition: bluetoothd' in error
                            for error in contract.validate(self.root, ['huginn'])))

    def test_new_user_defaults_are_portable(self):
        source = (PROJECT / 'scripts/stages/stage-gui.sh').read_text()
        start = source.index('install_wallpaper_dirs() {')
        end = source.index('stage_desktop_runtime() {', start)
        script = source[start:end] + '\nchown() { :; }\ninstall_desktop_defaults\n'
        env = dict(os.environ, PROJECT_ROOT=str(PROJECT), SYSROOT_DIR=str(self.root))
        subprocess.run(['bash', '-e', '-c', script], env=env, check=True)
        for home in ['etc/skel', 'home/raven']:
            config = contract.tomllib.loads((self.root / home / '.config/raven/desktop.toml').read_text())
            self.assertEqual(config['appearance']['accent'], '#22C5DD')
            self.assertEqual(config['appearance']['scale'], 0.9)
            self.assertNotIn('/home/javanstorm', (self.root / home / '.config/raven/config.toml').read_text())
            self.assertFalse((self.root / home / '.config/raven/session.d').exists())
        self.assertEqual(contract.digest(self.root / 'usr/share/wallpaper/set/wallpaper.jpg'),
                         contract.digest(PROJECT / 'configs/desktop/wallpaper.jpg'))

    def test_missing_component_cannot_hide_in_documentation(self):
        self.put('usr/share/doc/raven-output', 'not executable')
        self.assertTrue(any('raven-output' in e for e in contract.validate(self.root, ['raven-output'], False)))

    def test_absolute_symlink_does_not_use_host_binary(self):
        path = self.root / 'usr/bin/huginn'
        path.parent.mkdir(parents=True)
        path.symlink_to('/usr/bin/sh')
        self.assertEqual(contract.target_path(self.root, 'usr/bin/huginn'), self.root / 'usr/bin/sh')
        self.assertTrue(contract.validate(self.root, ['huginn'], False))

    def test_missing_service_executable_and_plugin(self):
        self.desktop()
        (self.root / 'usr/lib/bluetooth/bluetoothd').unlink()
        (self.root / 'usr/lib/spa-0.2/alsa/libspa-alsa.so').unlink()
        errors = contract.validate(self.root, ['huginn'])
        self.assertTrue(any('bluetoothd: missing service' in e for e in errors))
        self.assertTrue(any('Missing runtime plugins' in e for e in errors))

    def test_missing_elf_library_is_detected(self):
        if not shutil.which('cc'):
            self.skipTest('cc unavailable')
        library = self.put('lib.c', 'int test_value(void) { return 1; }\n')
        main = self.put('main.c', 'extern int test_value(void); int main(void) { return test_value(); }\n')
        subprocess.run(['cc', '-shared', '-fPIC', str(library), '-Wl,-soname,libraven-audit.so', '-o', str(self.root / 'libraven-audit.so')], check=True)
        target = self.root / 'usr/bin/demo'
        target.parent.mkdir(parents=True)
        subprocess.run(['cc', str(main), '-L' + str(self.root), '-lraven-audit', '-o', str(target)], check=True)
        errors = contract.validate(self.root, ['demo'], False)
        self.assertTrue(any('missing library libraven-audit.so' in e for e in errors))

    def test_strict_cli_rejects_incomplete_tree(self):
        result = subprocess.run(['python3', str(PROJECT / 'scripts/check-desktop-image.py'), str(self.root), '--binary', 'huginn'], capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / 'usr/share/raven/build/manifest.json').exists())

    def test_manifest_records_full_source_and_artifact_hash(self):
        self.put('usr/bin/huginn', 'binary', True)
        sha = 'a' * 40
        self.put('usr/share/raven/build/sources/RavenGUI.tsv', f'RavenGUI\thttps://example.invalid/RavenGUI\t{sha}\n')
        dest = contract.manifest(self.root, self.root / 'no-project', ['huginn'], [])
        manifest = json.loads((dest / 'manifest.json').read_text())
        self.assertEqual(manifest['sources'][0]['commit'], sha)
        self.assertEqual(manifest['files_sha256']['/usr/bin/huginn'], contract.digest(self.root / 'usr/bin/huginn'))


class SourceSelection(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.remote = self.base / 'remote'
        self.remote.mkdir()
        self.git('init', '-q', '-b', 'main')
        self.git('config', 'user.name', 'Build Test')
        self.git('config', 'user.email', 'build@example.invalid')
        (self.remote / 'source').write_text('one')
        self.git('add', 'source')
        self.git('commit', '-qm', 'one')
        self.first = self.git('rev-parse', 'HEAD').strip()
        (self.remote / 'source').write_text('two')
        self.git('commit', '-qam', 'two')
        self.second = self.git('rev-parse', 'HEAD').strip()
        self.dest = self.base / 'checkout'

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.remote), *args], text=True)

    def fetch(self, ref='', offline=False, url=None, lock=None):
        env = dict(os.environ, SYSROOT_DIR=str(self.base / 'sysroot'))
        if lock:
            env['RAVEN_SOURCE_LOCK'] = str(lock)
        return subprocess.run(['bash', '-c', '''
source "$1"
log_info() { :; }
log_warn() { echo "$*" >&2; }
raven_fetch_repo RavenGUI "$2" "$3" "$4" "$5" env
''', 'test', str(PROJECT / 'scripts/lib/components.sh'), url or self.remote.as_uri(), str(self.dest), ref, '1' if offline else '0'], env=env, capture_output=True, text=True)

    def test_exact_commit_and_offline_pin_mismatch(self):
        self.assertEqual(self.fetch(self.first).returncode, 0)
        self.assertNotEqual(self.fetch(self.second, True).returncode, 0)
        record = self.base / 'sysroot/usr/share/raven/build/sources/RavenGUI.tsv'
        self.assertIn(self.first, record.read_text())

    def test_invalid_branch_does_not_fall_back_to_default(self):
        self.assertNotEqual(self.fetch('does-not-exist').returncode, 0)

    def test_failed_update_does_not_accept_cached_checkout(self):
        self.assertEqual(self.fetch().returncode, 0)
        self.assertNotEqual(self.fetch(url=(self.base / 'absent').as_uri()).returncode, 0)

    def test_lock_selects_exact_commit(self):
        lock = self.base / 'sources.tsv'
        lock.write_text(f'RavenGUI\t{self.remote.as_uri()}\t{self.first}\n')
        self.assertEqual(self.fetch(lock=lock).returncode, 0)
        actual = subprocess.check_output(['git', '-C', str(self.dest), 'rev-parse', 'HEAD'], text=True).strip()
        self.assertEqual(actual, self.first)

    def test_container_forwards_component_controls(self):
        engine = self.base / 'engine'
        engine.write_text('#!/usr/bin/env python3\nimport json,os,sys\nopen(os.environ["CAPTURE"],"w").write(json.dumps(sys.argv[1:]))\n')
        engine.chmod(0o755)
        capture = self.base / 'args.json'
        env = dict(os.environ, RAVEN_ENGINE=str(engine), RAVEN_NO_BUILD='1', CAPTURE=str(capture),
                   GUI_REF='gui-pin', SETTINGS_REF='settings-pin', CONTROLS_OFFLINE='1', RAVEN_CAW_REF='caw-pin')
        subprocess.run(['bash', str(PROJECT / 'scripts/docker-build.sh'), 'all'], env=env, check=True, capture_output=True)
        args = json.loads(capture.read_text())
        for entry in ['GUI_REF=gui-pin', 'SETTINGS_REF=settings-pin', 'CONTROLS_OFFLINE=1', 'RAVEN_CAW_REF=caw-pin']:
            self.assertIn(entry, args)


if __name__ == '__main__':
    unittest.main()
