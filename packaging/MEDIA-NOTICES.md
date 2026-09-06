# Bundled software media tools

Toniator invokes its own `libexec/toniator-media/ffmpeg` and `ffprobe` processes.
Packaged builds do not search for host codecs or fall back to a host FFmpeg.
The same tools are included in AppImage and Flatpak; no additional sandbox
permissions are required for their invocation.

The pinned source archives, exact recipe, build logs and SDK commit accompany
the executables under `share/toniator-media/`. Original license texts and notices
are included in those unmodified source archives:

- FFmpeg 8.1.2: LGPL 2.1 or later, configured without GPL, version-3 or nonfree
  components. See `COPYING.LGPLv2.1` and `LICENSE.md` in its source archive and
  [FFmpeg's legal information](https://ffmpeg.org/legal.html).
- SVT-AV1 3.1.2: BSD 3-clause Clear license and Alliance for Open Media patent
  license; see its `LICENSE.md`, `LICENSE-BSD2.md` and `PATENTS.md`.
- dav1d 1.5.1: BSD 2-clause license; see `COPYING` in its archive.
- libvpx 1.15.2: BSD-style license and WebM patent grant; see `LICENSE` and `PATENTS`.

The recipe statically links these media libraries into the two executables.
SDK compression/system libraries remain dynamic. No codecs-extra extension,
x264 or x265 library is bundled. Static linkage does not remove the source and
license obligations; the supplied original sources and recipe allow rebuilding
and replacing both media executables independently of Toniator.

Run `python packaging/media.py` from the Toniator source checkout to reproduce
the media build in the installed GNOME 50 SDK. Downloads are hash pinned;
compilation is offline. Run `python packaging/build.py` afterward to compile
Toniator with package-relative tool selection and build the Flatpak, then
`python packaging/appimage.py` for AppImage. These commands create local
artifacts; they do not publish or install a release.
