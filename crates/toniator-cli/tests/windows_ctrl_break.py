"""Verifies cooperative CLI Ctrl+Break with a hidden native console/process group."""
import ctypes
import json
import pathlib
import subprocess
import sys
import time

executable, project, output, evidence = sys.argv[1:]
root = pathlib.Path(evidence)
root.mkdir(parents=True, exist_ok=True)
startup = subprocess.STARTUPINFO()
startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
startup.wShowWindow = subprocess.SW_HIDE
kernel = ctypes.WinDLL("kernel32", use_last_error=True)
kernel.AttachConsole.argtypes = [ctypes.c_ulong]
kernel.AttachConsole.restype = ctypes.c_int
kernel.GenerateConsoleCtrlEvent.argtypes = [ctypes.c_ulong, ctypes.c_ulong]
kernel.GenerateConsoleCtrlEvent.restype = ctypes.c_int
with (root / "stdout.log").open("wb") as stdout, (root / "stderr.log").open("wb") as stderr:
    child = subprocess.Popen(
        [executable, "render", "--input", project, "--output", output],
        stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
        creationflags=subprocess.CREATE_NEW_CONSOLE | subprocess.CREATE_NEW_PROCESS_GROUP,
        startupinfo=startup,
    )
    try:
        deadline = time.monotonic() + 20
        while "Preparing frame export" not in (root / "stderr.log").read_text(errors="replace"):
            if child.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("CLI did not enter its cancellable export: " + (root / "stderr.log").read_text(errors="replace"))
            time.sleep(0.02)
        # Detach the test runner's inherited console before joining the hidden CLI console.
        kernel.FreeConsole()
        if not kernel.AttachConsole(child.pid):
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            if not kernel.GenerateConsoleCtrlEvent(1, child.pid):
                raise ctypes.WinError(ctypes.get_last_error())
        finally:
            kernel.FreeConsole()
        code = child.wait(timeout=10)
        diagnostic = (root / "stderr.log").read_text(errors="replace")
        assert code != 0 and "export.cancelled" in diagnostic, diagnostic
        manifest = pathlib.Path(output).parent / "manifest.json"
        if manifest.exists():
            assert json.loads(manifest.read_text())["complete"] is False
        result = {"pid": child.pid, "exit_code": code, "ctrl_break": True, "cooperative_cancel": True}
        (root / "result.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result))
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()
