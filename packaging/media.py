#!/usr/bin/env python3
"""Build pinned software media tools in the GNOME SDK, with corresponding source.

Only target/packaging/media is generated. Downloads are hash checked on the host;
all compilation is offline in the existing SDK sandbox. No host codecs are copied.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import time
import urllib.request

from build import APP_ID, BRANCH, ROOT, SDK, WORK, install, output, run

MEDIA = WORK / 'media'
PREFIX = MEDIA / 'prefix'
SOURCES = json.loads((ROOT / 'packaging/media-sources.json').read_text())


def prepare_sources():
    """Verify pinned archives and extract fresh roots so prior local edits cannot enter a build."""
    source_root = MEDIA / 'sources' / ('build-' + str(time.time_ns()))
    source_root.mkdir(parents=True)
    for item in SOURCES:
        archive = MEDIA / 'downloads' / item['name']
        archive.parent.mkdir(parents=True, exist_ok=True)
        if not archive.exists():
            temporary = archive.with_suffix(archive.suffix + '.download')
            urllib.request.urlretrieve(item['url'], temporary)
            temporary.replace(archive)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != item['sha256']:
            raise SystemExit('Media source checksum mismatch: ' + item['name'])
        with tarfile.open(archive) as source:
            if any(member.name.split('/')[0] != item['directory'] for member in source):
                raise SystemExit('Unexpected source archive root: ' + item['name'])
            source.extractall(source_root, filter='data')
    return source_root


def sdk_step(label, arguments, directory, environment=None):
    """Run an offline SDK command and retain its exact arguments and combined log."""
    directory.mkdir(parents=True, exist_ok=True)
    logs = MEDIA / 'logs'
    logs.mkdir(exist_ok=True)
    command = ['flatpak', 'build', '--unshare=network', f'--filesystem={ROOT}',
               f'--build-dir={directory}', f'--env=PKG_CONFIG_PATH={PREFIX}/lib/pkgconfig']
    for name, value in (environment or {}).items():
        command.append(f'--env={name}={value}')
    command += [str(WORK / 'sdk'), *map(str, arguments)]
    (logs / (label + '.command.json')).write_text(json.dumps(command, indent=2) + '\n')
    print('Media build:', label, flush=True)
    with (logs / (label + '.log')).open('w') as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        print((logs / (label + '.log')).read_text()[-12000:], flush=True)
        raise SystemExit('Media build failed: ' + label)


def build_tools(jobs):
    """Compile static codec libraries and FFmpeg executables without GPL/nonfree options."""
    sources = prepare_sources()
    if not (WORK / 'sdk/metadata').exists():
        run('flatpak', 'build-init', WORK / 'sdk', APP_ID, SDK,
            'org.gnome.Platform', BRANCH)
    builds = MEDIA / 'build' / sources.name
    svt = builds / 'svt'
    sdk_step('svt-configure', ['cmake', '-S', sources / 'SVT-AV1-v3.1.2', '-B', svt,
             f'-DCMAKE_INSTALL_PREFIX={PREFIX}', '-DCMAKE_INSTALL_LIBDIR=lib',
             '-DCMAKE_BUILD_TYPE=Release', '-DBUILD_SHARED_LIBS=OFF', '-DBUILD_APPS=OFF',
             '-DBUILD_TESTING=OFF', '-DREPRODUCIBLE_BUILDS=ON', '-DNATIVE=OFF',
             '-DSVT_AV1_LTO=OFF'], builds)
    sdk_step('svt-build', ['cmake', '--build', svt, '--parallel', jobs], builds)
    sdk_step('svt-install', ['cmake', '--install', svt], builds)
    dav1d = builds / 'dav1d'
    setup = ['meson', 'setup', dav1d, sources / 'dav1d-1.5.1', f'--prefix={PREFIX}',
             '--libdir=lib', '--buildtype=release', '--default-library=static',
             '-Denable_tools=false', '-Denable_tests=false', '-Denable_examples=false',
             '-Denable_docs=false', '-Dxxhash_muxer=disabled']
    if (dav1d / 'meson-private/coredata.dat').exists():
        setup.append('--reconfigure')
    sdk_step('dav1d-configure', setup, builds)
    sdk_step('dav1d-build', ['ninja', '-C', dav1d, '-j', jobs], builds)
    sdk_step('dav1d-install', ['meson', 'install', '-C', dav1d], builds)
    vpx = builds / 'vpx'
    sdk_step('vpx-configure', [sources / 'libvpx-1.15.2/configure', f'--prefix={PREFIX}',
             '--target=x86_64-linux-gcc', '--as=nasm', '--enable-pic', '--enable-static',
             '--disable-shared', '--enable-vp8-decoder', '--enable-vp9-decoder',
             '--disable-vp8-encoder', '--disable-vp9-encoder', '--enable-vp9-highbitdepth',
             '--disable-examples', '--disable-tools', '--disable-docs', '--disable-unit-tests',
             '--disable-webm-io', '--disable-libyuv'], vpx)
    sdk_step('vpx-build', ['make', '-j', jobs], vpx)
    sdk_step('vpx-install', ['make', 'install'], vpx)
    ffmpeg = builds / 'ffmpeg'
    sdk_step('ffmpeg-configure', [sources / 'ffmpeg-8.1.2/configure', f'--prefix={PREFIX}',
             '--disable-autodetect', '--disable-shared', '--enable-static', '--disable-gpl',
             '--disable-nonfree', '--disable-version3', '--disable-network', '--disable-hwaccels',
             '--disable-devices', '--enable-indev=lavfi', '--enable-protocol=file,pipe',
             '--disable-ffplay', '--disable-doc', '--enable-pthreads', '--enable-libsvtav1',
             '--enable-libdav1d', '--enable-libvpx', '--enable-zlib', '--enable-bzlib',
             '--enable-lzma', '--pkg-config-flags=--static', f'--extra-cflags=-I{PREFIX}/include',
             f'--extra-ldflags=-L{PREFIX}/lib', '--disable-encoders',
             '--enable-encoder=ffv1,rawvideo,png,wrapped_avframe,libsvtav1'], ffmpeg)
    sdk_step('ffmpeg-build', ['make', '-j', jobs, 'ffmpeg', 'ffprobe'], ffmpeg)
    for name in ['ffmpeg', 'ffprobe']:
        install(ffmpeg / name, PREFIX / 'bin' / name)
    record = {'sources': SOURCES, 'sdk_commit': output('flatpak', 'info', '--show-commit',
               SDK + '//' + BRANCH), 'recipe_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    for name in ['ffmpeg', 'ffprobe']:
        sdk_step(name + '-dependencies', ['ldd', PREFIX / 'bin' / name], builds)
        needed = (MEDIA / 'logs' / (name + '-dependencies.log')).read_text()
        if any(value in needed.lower() for value in ['not found', 'libavcodec', 'libavformat',
                   'libavutil', 'libsvt', 'libdav1d', 'libvpx', 'libx264', 'libx265', 'libpostproc']):
            raise SystemExit('Unexpected dynamic media dependency: ' + name)
        record[name + '_sha256'] = hashlib.sha256((PREFIX / 'bin' / name).read_bytes()).hexdigest()
    sdk_step('ffmpeg-version', [PREFIX / 'bin/ffmpeg', '-version'], builds)
    (MEDIA / 'build-info.json').write_text(json.dumps(record, indent=2) + '\n')


def install_tools(destination):
    """Install verified private executables, notices, original source and reproducible recipe."""
    record = json.loads((MEDIA / 'build-info.json').read_text())
    if record['sources'] != SOURCES or record['recipe_sha256'] != hashlib.sha256(Path(__file__).read_bytes()).hexdigest():
        raise SystemExit('Media recipe changed; rebuild packaging/media.py first.')
    for name in ['ffmpeg', 'ffprobe']:
        binary = PREFIX / 'bin' / name
        if hashlib.sha256(binary.read_bytes()).hexdigest() != record[name + '_sha256']:
            raise SystemExit('Media executable changed; rebuild first: ' + name)
        install(binary, destination / 'libexec/toniator-media' / name)
    share = destination / 'share/toniator-media'
    for item in SOURCES:
        archive = MEDIA / 'downloads' / item['name']
        if hashlib.sha256(archive.read_bytes()).hexdigest() != item['sha256']:
            raise SystemExit('Media source archive changed: ' + item['name'])
        install(archive, share / 'sources' / item['name'])
    for name in ['media.py', 'media-sources.json', 'MEDIA-NOTICES.md', 'build.py']:
        install(ROOT / 'packaging' / name, share / 'recipe' / name)
    install(MEDIA / 'build-info.json', share / 'build-info.json')
    shutil.copytree(MEDIA / 'logs', share / 'logs', dirs_exist_ok=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jobs', type=int, default=min(8, os.cpu_count() or 1))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error('--jobs must be positive')
    build_tools(str(args.jobs))
