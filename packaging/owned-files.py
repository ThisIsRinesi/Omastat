#!/usr/bin/python3
"""Conservative, atomic installation of receipt-owned service/browser files."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import stat
import sys
import tempfile
import time

HOME = Path.home()
def xdg(name, default):
    return Path(os.environ.get(name) or HOME / default)


def safe_path(path):
    if not path.is_absolute() or path.resolve() != path:
        raise ValueError(f'Refusing symlink or non-canonical target: {path}')


def fingerprint(path):
    safe_path(path)
    try:
        info = path.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(info.st_mode):
        raise ValueError(f'Refusing non-regular target: {path}')
    return {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'mode': stat.S_IMODE(info.st_mode)}


def install_fingerprint(path):
    safe_path(path.parent)
    if path.is_symlink():
        return {'symlink': os.readlink(path)}
    return fingerprint(path)


def backup_target(path):
    # Omarchy refresh-config naming, with exclusive creation on collisions.
    base = str(path) + f'.bak.{int(time.time())}'
    suffix = 0
    while True:
        backup = Path(base + (f'-{suffix}' if suffix else ''))
        try:
            if path.is_symlink():
                os.symlink(os.readlink(path), backup)
            else:
                # Copy rather than hard-link: later writes through an old open
                # descriptor must not change the saved backup.
                with path.open('rb') as source:
                    fd = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                    with os.fdopen(fd, 'wb') as destination:
                        shutil.copyfileobj(source, destination)
                        os.fchmod(destination.fileno(), stat.S_IMODE(os.fstat(source.fileno()).st_mode))
                        destination.flush()
                        os.fsync(destination.fileno())
            print(f'Saved backup as {backup}')
            return backup
        except FileExistsError:
            suffix += 1


def atomic_write(path, data, mode, *, create_only=False, replace_symlink=False):
    safe_path(path.parent if replace_symlink else path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.omastat-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            os.fchmod(stream.fileno(), mode)
            stream.flush()
            os.fsync(stream.fileno())
        if create_only:
            # link() publishes a complete file and refuses a newly appeared target.
            os.link(temporary, path)
        else:
            os.replace(temporary, path)
    finally:
        if os.path.lexists(temporary):
            os.unlink(temporary)


def browser_plan():
    host = 'io.github.thisisrinesi.omastat'
    extension = 'omastat-domain-tracker@thisisrinesi.github.io.xpi'
    wrapper = xdg('XDG_BIN_HOME', '.local/bin') / 'omastat-native-host'
    binary = shutil.which('omastat')
    if not binary and os.access(HOME / '.cargo/bin/omastat', os.X_OK):
        binary = str(HOME / '.cargo/bin/omastat')
    if not binary:
        raise ValueError('omastat binary not found; install it first')
    binary = str(Path(binary).resolve())
    plan = [(wrapper, f'#!/bin/bash\nexec {shlex.quote(binary)} native-host\n'.encode(), 0o755)]
    manifest = json.dumps({'name': host, 'description': 'Omastat browser domain native host',
                          'path': str(wrapper), 'type': 'stdio',
                          'allowed_extensions': [extension[:-4]]}, indent=2).encode() + b'\n'
    for root in [xdg('XDG_CONFIG_HOME', '.config') / 'zen',
                 xdg('XDG_CONFIG_HOME', '.config') / 'mozilla', HOME / '.mozilla', HOME / '.zen']:
        plan.append((root / 'native-messaging-hosts' / (host + '.json'), manifest, 0o644))
    archive = (Path(__file__).parent / 'browser-extension/signed/omastat-domain-tracker-firefox.xpi').read_bytes()
    storage = xdg('XDG_DATA_HOME', '.local/share') / 'omastat/browser-extension'
    for browser in ['firefox', 'zen']:
        plan.append((storage / f'omastat-domain-tracker-{browser}.xpi', archive, 0o644))
    for root in [HOME / '.zen', HOME / '.mozilla/firefox']:
        if root.is_dir():
            for profile in root.iterdir():
                if profile.is_dir() and any((profile / name).is_file() for name in ['prefs.js', 'extensions.json']):
                    plan.append((profile / 'extensions' / extension, archive, 0o644))
    return plan


def main():
    action, group = sys.argv[1:]
    if action not in ('install', 'uninstall') or group not in ('service', 'browser'):
        raise ValueError('Expected install/uninstall and service/browser')
    state = xdg('XDG_STATE_HOME', '.local/state') / 'omastat/install-ownership'
    safe_path(state)
    state.mkdir(parents=True, exist_ok=True, mode=0o700)
    lock = state / 'lock'
    safe_path(lock)
    with open(lock, 'a') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX)
        receipt = state / (group + '.json')
        safe_path(receipt)
        records = json.loads(receipt.read_text()) if receipt.exists() else {}
        def save():
            atomic_write(receipt, (json.dumps(records, indent=2) + '\n').encode(), 0o600)
        if action == 'install':
            plan = browser_plan() if group == 'browser' else [
                (xdg('XDG_CONFIG_HOME', '.config') / 'systemd/user/omastat.service',
                 (Path(__file__).parent / 'systemd/omastat.service').read_bytes(), 0o644)]
            # Validate parents/types before changing any target. Leaf symlinks
            # are backed up as links; their referents are never written.
            for path, data, mode in plan:
                install_fingerprint(path)
            for path, data, mode in plan:
                current = install_fingerprint(path)
                expected = {'sha256': hashlib.sha256(data).hexdigest(), 'mode': mode}
                if current is not None and (records.get(str(path)) != current or current != expected):
                    backup_target(path)
                atomic_write(path, data, mode, create_only=current is None, replace_symlink=True)
                records[str(path)] = expected
                save()
        else:
            for name, expected in list(records.items()):
                path = Path(name)
                try:
                    current = fingerprint(path)
                except ValueError as error:
                    print(f'omastat: Preserving {path}: {error}', file=sys.stderr)
                    continue
                if current is None:
                    del records[name]
                    save()
                elif current == expected:
                    if group == 'service':
                        import subprocess
                        subprocess.run(['systemctl', '--user', 'disable', '--now', 'omastat.service'], check=True)
                        if fingerprint(path) != expected:
                            print(f'omastat: Preserving changed target: {path}', file=sys.stderr)
                            continue
                    path.unlink()
                    del records[name]
                    save()
                else:
                    print(f'omastat: Preserving modified target: {path}', file=sys.stderr)
            if group == 'browser':
                storage = xdg('XDG_DATA_HOME', '.local/share') / 'omastat/browser-extension'
                try:
                    safe_path(storage)
                    storage.rmdir()  # Never recursively remove untracked contents.
                except (OSError, ValueError):
                    pass


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError) as error:
        sys.exit(f'omastat: {error}')
