#!/usr/bin/env python3
"""Bundle the SDK-built release and its GTK dependencies into a local x86_64 AppImage."""
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import urllib.request
import time
import sys

from build import APP_ID, BRANCH, DIST, ROOT, SDK, WORK, install, output, payload, run

TOOL_URL = 'https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage'
TOOL_SHA256 = 'ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0'


def main():
    """Stage runtime dependencies without host glibc/graphics drivers and emit an AppImage."""
    sdk_files = Path(output('flatpak', 'info', '--show-location', SDK + '//' + BRANCH)) / 'files'
    appdir = WORK / 'Toniator.AppDir'
    if appdir.exists():
        appdir.rename(WORK / ('Toniator.AppDir-previous-' + str(time.time_ns())))
    payload(appdir / 'usr')
    private = appdir / 'usr/lib'
    private.mkdir(parents=True, exist_ok=True)
    binaries = [str(WORK / 'cargo/release' / name) for name in ['toniator-app', 'toniator']]
    # GTK's PNG/SVG resources and icons may use Glycin loaders at runtime.
    for name in ['glycin-image-rs', 'glycin-svg']:
        source = sdk_files / 'libexec/glycin-loaders/2+' / name
        install(source, appdir / 'usr/libexec/glycin-loaders/2+' / name)
        binaries.append('/usr/libexec/glycin-loaders/2+/' + name)
    install(sdk_files / 'bin/bwrap', appdir / 'usr/bin/bwrap')
    binaries.append('/usr/bin/bwrap')
    dependencies = output('flatpak', 'build', f'--filesystem={ROOT}:ro', WORK / 'sdk', 'ldd', *binaries)
    (WORK / 'appimage-dependencies.txt').write_text(dependencies + '\n')
    if 'not found' in dependencies:
        raise SystemExit('An SDK dependency is missing; see appimage-dependencies.txt.')
    system_libraries = re.compile(r'^(ld-linux|lib(c|m|dl|pthread|rt|resolv|nss_[\w]+)\.so|lib(GL|EGL|GLX|GLdispatch|OpenGL|vulkan|drm|gbm))')
    for path in sorted(set(re.findall(r'=> (/usr/\S+)', dependencies))):
        name = Path(path).name
        if not system_libraries.match(name):
            install(sdk_files / path.removeprefix('/usr/'), private / name)
    # Dereference absolute /usr symlinks inside the SDK, never against the host.
    resources = WORK / 'appimage-gtk-data.tar'
    run('flatpak', 'build', f'--filesystem={WORK}', WORK / 'sdk', 'tar', '-chf', resources,
        '-C', '/usr/share', 'glib-2.0/schemas', 'icons/Adwaita', 'icons/hicolor', 'mime', 'licenses')
    run('tar', '-xf', resources, '-C', appdir / 'usr/share')
    configs = appdir / 'usr/share/glycin-loaders/2+/conf.d'
    for name in ['glycin-image-rs', 'glycin-svg']:
        install(sdk_files / 'share/glycin-loaders/2+/conf.d' / (name + '.conf'), configs / (name + '.conf'))
    install(ROOT / 'packaging' / (APP_ID + '.desktop'), appdir / (APP_ID + '.desktop'))
    install(ROOT / 'assets/appicon.png', appdir / (APP_ID + '.png'))
    (appdir / '.DirIcon').symlink_to(APP_ID + '.png')
    install(ROOT / 'packaging/AppRun', appdir / 'AppRun')
    (appdir / 'AppRun').chmod(0o755)
    tool = WORK / 'appimagetool-x86_64.AppImage'
    if not tool.exists():
        urllib.request.urlretrieve(TOOL_URL, tool)
    if hashlib.sha256(tool.read_bytes()).hexdigest() != TOOL_SHA256:
        raise SystemExit('appimagetool checksum mismatch.')
    tool.chmod(0o755)
    destination = DIST / 'Toniator-0.2.0-x86_64.AppImage'
    temporary = DIST / ('.Toniator-' + str(time.time_ns()) + '.AppImage')
    environment = dict(os.environ, ARCH='x86_64', APPIMAGE_EXTRACT_AND_RUN='1')
    run(tool, '--no-appstream', appdir, temporary, env=environment)
    temporary.replace(destination)
    run(sys.executable, ROOT / 'packaging/check-artifacts.py')
    print('AppImage:', destination)


if __name__ == '__main__':
    main()
