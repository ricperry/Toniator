Toniator 0.3.2 - native Windows x64 development checkpoint (2026-09-27)

This unsigned prerelease contains the verified DEBUG development GUI and CLI.
It is not an installer or a new optimized build. Extract the entire ZIP to a
writable folder on Windows 10/11 x64. Paths containing spaces and Unicode work.
No Rust, GTK SDK, MSYS2, Python, WSL, or machine-wide PATH change is required.

Double-click Toniator.vbs to open the GUI quietly. A .toniator project can also
be dragged onto this launcher. Windows Script Host must be enabled to use it.
From an existing terminal, run .\Toniator-CLI.cmd --help for CLI usage.
The launchers set only their child/process environment. Normal application
documents, presets, and preferences still use Toniator's Windows user folders.

Video import, sequences requiring media tools, and video export need FFmpeg.
FFmpeg is not redistributed in this ZIP. To download the exact verified upstream
Gyan FFmpeg 8.1.2 full build into this extracted folder, run this explicit step:

  powershell -NoProfile -ExecutionPolicy Bypass -File .\Setup-Media.ps1

Setup verifies the pinned upstream SHA-256 before extraction. It uses installed
7-Zip when available; otherwise it downloads and verifies official standalone
7zr 26.03. Only ffmpeg.exe, ffprobe.exe and their original upstream notices are
extracted. No installer runs. Setup needs an internet connection and roughly
600 MB of free space. No download happens when Toniator launches. An existing
FFmpeg on the caller's PATH can also be used. Setup never overwrites existing
media; move it yourself before rerunning. -ArchivePath permits a retained,
hash-matching Gyan .7z archive. See Setup-Media.ps1 for exact source URLs/hashes.

Known limits: native GUI sequence-video export remains pending verification.
Fine Feature size 0.5 previews may take around 40 seconds and progress currently
advances per output, sometimes with roughly ten-second gaps. GTK UI Automation
Value.SetValue did not edit the tested entry; keyboard editing works. The visible
Stopped state and UI Automation progress text differ after video cancellation.
An automated shutdown sequence involving the main and welcome windows caused
an AccessKit unknown-child panic in one verification run. Closing Welcome through
its own Close action, then closing the resulting Untitled main window, exits
normally with code 0. Full stability is not claimed.
Hosted CI is prepared but has not run. Signing and installer packaging are absent.

PACKAGE-MANIFEST.json records executable hashes and checkpoint/source metadata.
THIRD-PARTY-NOTICES.md and notices/ contain bundled licenses and attribution.
Download the matching Toniator-0.3.2-windows-x64-dev-20260927-sources.zip from the
same release for original source archives, dependency sources, build recipes,
and the exact Toniator checkpoint. See SOURCES.md. Keep these notices with copies.
