# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

"""Exercise both distributions on a disposable Windows runner."""
import ctypes
from ctypes import wintypes
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import winreg
from zipfile import ZipFile


def require(condition, message):
    if not condition:
        raise RuntimeError(message)
    print(message, flush=True)


def registration():
    key = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\ALTRunNext_is1"
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, key, 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as handle:
            return {field: winreg.QueryValueEx(handle, field)[0] for field in
                    ("InstallLocation", "DisplayName", "DisplayVersion", "UninstallString")}
    except FileNotFoundError:
        return {}


def hashes(directory):
    return {p.relative_to(directory): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in directory.rglob("*") if p.is_file()}


def install(exe, log, *options, expect_failure=False):
    logfile = work / (log + ".log")
    result = subprocess.run([exe, "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART",
                             f"/LOG={logfile}", *options], timeout=120)
    if (result.returncode != 0) != expect_failure:
        print(logfile.read_text(encoding="utf-8-sig", errors="replace")[-7000:], flush=True)
        raise RuntimeError(f"{log} returned {result.returncode}")


def launch(directory, while_running=None):
    user = ctypes.windll.user32
    user.FindWindowW.restype = wintypes.HWND
    process = subprocess.Popen([directory / "ALTRunNext.exe", "--safe-mode", "--show"])
    try:
        for _ in range(150):
            window = wintypes.HWND(user.FindWindowW("ALTRun.Native.PerformanceProbe", "ALTRunNext"))
            owner = wintypes.DWORD()
            user.GetWindowThreadProcessId(window, ctypes.byref(owner))
            if owner.value == process.pid:
                break
            time.sleep(0.1)
        else:
            raise RuntimeError("Launcher did not create its window")
        if while_running:
            while_running()
        user.PostMessageW(window, 0x111, 18002, 0)  # Exit from tray
        require(process.wait(15) == 0, f"{directory.name} launcher started and exited")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


if __name__ == "__main__":
    require(os.getenv("GITHUB_ACTIONS") == "true" and os.getenv("RUNNER_ENVIRONMENT") == "github-hosted",
            "Running on a disposable hosted runner")
    setup, version = Path(sys.argv[1]).resolve(), sys.argv[2]
    work = Path(os.environ["RUNNER_TEMP"]) / "ALTRunNext installation test"
    installed, portable, old = (work / n for n in ("Custom installation", "Portable", "Previous payload"))
    profile = Path(os.environ["LOCALAPPDATA"]) / "ALTRunNext"
    require(not registration() and not profile.exists(), "Clean machine profile")
    work.mkdir()
    with ZipFile("target/ALTRunNext-Win11-x64.zip") as archive:
        archive.extractall(portable)
    payload = hashes(portable)
    shutil.copytree(portable, old)
    # The previous fixture is still a valid PE, with a different hash and changelog.
    with (old / "ALTRunNext.exe").open("ab") as exe:
        exe.write(b"previous fixture")
    (old / "CHANGELOG.md").write_text("Previous test build", encoding="utf-8")
    old_setup = work / "Previous.exe"
    subprocess.run(["pwsh", "-NoProfile", "-File", "scripts/build-installer.ps1",
                    "-Payload", old, "-Version", "0.0.1", "-Output", old_setup], check=True)
    install(old_setup, "default-install")
    default_dir = Path(registration()["InstallLocation"])
    require(default_dir == Path(os.environ["ProgramFiles"]) / "ALTRunNext", "First install uses Program Files")
    install(default_dir / "unins000.exe", "default-uninstall")
    require(not registration(), "Default installation unregistered")
    install(old_setup, "protect-portable", f"/DIR={portable}", expect_failure=True)
    install(old_setup, "custom-install", f"/DIR={installed}")
    launch(installed, lambda: install(setup, "busy-upgrade", expect_failure=True))
    items = profile / "shortcuts.tsv"
    require(items.is_file() and not (installed / items.name).exists(), "First run seeded the installed profile")
    items.write_text("trial\tTrial item\tnotepad.exe\t0\tnone\n", encoding="utf-8")
    (profile / "shortcuts.settings.ini").write_text("Lang=en\n", encoding="utf-8")
    (profile / "shortcuts.state.tsv").write_bytes(b"")
    shutil.copyfile(items, profile / "shortcuts.tsv.bak")
    media = profile / "altrun-media-item-1-2-3"
    media.mkdir()
    (media / "notify.wav").write_bytes(b"fixture")
    notes = profile / "my-notes.txt"
    notes.write_bytes(b"unrelated file")
    before = hashes(profile)
    install(setup, "upgrade")
    entry = registration()
    require(Path(entry["InstallLocation"]) == installed and entry["DisplayVersion"] == version
            and entry["DisplayName"].startswith("ALTRunNext") and Path(entry["UninstallString"].strip('"')).is_file(),
            "Upgrade reused the previous directory and updated the Installed apps entry")
    require(hashes(profile) == before, "All profile files match byte for byte after upgrading")
    actual = hashes(installed)
    require(all(actual.get(name) == digest for name, digest in payload.items()), "All installed payload hashes match ZIP")
    launch(installed)
    uninstall = Path(entry["UninstallString"].strip('"'))
    install(uninstall, "keep")
    require(not registration() and not installed.exists() and hashes(profile) == before,
            "Default uninstall removed program and kept all user data")
    items.write_bytes(b"")
    install(setup, "reinstall", f"/DIR={installed}")
    launch(installed)
    require(items.read_bytes() == b"", "Reinstall respected an intentionally empty list")
    launch(portable)
    require((portable / "shortcuts.tsv").stat().st_size > 0, "Portable launcher seeded its independent list")
    portable_before = hashes(portable)
    install(uninstall, "purge", "/PURGEUSERDATA")
    require(not registration() and not installed.exists(), "Opt-in uninstall removed program and registration")
    require(list(profile.iterdir()) == [notes] and notes.read_bytes() == b"unrelated file", "Only owned user files were cleared")
    require(hashes(portable) == portable_before, "Portable data survived uninstall")
    launch(portable)
