use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

const BASE_URL: &str = "https://api.nextdns.io";

pub struct NextDnsClient {
    client: Client,
    api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ProfileListResponse {
    pub data: Vec<Profile>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LogDevice {
    pub id: Option<String>,
    pub name: Option<String>,
    pub model: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LogReason {
    pub id: String,
    pub name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub domain: Option<String>,
    pub root: Option<String>,
    pub encrypted: Option<bool>,
    pub protocol: Option<String>,
    #[serde(rename = "clientIp")]
    pub client_ip: Option<String>,
    pub device: Option<LogDevice>,
    pub status: Option<String>,
    pub reasons: Option<Vec<LogReason>>,
}

#[derive(Debug, Deserialize)]
pub struct LogMeta {
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize)]
pub struct Pagination {
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogResponse {
    pub data: Vec<LogEntry>,
    pub meta: Option<LogMeta>,
}

#[derive(Debug, Deserialize)]
pub struct ListEntry {
    pub id: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListResponse {
    pub data: Vec<ListEntry>,
}

#[derive(Debug, Serialize)]
pub struct ListEntryRequest {
    pub id: String,
    pub active: bool,
}

impl NextDnsClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>, String> {
        let url = format!("{}/profiles", BASE_URL);
        let resp = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API error ({}): {}", status, body));
        }

        let result: ProfileListResponse = resp
            .json()
            .map_err(|e| format!("Failed to parse profiles response: {}", e))?;

        Ok(result.data)
    }

    pub fn stream_logs<F>(
        &self,
        profile_id: &str,
        from: &str,
        to: &str,
        mut on_batch: F,
    ) -> Result<usize, String>
    where
        F: FnMut(&[LogEntry]),
    {
        let mut total = 0usize;
        let mut cursor: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/profiles/{}/logs?from={}&to={}&limit=1000",
                BASE_URL, profile_id, from, to
            );

            if let Some(ref c) = cursor {
                url.push_str(&format!("&cursor={}", c));
            }

            let resp = self
                .client
                .get(&url)
                .header("X-Api-Key", &self.api_key)
                .send()
                .map_err(|e| format!("Request failed: {}", e))?;

            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().unwrap_or_default();
                return Err(format!("API error ({}): {}", status, body));
            }

            let result: LogResponse = resp
                .json()
                .map_err(|e| format!("Failed to parse logs response: {}", e))?;

            total += result.data.len();
            on_batch(&result.data);

            let next_cursor = result
                .meta
                .as_ref()
                .and_then(|m| m.pagination.as_ref())
                .and_then(|p| p.cursor.clone());

            match next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        Ok(total)
    }

    pub fn resolve_profile(&self, configured_profile: Option<&str>) -> Result<String, String> {
        let profiles = self.list_profiles()?;

        // If user provided a profile value, match by ID or name
        if let Some(profile) = configured_profile {
            // Check if it matches an ID directly
            if profiles.iter().any(|p| p.id == profile) {
                return Ok(profile.to_string());
            }
            // Check if it matches a name (case-insensitive)
            if let Some(p) = profiles
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(profile))
            {
                eprintln!("Resolved profile '{}' -> {} ({})", profile, p.id, p.name);
                return Ok(p.id.clone());
            }
            return Err(format!(
                "Profile '{}' not found. Available profiles:\n{}",
                profile,
                profiles
                    .iter()
                    .map(|p| format!("  {}  # {}", p.id, p.name))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        match profiles.len() {
            0 => Err("No profiles found in your NextDNS account.".to_string()),
            1 => {
                let p = &profiles[0];
                eprintln!("Using profile: {} ({})", p.name, p.id);
                Ok(p.id.clone())
            }
            _ => {
                eprintln!("Multiple profiles found. Set a default in ~/.nextdns:");
                eprintln!();
                for p in &profiles {
                    eprintln!("  profile={}  # {}", p.id, p.name);
                }
                eprintln!();
                Err(
                    "Add 'profile=PROFILE_ID' to ~/.nextdns to select a default profile."
                        .to_string(),
                )
            }
        }
    }

    pub fn list_entries(
        &self,
        profile_id: &str,
        list: &str,
    ) -> Result<Vec<ListEntry>, String> {
        let url = format!("{}/profiles/{}/{}", BASE_URL, profile_id, list);
        let resp = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API error ({}): {}", status, body));
        }

        let result: ListResponse = resp
            .json()
            .map_err(|e| format!("Failed to parse {} response: {}", list, e))?;

        Ok(result.data)
    }

    pub fn add_entry(
        &self,
        profile_id: &str,
        list: &str,
        domain: &str,
    ) -> Result<(), String> {
        let url = format!("{}/profiles/{}/{}", BASE_URL, profile_id, list);
        let body = ListEntryRequest {
            id: domain.to_string(),
            active: true,
        };

        let resp = self
            .client
            .post(&url)
            .header("X-Api-Key", &self.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API error ({}): {}", status, body));
        }

        Ok(())
    }

    pub fn remove_entry(
        &self,
        profile_id: &str,
        list: &str,
        domain: &str,
    ) -> Result<(), String> {
        let url = format!("{}/profiles/{}/{}/{}", BASE_URL, profile_id, list, domain);
        let resp = self
            .client
            .delete(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API error ({}): {}", status, body));
        }

        Ok(())
    }
}
