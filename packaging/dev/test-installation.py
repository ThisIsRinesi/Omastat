#!/usr/bin/env python3
"""Exercise installation in a filesystem namespace with fake Cargo and desktop IPC."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[2]
OMARCHY_BIN = Path("/usr/share/omarchy/bin")
USER_HOME = Path.home()
MOCK = r'''#!/usr/bin/python3
import json, os, pathlib, subprocess, sys
p = pathlib.Path(os.environ['OMASTAT_TEST_STATE'])
s = json.loads(p.read_text())
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
s.setdefault('calls', []).append([name] + args)
def save(): p.write_text(json.dumps(s))
def fail():
    save()
    sys.exit(1)
home = pathlib.Path.home()
plugin = home / '.config/omarchy/plugins/local.omastat'
if name == 'cargo':
    if '--list' in args:
        if s.get('installed'): print('omastat v0.1.7:\n    omastat\n    omastatd')
    elif args[0] == 'install':
        if s.get('build_fail'): fail()
        s['installed'] = True
        root = pathlib.Path(args[args.index('--root') + 1])
        (root / 'bin').mkdir(parents=True, exist_ok=True)
        for binary in ['omastat', 'omastatd']:
            f = root / 'bin' / binary
            f.write_text('#!/bin/sh\nexit 0\n')
            f.chmod(0o755)
    elif args[0] == 'uninstall':
        s['installed'] = False
        for binary in ['omastat', 'omastatd']:
            (home / '.cargo/bin' / binary).unlink(missing_ok=True)
elif name == 'systemctl':
    if 'show-environment' in args and s.get('no_session'): fail()
    if 'restart' in args: s['service'] = True
    if 'disable' in args: s['service'] = False
    if 'is-active' in args and not s.get('service'): fail()
    if 'cat' in args and not (home / '.config/systemd/user/omastat.service').exists(): fail()
elif name == 'omarchy-shell':
    if 'ping' in args:
        if s.get('no_shell'): fail()
        print('pong')
    elif 'setPluginEnabled' in args:
        s['enabled'] = args[-1] == 'true'
        print('ok')
    elif 'listPlugins' in args:
        print(json.dumps([{'id': 'local.omastat', 'enabled': s.get('enabled', False)}] if plugin.exists() and not s.get('no_discovery') else []))
elif name == 'omarchy':
    if args[:2] == ['plugin', 'validate']:
        manifest = json.loads((pathlib.Path(args[2]) / 'manifest.json').read_text())
        assert manifest['id'] == 'local.omastat'
    elif args[:2] == ['plugin', 'enable']:
        assert args == ['plugin', 'enable', 'local.omastat'], 'must preserve bar placement'
        s['enabled'] = True
    elif args[:2] == ['plugin', 'disable']: s['enabled'] = False
    elif args[:2] in [['plugin', 'remove'], ['plugin', 'update']]:
        # Exercise Omarchy's actual lifecycle commands, with only desktop IPC mocked.
        save()
        sys.exit(subprocess.run(['/usr/share/omarchy/bin/omarchy-plugin-' + args[1]] + args[2:]).returncode)
save()
'''


class InstallationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='omastat-install-test-')
        self.root = Path(self.temp.name)
        self.home = self.root / 'home'
        self.home.mkdir()
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.source = self.root / 'repo'
        self.source.mkdir()
        self.state = self.root / 'state.json'
        self.state.write_text('{}')
        for name in ['cargo', 'systemctl', 'omarchy', 'omarchy-shell']:
            path = self.bin / name
            path.write_text(MOCK)
            path.chmod(0o755)
        self.data = self.home / '.local/share/omastat/omastat.db'
        self.data.parent.mkdir(parents=True)
        self.data.write_text('recorded activity must survive')
        self.config = self.home / '.config/omastat/config.toml'
        self.config.parent.mkdir(parents=True)
        self.config.write_text('# user settings')
        self.other_plugin = self.home / '.config/omarchy/plugins/other/keep'
        self.other_plugin.parent.mkdir(parents=True)
        self.other_plugin.write_text('unrelated plugin')

    def tearDown(self):
        self.temp.cleanup()

    def run_script(self, script, *args, expected=0):
        # HOME retains its real value. Only its filesystem mount is replaced;
        # the real desktop, binaries, and database are inaccessible to the scripts.
        command = [
            'bwrap', '--die-with-parent', '--unshare-all', '--ro-bind', '/', '/',
            '--dev', '/dev', '--proc', '/proc',
            '--bind', str(self.root), str(self.root),
            '--bind', str(self.home), str(USER_HOME),
            '--ro-bind', str(REPO), str(self.source), '--chdir', str(self.source),
            '--setenv', 'PATH', str(self.bin) + ':' + str(OMARCHY_BIN) + ':/usr/bin',
            '--setenv', 'OMASTAT_TEST_STATE', str(self.state),
        ]
        for variable in ['XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_BIN_HOME', 'CARGO_HOME', 'CARGO_INSTALL_ROOT']:
            command += ['--unsetenv', variable]
        result = subprocess.run(command + ['bash', str(self.source / script), *args], capture_output=True, text=True, timeout=30)
        self.assertNotIn("bwrap:", result.stderr, result.stderr)
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return result

    def read_state(self):
        return json.loads(self.state.read_text())

    def assert_preserved(self):
        self.assertEqual(self.data.read_text(), 'recorded activity must survive')
        self.assertEqual(self.config.read_text(), '# user settings')
        self.assertEqual(self.other_plugin.read_text(), 'unrelated plugin')

    def test_install_update_uninstall_and_repeat(self):
        self.run_script('install.sh')
        plugin = self.home / '.config/omarchy/plugins/local.omastat'
        self.assertEqual((plugin / 'Panel.qml').read_bytes(), (REPO / 'packaging/omarchy/omastat/Panel.qml').read_bytes())
        self.assertTrue(self.read_state()['enabled'])
        self.assertTrue(self.read_state()['service'])
        self.assertTrue((self.home / '.cargo/bin/omastatd').exists())
        (plugin / 'custom-note').write_text('preserve in backup')
        self.run_script('install.sh')
        backups = list((self.home / '.local/state/omastat/install-backups').glob('*/plugin/custom-note'))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), 'preserve in backup')
        self.assertFalse((plugin / 'custom-note').exists())
        self.run_script('uninstall.sh')
        self.assertFalse(plugin.exists())
        self.assertFalse(self.read_state()['service'])
        self.assertFalse(self.read_state()['installed'])
        self.assertFalse((self.home / '.config/systemd/user/omastat.service').exists())
        self.assertFalse((self.home / '.cargo/bin/omastat').exists())
        self.run_script('uninstall.sh')
        self.assert_preserved()

    def test_browser_install_and_removal_is_scoped(self):
        for relative in ['.zen/test-profile', '.mozilla/firefox/test-profile']:
            profile = self.home / relative
            (profile / 'extensions').mkdir(parents=True)
            (profile / 'prefs.js').write_text('// existing preferences')
            (profile / 'extensions/other.xpi').write_text('unrelated extension')
        self.run_script('install.sh', '--with-browser')
        self.assertTrue((self.home / '.zen/test-profile/extensions/omastat-domain-tracker@thisisrinesi.github.io.xpi').exists())
        signed = (REPO / 'packaging/browser-extension/signed/omastat-domain-tracker-firefox.xpi').read_bytes()
        self.assertEqual((self.home / '.zen/test-profile/extensions/omastat-domain-tracker@thisisrinesi.github.io.xpi').read_bytes(), signed)
        self.assertEqual((self.home / '.mozilla/firefox/test-profile/extensions/omastat-domain-tracker@thisisrinesi.github.io.xpi').read_bytes(), signed)
        self.assertEqual((self.home / '.local/share/omastat/browser-extension/omastat-domain-tracker-firefox.xpi').read_bytes(), signed)
        self.run_script('uninstall.sh')
        for relative in ['.zen/test-profile', '.mozilla/firefox/test-profile']:
            profile = self.home / relative
            self.assertEqual((profile / 'extensions/other.xpi').read_text(), 'unrelated extension')
            self.assertFalse((profile / 'extensions/omastat-domain-tracker@thisisrinesi.github.io.xpi').exists())
        self.assertFalse((self.home / '.local/bin/omastat-native-host').exists())
        self.assertFalse((self.home / '.local/share/omastat/browser-extension').exists())
        self.assert_preserved()

    def test_foreign_service_is_backed_up_and_replaced(self):
        service = self.home / '.config/systemd/user/omastat.service'
        service.parent.mkdir(parents=True)
        service.write_text('foreign service')
        self.run_script('install.sh')
        backups = list(service.parent.glob('omastat.service.bak.*'))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), 'foreign service')
        self.assertIn('ExecStart=%h/.cargo/bin/omastatd', service.read_text())
        self.assertNotIn('/usr/bin/env', service.read_text())
        self.run_script('uninstall.sh')
        self.assertFalse(service.exists())
        self.assertEqual(backups[0].read_text(), 'foreign service')

    def test_modified_owned_service_is_preserved(self):
        self.run_script('install.sh')
        service = self.home / '.config/systemd/user/omastat.service'
        service.write_text('user replacement')
        self.run_script('uninstall.sh')
        self.assertEqual(service.read_text(), 'user replacement')
        self.assertFalse(any('disable' in call and call[0] == 'systemctl'
                             for call in self.read_state()['calls']))

    def browser_targets(self):
        return [self.home / relative for relative in [
            '.local/bin/omastat-native-host',
            '.config/zen/native-messaging-hosts/io.github.thisisrinesi.omastat.json',
            '.local/share/omastat/browser-extension/omastat-domain-tracker-firefox.xpi',
            '.zen/test-profile/extensions/omastat-domain-tracker@thisisrinesi.github.io.xpi']]

    def test_browser_backs_up_foreign_targets(self):
        self.run_script('install.sh')
        profile = self.home / '.zen/test-profile'
        profile.mkdir(parents=True)
        (profile / 'prefs.js').touch()
        for target in self.browser_targets():
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text('foreign')
        self.run_script('packaging/browser-extension/install.sh')
        self.run_script('packaging/browser-extension/uninstall.sh')
        for target in self.browser_targets():
            self.assertFalse(target.exists())
            backups = list(target.parent.glob(target.name + '.bak.*'))
            self.assertEqual(len(backups), 1)
            self.assertEqual(backups[0].read_text(), 'foreign')

    def test_browser_modified_files_and_untracked_contents_survive(self):
        profile = self.home / '.zen/test-profile'
        profile.mkdir(parents=True)
        (profile / 'prefs.js').touch()
        self.run_script('install.sh', '--with-browser')
        self.run_script('packaging/browser-extension/install.sh')
        for target in self.browser_targets():
            target.write_text('user replacement')
        extra = self.home / '.local/share/omastat/browser-extension/unrelated'
        extra.write_text('keep')
        self.run_script('uninstall.sh')
        self.run_script('uninstall.sh')
        for target in self.browser_targets():
            self.assertEqual(target.read_text(), 'user replacement')
        self.assertEqual(extra.read_text(), 'keep')
        self.assertFalse((extra.parent / 'omastat-domain-tracker-zen.xpi').exists())

    def test_symlink_replacement_is_preserved(self):
        self.run_script('install.sh', '--with-browser')
        wrapper = self.browser_targets()[0]
        wrapper.unlink()
        wrapper.symlink_to(self.config)
        self.run_script('uninstall.sh')
        self.assertTrue(wrapper.is_symlink())
        self.assert_preserved()

    def test_reinstall_backs_up_symlink_without_changing_referent(self):
        self.run_script('install.sh', '--with-browser')
        wrapper = self.browser_targets()[0]
        wrapper.unlink()
        wrapper.symlink_to(self.config)
        self.run_script('packaging/browser-extension/install.sh')
        self.assertFalse(wrapper.is_symlink())
        backups = list(wrapper.parent.glob(wrapper.name + '.bak.*'))
        self.assertEqual(len(backups), 1)
        self.assertTrue(backups[0].is_symlink())
        self.assertEqual(backups[0].readlink(), self.config)
        self.assert_preserved()

    def test_backup_timestamp_collisions_do_not_overwrite(self):
        # Freeze time to exercise multiple backups in the same second.
        import importlib.util
        spec = importlib.util.spec_from_file_location('owned_files', REPO / 'packaging/owned-files.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        from unittest.mock import patch
        target = self.root / 'target'
        target.write_text('first')
        with patch.object(module.time, 'time', return_value=123):
            first = module.backup_target(target)
            target.write_text('second')
            second = module.backup_target(target)
        self.assertEqual(first.name, 'target.bak.123')
        self.assertEqual(second.name, 'target.bak.123-1')
        self.assertEqual(first.read_text(), 'first')
        self.assertEqual(second.read_text(), 'second')

    def test_preflight_and_build_failure_leave_install_untouched(self):
        for failure in ['no_session', 'no_shell', 'build_fail']:
            self.state.write_text(json.dumps({failure: True}))
            self.run_script('install.sh', expected=1)
            self.assertFalse((self.home / '.config/omarchy/plugins/local.omastat').exists())
            self.assertFalse((self.home / '.config/systemd/user/omastat.service').exists())
            self.assert_preserved()

    def test_failed_upgrade_preserves_working_plugin(self):
        self.run_script('install.sh')
        plugin = self.home / '.config/omarchy/plugins/local.omastat/Panel.qml'
        previous = plugin.read_bytes()
        state = self.read_state()
        state['build_fail'] = True
        self.state.write_text(json.dumps(state))
        self.run_script('install.sh', expected=1)
        self.assertEqual(plugin.read_bytes(), previous)
        self.assertTrue(self.read_state()['service'])
        self.assert_preserved()

    def test_installed_checkout_is_not_moved_or_removed(self):
        plugin = self.home / '.config/omarchy/plugins/local.omastat'
        plugin.symlink_to(self.source, target_is_directory=True)
        for script in ['install.sh', 'uninstall.sh']:
            result = self.run_script(script, expected=1)
            self.assertIn('separate checkout', result.stderr)
        self.assertTrue(plugin.is_symlink())
        self.assertFalse(any(call[0] == 'cargo' for call in self.read_state()['calls']))
        self.assert_preserved()

    def git(self, directory, *args):
        return subprocess.run(['git', '-c', 'core.hooksPath=/dev/null', '-c',
                               'user.name=Install Test', '-c', 'user.email=test@example.invalid',
                               '-C', str(directory), *args], check=True, capture_output=True, text=True).stdout.strip()

    def git_fixture(self):
        origin = self.root / 'origin'
        origin.mkdir()
        self.git(origin, 'init', '--initial-branch=main')
        shutil.copy(REPO / 'manifest.json', origin / 'manifest.json')
        widget = origin / 'packaging/omarchy/omastat'
        shutil.copytree(REPO / 'packaging/omarchy/omastat', widget)
        (origin / 'crates/omastat').mkdir(parents=True)
        shutil.copy(REPO / 'crates/omastat/Cargo.toml', origin / 'crates/omastat/Cargo.toml')
        self.git(origin, 'add', '.')
        self.git(origin, 'commit', '-m', 'Original plugin')
        plugin = self.home / '.config/omarchy/plugins/local.omastat'
        self.git(self.root, 'clone', '--no-hardlinks', str(origin), str(plugin))
        return origin, plugin

    def test_git_managed_plugin_uses_native_update_and_matching_backend(self):
        origin, plugin = self.git_fixture()
        (origin / 'new-version').write_text('updated upstream')
        self.git(origin, 'add', '.')
        self.git(origin, 'commit', '-m', 'Update plugin')
        self.run_script('install.sh')
        self.assertTrue((plugin / '.git').is_dir())
        self.assertEqual(self.git(plugin, 'rev-parse', 'HEAD'), self.git(origin, 'rev-parse', 'HEAD'))
        self.assertEqual(self.git(plugin, 'remote', 'get-url', 'origin'), str(origin))
        calls = self.read_state()['calls']
        self.assertIn(['omarchy', 'plugin', 'update', 'local.omastat', '--yes'], calls)
        cargo = next(call for call in calls if call[:2] == ['cargo', 'install'])
        self.assertEqual(cargo[cargo.index('--path') + 1], str(USER_HOME / '.config/omarchy/plugins/local.omastat/crates/omastat'))
        self.run_script('uninstall.sh')
        self.assertIn(['omarchy', 'plugin', 'remove', 'local.omastat', '--yes'], self.read_state()['calls'])
        self.assertFalse(plugin.exists())
        self.assert_preserved()

    def test_native_update_refuses_to_overwrite_local_changes(self):
        origin, plugin = self.git_fixture()
        (origin / 'manifest.json').write_text((origin / 'manifest.json').read_text().replace('Omastat', 'Upstream Omastat'))
        self.git(origin, 'add', '.')
        self.git(origin, 'commit', '-m', 'Upstream manifest update')
        local = (plugin / 'manifest.json').read_text().replace('Omastat', 'My Omastat')
        (plugin / 'manifest.json').write_text(local)
        self.run_script('install.sh', expected=1)
        self.assertEqual((plugin / 'manifest.json').read_text(), local)
        self.assertFalse(any(call[0] == 'cargo' for call in self.read_state()['calls']))
        self.assert_preserved()

    def test_help_and_invalid_option_do_not_mutate(self):
        for script in ['install.sh', 'uninstall.sh']:
            self.run_script(script, '--help')
            self.run_script(script, '--unknown', expected=2)
        self.assertEqual(self.read_state(), {})
        self.assert_preserved()


if __name__ == '__main__':
    if not (OMARCHY_BIN / 'omarchy-plugin-update').exists():
        raise SystemExit('The installed Omarchy CLI is required for lifecycle tests')
    if not shutil.which('bwrap'):
        raise SystemExit('bubblewrap is required for isolated installation tests')
    unittest.main()
