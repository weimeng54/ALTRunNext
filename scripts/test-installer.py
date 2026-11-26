# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

"""Exercise both distributions on a disposable Windows runner."""
import ctypes
from ctypes import wintypes
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import winreg
from zipfile import ZipFile
from installer_dialog import start_installer, exercise_wizard


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


def create_shell_entries(directory, data, name):
    exe = directory / "ALTRunNext.exe"
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, run_key) as handle:
        winreg.SetValueEx(handle, name, 0, winreg.REG_SZ, f'"{exe}" --enable-launch --data "{data}"')
    command = """
        $ErrorActionPreference = 'Stop'
        $path = Join-Path ([Environment]::GetFolderPath('SendTo')) 'ALTRunNext.lnk'
        $link = (New-Object -ComObject WScript.Shell).CreateShortcut($path)
        $link.TargetPath = $env:TEST_EXE
        $link.Arguments = '--enable-launch --data "' + $env:TEST_DATA + '" --add'
        $link.Save()
        $path
    """
    path = subprocess.check_output(["pwsh", "-NoProfile", "-Command", command],
                                   env={**os.environ, "TEST_EXE": str(exe), "TEST_DATA": str(data)}, text=True)
    return Path(path.strip())


def registered_run(name):
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, run_key) as handle:
        try:
            return winreg.QueryValueEx(handle, name)[0]
        except FileNotFoundError:
            return None


def verify_upgrade_rollback(setup, installed):
    before = hashes(installed)
    entry = registration()
    kernel = ctypes.windll.kernel32
    kernel.CreateFileW.restype = wintypes.HANDLE
    file = kernel.CreateFileW(str(installed / "languages/fr.json"), 0x80000000, 1, None, 3, 0, None)
    if file == wintypes.HANDLE(-1).value:
        raise RuntimeError("Could not lock the synthetic language file")
    try:
        install(setup, "locked-resource", expect_failure=True)
    finally:
        kernel.CloseHandle(wintypes.HANDLE(file))
    require(hashes(installed) == before and registration() == entry,
            "A locked resource rolls back the upgrade and retains the previous uninstaller")


def install(exe, log, *options, expect_failure=False):
    # NSIS parses the entire unquoted tail after /D= as the directory.
    result = start_installer(exe, " ".join(("/S", *options)))
    result.wait(120)
    if (result.returncode != 0) != expect_failure:
        raise RuntimeError(f"{log} returned {result.returncode}")


def launch(directory, while_running=None, arguments=()):
    user = ctypes.windll.user32
    user.FindWindowW.restype = wintypes.HWND
    process = subprocess.Popen([directory / "ALTRunNext.exe", *arguments, "--safe-mode", "--show"])
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


def shell_state(directory, profile):
    command = """
        $shell = New-Object -ComObject WScript.Shell
        $result = @{}
        foreach ($folder in 'CommonPrograms','CommonDesktopDirectory','SendTo') {
            $path = Join-Path ([Environment]::GetFolderPath($folder)) 'ALTRunNext.lnk'
            $result[$folder] = if (Test-Path -LiteralPath $path) {
                $link = $shell.CreateShortcut($path)
                @($link.TargetPath, $link.Arguments)
            }
        }
        $result | ConvertTo-Json -Compress
    """
    state = json.loads(subprocess.check_output(["pwsh", "-NoProfile", "-Command", command], text=True))
    exe = str(directory / "ALTRunNext.exe")
    data = profile / "shortcuts.tsv"
    run_value = f'"{exe}" --enable-launch --data "{data}"'
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, run_key) as key:
        runs = [winreg.EnumValue(key, i)[1] for i in range(winreg.QueryInfoKey(key)[1])]
    return (state["CommonPrograms"] == [exe, "--show"], state["CommonDesktopDirectory"] == [exe, "--show"],
            run_value in runs, state["SendTo"] == [exe, run_value.split('" ', 1)[1] + " --add"])


