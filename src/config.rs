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
