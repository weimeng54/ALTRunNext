# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

"""Exercise the visible install and uninstall choices on the hosted test desktop."""
import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import subprocess
import time


def start_installer(executable, arguments):
    # The uninstaller leaves its original executable before the temporary
    # worker deletes it. Start-Process -Wait monitors the whole process tree.
    return subprocess.Popen(
        ["pwsh", "-NoProfile", "-Command",
         "$ErrorActionPreference='Stop'; $options=@{}; if ($env:TEST_INSTALLER_ARGS) {"
         "$options.ArgumentList=$env:TEST_INSTALLER_ARGS }; $p=Start-Process -FilePath $env:TEST_INSTALLER "
         "@options -PassThru -Wait; exit $p.ExitCode"],
        env={**os.environ, "TEST_INSTALLER": str(executable), "TEST_INSTALLER_ARGS": arguments if isinstance(arguments, str) else subprocess.list2cmdline(arguments)},
    )


def capture_wizard(dialog, filename):
    bounds = wintypes.RECT()
    if not ctypes.windll.user32.GetWindowRect(dialog, ctypes.byref(bounds)):
        raise RuntimeError("Could not measure the installer window")
    output = Path("target/ui") / filename
    output.parent.mkdir(parents=True, exist_ok=True)
    command = """
        $ErrorActionPreference='Stop'
        Add-Type -AssemblyName System.Drawing
        Add-Type -MemberDefinition '[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr w, IntPtr dc, uint flags);' -Name Capture -Namespace Native
        $image=[Drawing.Bitmap]::new([int]$env:SHOT_WIDTH,[int]$env:SHOT_HEIGHT)
        $g=[Drawing.Graphics]::FromImage($image)
        $dc=$g.GetHdc()
        try {
            if (![Native.Capture]::PrintWindow([IntPtr]::new([long]$env:SHOT_HWND),$dc,2)) { throw 'PrintWindow failed' }
        } finally { $g.ReleaseHdc($dc); $g.Dispose() }
        try { $image.Save($env:SHOT_PATH,[Drawing.Imaging.ImageFormat]::Png) } finally { $image.Dispose() }
    """
    subprocess.run(["pwsh", "-NoProfile", "-Command", command], check=True, env={
        **os.environ, "SHOT_WIDTH": str(bounds.right - bounds.left), "SHOT_HEIGHT": str(bounds.bottom - bounds.top),
        "SHOT_HWND": str(dialog.value), "SHOT_PATH": str(output.resolve()),
    })


def exercise_wizard(executable, *, uninstall=False):
    user = ctypes.windll.user32
    user.GetDlgItem.restype = wintypes.HWND
    enumerate_proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def windows(parent=None):
        found = []

        @enumerate_proc
        def collect(handle, _):
            found.append(wintypes.HWND(handle))
            return True

        if parent:
            user.EnumChildWindows(parent, collect, 0)
        else:
            user.EnumWindows(collect, 0)
        return found

    def caption(handle):
        buffer = ctypes.create_unicode_buffer(512)
        result = ctypes.c_size_t()
        user.SendMessageTimeoutW(handle, 0x000D, len(buffer), ctypes.byref(buffer), 2, 250, ctypes.byref(result))
        return buffer.value

    process = start_installer(executable, [])
    deadline = time.monotonic() + 65
    clicked = set()
    verified = False
    while process.poll() is None:
        if time.monotonic() > deadline:
            raise RuntimeError(f"Wizard timed out after clicking {clicked}")
        for dialog in windows():
            title = caption(dialog)
            if "altrunnext" not in title.lower():
                continue
            if not user.IsWindowVisible(dialog):
                continue
            controls = {caption(child).replace("&", ""): child for child in windows(dialog) if user.IsWindowVisible(child)}
            if not uninstall and not verified and str(Path(os.environ["ProgramFiles"]) / "ALTRunNext") in controls:
                capture_wizard(dialog, "nsis-install.png")
                verified = True
            check = controls.get("Also delete this user's installed launch items, settings and history")
            if check and not verified:
                if user.SendMessageW(check, 0xF0, 0, 0) != 0:  # BM_GETCHECK
                    raise RuntimeError("Purge checkbox was selected by default")
                capture_wizard(dialog, "nsis-uninstall.png")
                user.SendMessageW(check, 0xF5, 0, 0)  # BM_CLICK
                if user.SendMessageW(check, 0xF0, 0, 0) != 1:
                    raise RuntimeError("Could not select purge checkbox")
                verified = True
            for label in ("Install", "Uninstall", "Next >", "Close", "OK"):
                button = controls.get(label)
                if button and user.IsWindowEnabled(button) and (dialog.value, label) not in clicked:
                    if not verified:
                        continue
                    clicked.add((dialog.value, label))
                    user.PostMessageW(button, 0xF5, 0, 0)
                    break
        time.sleep(0.12)
    if process.returncode or not verified:
        raise RuntimeError(f"Interactive wizard failed (exit {process.returncode}, selection verified {verified})")