def check_post_install_launch(directory, expected):
    # A hosted service session may not expose the desktop user's HWND. Verify
    # the actual launched image by full path and stop only the test instance.
    command = """
        $until = [DateTime]::UtcNow.AddSeconds(8)
        do {
            $app = Get-Process ALTRunNext -ErrorAction SilentlyContinue | Where-Object Path -eq $env:TEST_EXE
            if ($app -or $env:EXPECT_OPEN -eq '0') { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $until)
        if ($app) { $app | Stop-Process -Force; $app | Wait-Process; exit 10 }
        exit 0
    """
    result = subprocess.run(["pwsh", "-NoProfile", "-Command", command], env={
        **os.environ, "TEST_EXE": str(directory / "ALTRunNext.exe"), "EXPECT_OPEN": str(int(expected)),
    })
    require(result.returncode == (10 if expected else 0), f"Post-install launch matched selection ({expected})")


if __name__ == "__main__":
    require(os.getenv("GITHUB_ACTIONS") == "true" and os.getenv("RUNNER_ENVIRONMENT") == "github-hosted",
            "Running on a disposable hosted runner")
    setup, version = Path(sys.argv[1]).resolve(), sys.argv[2]
    run_key = r"Software\Microsoft\Windows\CurrentVersion\Run"
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
    install(setup, "fresh-silent")
    default_dir = Path(registration()["InstallLocation"])
    require(default_dir == Path(os.environ["ProgramFiles"]) / "ALTRunNext", "First install uses Program Files")
    require(shell_state(default_dir, profile) == (False, False, True, True), "Fresh install enabled sign-in and Send to without creating shortcuts")
    check_post_install_launch(default_dir, False)
    launch(default_dir, arguments=("--enable-launch", "--data", str(profile / "shortcuts.tsv")))
    require((profile / "shortcuts.tsv").stat().st_size > 0, "First sign-in launch initialized the installed profile")
    install(default_dir / "Uninstall.exe", "default-uninstall")
    require(not registration(), "Default installation unregistered")
    require(shell_state(default_dir, profile) == (False,) * 4, "Uninstall removed all selected integrations")
    exercise_wizard(setup)
    check_post_install_launch(default_dir, True)
    install(default_dir / "Uninstall.exe", "default-wizard-uninstall")
    exercise_wizard(setup, choices=(True, True, False, False, False))
    check_post_install_launch(default_dir, False)
    require(shell_state(default_dir, profile) == (True, True, False, False), "Custom first-install choices took effect")
    custom_profile = hashes(profile)
    exercise_wizard(setup, upgrade=True)
    check_post_install_launch(default_dir, True)
    require(hashes(profile) == custom_profile, "Interactive upgrade preserved every existing profile file")
    require(shell_state(default_dir, profile) == (True, True, False, False), "Interactive upgrade preserved custom integrations")
    install(default_dir / "Uninstall.exe", "custom-uninstall")
    install(old_setup, "protect-portable", f"/D={portable}", expect_failure=True)
    # Build the legacy trial from its immutable source so migration remains
    # reproducible after its short-lived CI artifact expires.
    legacy_commit = "5cc43c4a83ed2923330a657a86ff773359390065"
    subprocess.run(["git", "fetch", "--depth=1", "origin", legacy_commit], check=True)
    legacy_root = work / "Legacy installer sources"
    archive = subprocess.check_output(["git", "archive", "--format=zip", legacy_commit,
                                      "installer", "scripts/build-installer.ps1", "assets/app.ico"])
    with ZipFile(io.BytesIO(archive)) as sources:
        sources.extractall(legacy_root)
    legacy_setup = work / "Legacy.exe"
    subprocess.run(["pwsh", "-NoProfile", "-File", legacy_root / "scripts/build-installer.ps1",
                    "-Payload", old, "-Version", "0.0.1", "-Output", legacy_setup], check=True)
    legacy = start_installer(legacy_setup, ["/VERYSILENT", "/SUPPRESSMSGBOXES", f"/DIR={installed}"])
    require(legacy.wait(120) == 0 and (installed / "unins000.exe").exists(), "Installed the actual legacy Inno package")

    launch(installed, lambda: install(setup, "busy-upgrade", expect_failure=True))
    items = profile / "shortcuts.tsv"
    require(items.is_file() and not (installed / items.name).exists(), "First run seeded the installed profile")
    items.write_text("trial\tTrial item\tnotepad.exe\t0\tnone\n", encoding="utf-8")
    (profile / "shortcuts.settings.ini").write_text("Lang=en\n", encoding="utf-8")
    (profile / "shortcuts.state.tsv").write_bytes(b"R\ttrial\nF\ttrial\t7\nH\ttrial\ttest argument\n")
    for file in (items, profile / "shortcuts.settings.ini", profile / "shortcuts.state.tsv"):
        shutil.copyfile(file, file.with_suffix(file.suffix + ".bak"))
    media = profile / "altrun-media-item-1-2-3"
    media.mkdir()
    (media / "notify.wav").write_bytes(b"fixture")
    (media / "background.bin").write_bytes(b"synthetic background")
    notes = profile / "my-notes.txt"
    notes.write_bytes(b"unrelated file")
    before = hashes(profile)
    install(old_setup, "upgrade-from-inno")
    require(hashes(profile) == before, "Inno migration preserved every profile file byte for byte")
    require(not list(installed.glob("unins000.*")), "Migration retired the Inno uninstaller")
    require(Path(registration()["UninstallString"].strip('"')).name == "Uninstall.exe", "NSIS owns the existing registration")
    verify_upgrade_rollback(setup, installed)
    require(hashes(profile) == before, "Failed upgrade preserved every profile file byte for byte")
    install(setup, "native-upgrade")
    entry = registration()
    require(Path(entry["InstallLocation"]) == installed and entry["DisplayVersion"] == version
            and entry["DisplayName"].startswith("ALTRunNext") and Path(entry["UninstallString"].strip('"')).is_file(),
            "Upgrade reused the previous directory and updated the Installed apps entry")
    require(hashes(profile) == before, "All profile files match byte for byte after upgrading")
    actual = hashes(installed)
    require(all(actual.get(name) == digest for name, digest in payload.items()), "All installed payload hashes match ZIP")
    launch(installed)
    uninstall = Path(entry["UninstallString"].strip('"'))
    own_run = "ALTRunNext-0000000000000001"
    link = create_shell_entries(installed, items, own_run)
    install(uninstall, "keep")
    require(not registered_run(own_run) and not link.exists(), "Installed startup and Send to entries were removed")
    require(not registration() and not installed.exists() and hashes(profile) == before,
            "Default uninstall removed program and kept all user data")
    items.write_bytes(b"")
    install(setup, "reinstall", f"/D={installed}")
    launch(installed)
    require(items.read_bytes() == b"", "Reinstall respected an intentionally empty list")
    launch(portable)
    require((portable / "shortcuts.tsv").stat().st_size > 0, "Portable launcher seeded its independent list")
    portable_before = hashes(portable)
    other_run = "ALTRunNext-0000000000000002"
    create_shell_entries(portable, portable / "shortcuts.tsv", other_run)
    original_link = link.read_bytes()
    original_run = registered_run(other_run)
    exercise_wizard(uninstall, uninstall=True)
    require(not registration() and not installed.exists(), "Opt-in uninstall removed program and registration")
    require(list(profile.iterdir()) == [notes] and notes.read_bytes() == b"unrelated file", "Only owned user files were cleared")
    require(hashes(portable) == portable_before, "Portable data survived uninstall")
    require(registered_run(other_run) == original_run and link.read_bytes() == original_link,
            "The portable copy retained its own startup and Send to entries")
    launch(portable)
