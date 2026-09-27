# Toniator Windows development package

Toniator is GPL-3.0 software. Its original LICENSE is included under `notices/toniator/`.
No additional restriction on modification, reverse engineering, or replacement of the free libraries is imposed by this package.

The GTK stack is supplied as replaceable DLLs. It includes GTK, GLib/GObject/GIO,
GdkPixbuf, Pango, Cairo, Graphene, HarfBuzz, FriBidi, libepoxy, fontconfig,
FreeType, libffi, gettext/libintl, PCRE2, pixman, libpng, libjpeg-turbo, TIFF,
Expat, zlib, librsvg/libxml2/win-iconv, and dav1d. See `runtime-dependencies.json`
for the actual imports. GLib's native gdbus and gspawn helper executables are also
included for session activation and process launch; they use the same bundled
GLib runtime and licenses. See `notices/native/` for original component license,
copyright, attribution, and patent notices extracted from the retained sources.
The corresponding original source archives and the patched gvsbuild recipes are
provided in the matching source ZIP on the same release download page.

GNOME Project supplies the unmodified Adwaita icon artwork: https://www.gnome.org/.
The original dual LGPL/CC-BY-SA terms and hicolor notices are retained with the
other native component notices. Icons remain unmodified.

GTK statically incorporates AccessKit C 0.18 from commit
`0c52a8ce2357bbeb927f90dc9a1c19c8ec1bd2c3`. Its original MIT/Apache notices
are under `notices/accesskit-c/`; complete sources are in the companion ZIP.
The Rust dependency notices are under `notices/rust-crates/`, with corresponding
original `.crate` archives and manifests in the source ZIP. The Rust standard
library's copyright/license inventory is under `notices/rust-toolchain/`.

The two Microsoft Visual C++ runtime files are unmodified release redistributables
from Visual Studio 2022's x64 `Microsoft.VC143.CRT` redist directory. They are
Microsoft copyrighted proprietary redistributable components, not GPL software.
No debug CRT or Windows operating-system DLL is redistributed. Microsoft's
redistribution terms and list are described at:
https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files
https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution

FFmpeg and 7-Zip executables are **not bundled** in this package. Explicit media
setup retrieves upstream downloads directly for the recipient and retains the
FFmpeg upstream LICENSE and README. Gyan's Windows full build is GPLv3 and differs
from Toniator's Linux media build. See https://www.gyan.dev/ffmpeg/builds/ and
https://www.7-zip.org/download.html for those independent upstream distributions.
