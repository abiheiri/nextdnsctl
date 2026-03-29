use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const GITHUB_API: &str = "https://api.github.com/repos/abiheiri/nextdnsctl/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    url: String,
}

fn asset_name() -> Result<String, String> {
    let os = if cfg!(target_os = "macos") {
        "mac"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported operating system for self-update.".to_string());
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        return Err("Unsupported architecture for self-update.".to_string());
    };

    Ok(format!("nextdnsctl-{}-{}.tar.gz", os, arch))
}

pub fn run_update(current_version: &str) {
    let client = Client::new();

    // Fetch latest release
    let resp = match client
        .get(GITHUB_API)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "nextdnsctl")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Failed to check for updates: {}", e);
            std::process::exit(1);
        }
    };

    let status = resp.status();
    if status.as_u16() == 404 {
        eprintln!("Error: No releases found.");
        std::process::exit(1);
    }
    if !status.is_success() {
        eprintln!("Error: GitHub API error ({})", status);
        std::process::exit(1);
    }

    let release: Release = match resp.json() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Failed to parse release info: {}", e);
            std::process::exit(1);
        }
    };

    // Compare versions (strip leading 'v')
    let latest = release.tag_name.trim_start_matches('v');
    if latest == current_version {
        eprintln!("Already up to date (v{}).", current_version);
        return;
    }

    eprintln!("Updating v{} -> v{} ...", current_version, latest);

    // Find the matching asset
    let expected = match asset_name() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let asset = match release.assets.iter().find(|a| a.name == expected) {
        Some(a) => a,
        None => {
            eprintln!(
                "Error: No release asset found for this platform ({}).",
                expected
            );
            std::process::exit(1);
        }
    };

    // Download the asset
    let resp = match client
        .get(&asset.url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", "nextdnsctl")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Failed to download update: {}", e);
            std::process::exit(1);
        }
    };

    if !resp.status().is_success() {
        eprintln!("Error: Failed to download asset ({})", resp.status());
        std::process::exit(1);
    }

    let bytes = match resp.bytes() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: Failed to read download: {}", e);
            std::process::exit(1);
        }
    };

    // Extract the binary from the tar.gz
    let binary = match extract_binary_from_tar_gz(&bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: Failed to extract update: {}", e);
            std::process::exit(1);
        }
    };

    // Replace the current executable
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: Could not determine executable path: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = replace_executable(&current_exe, &binary) {
        eprintln!("Error: Failed to replace executable: {}", e);
        std::process::exit(1);
    }

    eprintln!("Updated to v{}.", latest);
}

fn extract_binary_from_tar_gz(data: &[u8]) -> Result<Vec<u8>, String> {
    let gz = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(gz);

    let entries = archive
        .entries()
        .map_err(|e| format!("Failed to read archive: {}", e))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to read entry path: {}", e))?
            .to_path_buf();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == "nextdnsctl" {
                let mut buf = Vec::new();
                entry
                    .read_to_end(&mut buf)
                    .map_err(|e| format!("Failed to read binary from archive: {}", e))?;
                return Ok(buf);
            }
        }
    }

    Err("Could not find nextdnsctl binary in the release archive.".to_string())
}

fn replace_executable(exe_path: &PathBuf, new_binary: &[u8]) -> Result<(), String> {
    let dir = exe_path
        .parent()
        .ok_or_else(|| "Could not determine executable directory.".to_string())?;

    let tmp_path = dir.join(".nextdnsctl-update.tmp");

    // Write new binary to temp file
    fs::write(&tmp_path, new_binary)
        .map_err(|e| format!("Failed to write temp file: {}", e))?;

    // Set executable permissions
    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("Failed to set permissions: {}", e))?;

    // Atomic rename to replace the current binary
    fs::rename(&tmp_path, exe_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("Failed to replace executable: {}", e)
    })?;

    Ok(())
}
