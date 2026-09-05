#!/usr/bin/env python3
"""Build local x86_64 release bundles with the installed GNOME SDK and locked Rust sources.

Only target/packaging and dist are generated. This does not install the application,
change Flatpak remotes, publish a repository, or alter the user's personal library.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / 'target/packaging'
DIST = ROOT / 'dist'
APP_ID = 'com.sbdd.Toniator'
SDK = 'org.gnome.Sdk'
RUNTIME = 'org.gnome.Platform'
BRANCH = '50'


def run(*args, **kwargs):
    """Run one checked build step, exposing its output without shell interpolation."""
    print('+', ' '.join(map(str, args)), flush=True)
    return subprocess.run(list(map(str, args)), cwd=ROOT, check=True, **kwargs)


def output(*args):
    """Read one checked command's UTF-8 output."""
    return subprocess.check_output(list(map(str, args)), cwd=ROOT, text=True).strip()


def install(source, destination):
    """Copy a build-owned file, creating its destination directory."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def sdk_build():
    """Compile both release binaries inside the SDK using the installed Rust toolchain/cache."""
    toolchain = Path(output('rustup', 'which', 'cargo')).parent
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
    build = WORK / 'sdk'
    if not (build / 'metadata').exists():
        run('flatpak', 'build-init', build, APP_ID, SDK, RUNTIME, BRANCH)
    run('flatpak', 'build', '--unshare=network', f'--filesystem={ROOT}',
        f'--filesystem={toolchain.parent}:ro', f'--filesystem={cargo_home}',
        f'--env=PATH={toolchain}:/usr/bin', f'--env=CARGO_HOME={cargo_home}',
        f'--env=CARGO_TARGET_DIR={WORK / "cargo"}', build,
        toolchain / 'cargo', 'build', '--manifest-path', ROOT / 'Cargo.toml',
        '--release', '--locked', '--offline', '-p', 'toniator-app', '-p', 'toniator-cli')
    return build


def payload(prefix):
    """Install the two binaries and standard desktop integration into a private prefix."""
    for name in ['toniator-app', 'toniator']:
        install(WORK / 'cargo/release' / name, prefix / 'bin' / name)
        run('strip', '--strip-unneeded', prefix / 'bin' / name)
    for suffix, folder in [('.desktop', 'applications'), ('.metainfo.xml', 'metainfo')]:
        install(ROOT / 'packaging' / (APP_ID + suffix), prefix / 'share' / folder / (APP_ID + suffix))
    for suffix, size in [('.svg', 'scalable'), ('.png', '512x512')]:
        install(ROOT / 'assets' / ('appicon' + suffix),
                prefix / 'share/icons/hicolor' / size / 'apps' / (APP_ID + suffix))
    install(ROOT / 'LICENSE', prefix / 'share/licenses' / APP_ID / 'LICENSE')


def flatpak_bundle():
    """Finalize a local application tree and export an installable single-file Flatpak bundle."""
    build = WORK / 'flatpak'
    if build.exists():
        build.rename(WORK / ('flatpak-previous-' + str(time.time_ns())))
    run('flatpak', 'build-init', build, APP_ID, SDK, RUNTIME, BRANCH)
    payload(build / 'files')
    run('flatpak', 'build-finish', '--command=toniator-app', '--socket=wayland',
        '--socket=fallback-x11', '--share=ipc', '--device=dri', build)
    run('flatpak', 'build-export', '--disable-sandbox', WORK / 'repo', build, 'stable')
    bundle = DIST / 'Toniator-0.2.0-x86_64.flatpak'
    temporary = DIST / ('.Toniator-' + str(time.time_ns()) + '.flatpak')
    run('flatpak', 'build-bundle', WORK / 'repo', temporary, APP_ID, 'stable',
        '--runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo')
    temporary.replace(bundle)
    return bundle


def main():
    """Build the requested local format and record exact artifact/source/runtime provenance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-build', action='store_true', help='Reuse this packaging directory’s SDK binaries')
    args = parser.parse_args()
    if output('uname', '-m') != 'x86_64':
        raise SystemExit('This initial packaging recipe supports x86_64 only.')
    WORK.mkdir(parents=True, exist_ok=True)
    DIST.mkdir(exist_ok=True)
    if not args.skip_build:
        sdk_build()
    bundle = flatpak_bundle()
    record = {'version': '0.2.0', 'app_id': APP_ID,
              'base_commit': output('git', 'rev-parse', 'HEAD'),
              'packaging_diff': output('git', 'diff', '--', 'crates/toniator-app/src/main.rs',
                                        'crates/toniator-app/src/main_view_state.rs'),
              'rust': output('rustc', '--version'),
              'sdk_commit': output('flatpak', 'info', '--show-commit', SDK + '//' + BRANCH),
              'runtime_commit': output('flatpak', 'info', '--show-commit', RUNTIME + '//' + BRANCH),
              'artifact': bundle.name, 'sha256': hashlib.sha256(bundle.read_bytes()).hexdigest(),
              'icons': {name: hashlib.sha256((ROOT / 'assets' / name).read_bytes()).hexdigest()
                        for name in ['appicon.svg', 'appicon.png']}}
    (DIST / 'build-info.json').write_text(json.dumps(record, indent=2) + '\n')


if __name__ == '__main__':
    main()
