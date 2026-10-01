#!/usr/bin/env python3
"""Install the local Git driver; hydrate untouched files after a fresh clone."""

import argparse
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, help='ordinary Git worktree (otherwise use yadm)')
    args = parser.parse_args()
    if args.repo:
        root = args.repo.resolve()
        git = ['git', '-C', str(root)]
    else:
        repository = os.environ.get('YADM_HOOK_REPO') or subprocess.check_output(['yadm', 'introspect', 'repo'], text=True).strip()
        git = ['git', '--git-dir=' + repository]
        worktree = os.environ.get('YADM_HOOK_WORK') or subprocess.check_output(git + ['config', '--get', 'core.worktree'], text=True).strip()
        root = Path(worktree).resolve()
        git += ['--work-tree=' + str(root), '-C', str(root)]
    driver = root / '.config/yadm/path-filter.py'
    if not driver.is_file():
        sys.exit('Missing path filter: ' + str(driver))
    command = shlex.quote(sys.executable) + ' ' + shlex.quote(str(driver))
    for operation in ('clean', 'smudge'):
        subprocess.run(git + ['config', '--local', 'filter.dotfiles-home.' + operation, command + ' ' + operation + ' %f'], check=True)
    subprocess.run(git + ['config', '--local', 'filter.dotfiles-home.required', 'true'], check=True)
    subprocess.run(git + ['config', '--local', 'merge.renormalize', 'true'], check=True)

    names = subprocess.check_output(git + ['ls-files', '-z']).split(b'\0')
    attributes = subprocess.check_output(git + ['check-attr', '-z', '--stdin', 'filter'], input=b'\0'.join(names))
    fields = attributes.split(b'\0')
    hydrated = []
    for index in range(0, len(fields) - 2, 3):
        name, _, value = fields[index:index + 3]
        if value != b'dotfiles-home':
            continue
        file = root / os.fsdecode(name)
        if not file.is_file() or file.is_symlink():
            continue
        live = file.read_bytes()
        if b'@@DOTFILES_' not in live:
            continue
        canonical = subprocess.check_output(git + ['show', ':' + os.fsdecode(name)])
        if live != canonical:
            print('Preserving locally edited file: ' + os.fsdecode(name), file=sys.stderr)
            continue
        if file.read_bytes() != live:
            sys.exit('File changed during setup: ' + str(file))
        # Git must record the rendered file size, otherwise status reports a
        # false modification forever. -u updates stat data without staging.
        # Move the raw file aside: checkout-index skips already up-to-date files,
        # even with -f, and doesn't know that the driver was just installed.
        descriptor, temporary = tempfile.mkstemp(prefix='.yadm-path-', dir=file.parent)
        os.close(descriptor)
        backup = Path(temporary)
        try:
            file.replace(backup)
            try:
                subprocess.run(git + ['checkout-index', '-f', '-u', '--', os.fsdecode(name)], check=True)
            except BaseException:
                backup.replace(file)
                raise
        finally:
            backup.unlink(missing_ok=True)
        hydrated.append(os.fsdecode(name))
    print('Installed dotfiles path filter; hydrated ' + str(len(hydrated)) + ' untouched files.')


if __name__ == '__main__':
    main()
