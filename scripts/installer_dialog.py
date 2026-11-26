# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

"""Drive the English uninstall wizard on the disposable test desktop."""
import ctypes
from ctypes import wintypes
import os
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


def uninstall_with_checkbox(uninstaller):
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
        buffer = ctypes.create_unicode_buffer(256)
        user.GetWindowTextW(handle, buffer, len(buffer))
        return buffer.value

    process = start_installer(uninstaller, [])
    deadline = time.monotonic() + 65
    clicked = set()
    verified = False
    while process.poll() is None:
        if time.monotonic() > deadline:
            raise RuntimeError("Uninstall dialogs timed out")
        for dialog in windows():
            title = caption(dialog)
            if "altrunnext" not in title.lower() or "uninstall" not in title.lower():
                continue
            if not user.IsWindowVisible(dialog):
                continue
            controls = {caption(child).replace("&", ""): child for child in windows(dialog)}
            check = controls.get("Also delete this user's installed launch items, settings and history")
            if check and not verified:
                if user.SendMessageW(check, 0xF0, 0, 0) != 0:  # BM_GETCHECK
                    raise RuntimeError("Purge checkbox was selected by default")
                user.SendMessageW(check, 0xF5, 0, 0)  # BM_CLICK
                if user.SendMessageW(check, 0xF0, 0, 0) != 1:
                    raise RuntimeError("Could not select purge checkbox")
                verified = True
            for label in ("Uninstall", "Next >", "Close", "OK"):
                button = controls.get(label)
                if button and (dialog.value, label) not in clicked:
                    if label == "Uninstall" and not verified:
                        continue
                    clicked.add((dialog.value, label))
                    user.PostMessageW(button, 0xF5, 0, 0)
                    break
        time.sleep(0.12)
    if process.returncode or not verified:
        raise RuntimeError(f"Interactive uninstall failed (exit {process.returncode}, checkbox {verified})")
