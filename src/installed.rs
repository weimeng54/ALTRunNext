// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use std::path::{Path, PathBuf};

fn installed_profile(exe: &Path) -> std::io::Result<Option<PathBuf>> {
    if !exe.with_file_name("ALTRunNext.installed").try_exists()? {
        return Ok(None);
    }
    let local = std::env::var_os("LOCALAPPDATA")
        .filter(|path| Path::new(path).is_absolute())
        .ok_or_else(|| std::io::Error::other("LOCALAPPDATA is unavailable"))?;
    let directory = PathBuf::from(local).join("ALTRunNext");
    Ok(Some(directory.join("shortcuts.tsv")))
}

pub(crate) fn is_installed_profile(path: &Path) -> bool {
    std::env::current_exe()
        .and_then(|exe| installed_profile(&exe))
        .is_ok_and(|profile| profile.as_deref() == Some(path))
}

pub(crate) fn default_data_path() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let Some(profile) = installed_profile(&exe)? else {
        return Ok(exe.with_file_name("shortcuts.tsv"));
    };
    Ok(profile)
}

pub(crate) fn configure_integrations(options: u8) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let data = installed_profile(&exe)
        .map_err(|error| error.to_string())?
        .ok_or("Expected an installed copy")?;
    let integration = altrun_probe::integration::Integration::for_data(&data)?;
    integration.apply(options & 1 != 0, options & 2 != 0)
}
