//
// This module handles loading the configuration from a file in the user's home directory.
//

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Config {
    pub api_key: String,
    pub profile: Option<String>,
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".nextdns")
}

pub fn load_config() -> Result<Config, String> {
    let path = config_path();

    if !path.exists() {
        return Err(format!(
            "Config file not found: {}\n\n\
             Create the file and add your NextDNS API key:\n\n\
             echo 'api=YOUR_API_KEY_HERE' > ~/.nextdns\n\n\
             You can find your API key at: https://my.nextdns.io/account\n\
             Optionally add a default profile:\n\n\
             echo 'profile=YOUR_PROFILE_ID' >> ~/.nextdns",
            path.display()
        ));
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut values: HashMap<String, String> = HashMap::new();

    for line in contents.lines() {
        let trimmed = line.trim();

        // skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            let cleaned = value.trim().trim_matches(|c| c == '"' || c == '\'');
            values.insert(key.trim().to_lowercase(), cleaned.to_string());
        }
    }

    let api_key = values
        .get("api")
        .cloned()
        .ok_or_else(|| {
            format!(
                "No API key found in {}\n\n\
                 Add your NextDNS API key like this:\n\n\
                 api=YOUR_API_KEY_HERE\n\n\
                 You can find your API key at: https://my.nextdns.io/account",
                path.display()
            )
        })?;

    if api_key.is_empty() || api_key == "YOUR_API_KEY_HERE" {
        return Err(
            "API key is not set. Replace YOUR_API_KEY_HERE with your actual API key in ~/.nextdns"
                .to_string(),
        );
    }

    let profile = values.get("profile").cloned();

    Ok(Config { api_key, profile })
}

/// Load the GitHub token from ~/.nextdns for self-update.
/// Returns Ok(token) if found and not a placeholder.
/// If the file exists but has no github_token key, appends a placeholder and tells the user.
/// If the file doesn't exist, tells the user to create it.
pub fn load_github_token() -> Result<String, String> {
    let path = config_path();

    if !path.exists() {
        return Err(
            "Config file ~/.nextdns not found.\n\n\
             Create it and add your GitHub token:\n\n\
             echo 'github_token=YOUR_GITHUB_TOKEN' > ~/.nextdns\n\n\
             The token needs 'repo' scope for private repository access."
                .to_string(),
        );
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut values: HashMap<String, String> = HashMap::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let cleaned = value.trim().trim_matches(|c| c == '"' || c == '\'');
            values.insert(key.trim().to_lowercase(), cleaned.to_string());
        }
    }

    match values.get("github_token") {
        None => {
            // Append placeholder to existing file
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;

            writeln!(file, "\ngithub_token=YOUR_GITHUB_TOKEN")
                .map_err(|e| format!("Failed to write to {}: {}", path.display(), e))?;

            Err(
                "GitHub token not found in ~/.nextdns.\n\n\
                 A placeholder has been added. Edit ~/.nextdns and replace YOUR_GITHUB_TOKEN \
                 with your actual token.\n\n\
                 The token needs 'repo' scope for private repository access."
                    .to_string(),
            )
        }
        Some(token) if token.is_empty() || token == "YOUR_GITHUB_TOKEN" => {
            Err(
                "GitHub token is not set. Replace YOUR_GITHUB_TOKEN with your actual token in ~/.nextdns.\n\n\
                 The token needs 'repo' scope for private repository access."
                    .to_string(),
            )
        }
        Some(token) => Ok(token.clone()),
    }
}
