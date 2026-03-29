mod api;
mod config;
mod update;

use clap::{Parser, Subcommand};
use std::io::Write;

const VERSION: &str = env!("NEXTDNS_VERSION");

/// Write to stdout, exit silently on broken pipe.
macro_rules! out {
    ($($arg:tt)*) => {
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    };
}

#[derive(Parser)]
#[command(
    name = "nextdns-cli",
    about = "CLI tool for interacting with NextDNS API",
    version = VERSION,
    after_help = "\
Credentials are read from ~/.nextdns (key=value format):

  api=YOUR_API_KEY
  profile=PROFILE_ID_OR_NAME   (optional)

Use --api-key and --profile to override these values. When --api-key is \
provided, the config file is not required.

Get your API key at: https://my.nextdns.io/account",
)]
struct Cli {
    /// API key (overrides ~/.nextdns)
    #[arg(long, global = true)]
    api_key: Option<String>,

    /// Profile ID or name (overrides ~/.nextdns)
    #[arg(long, global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List resources (logs, profiles, allow, deny)
    Ls {
        #[command(subcommand)]
        resource: LsResource,
    },
    /// Add a domain to the allowlist
    Allow {
        /// Domain to allow
        domain: String,
    },
    /// Add a domain to the denylist
    Deny {
        /// Domain to deny
        domain: String,
    },
    /// Remove a domain from a list
    Rm {
        #[command(subcommand)]
        resource: RmResource,
    },
    /// Update to the latest release from GitHub
    Update,
    /// Show analytics and statistics for your profile
    Stat {
        /// Number of days of stats to retrieve (default: 30)
        #[arg(short, long, default_value_t = 30)]
        days: u32,

        /// Number of minutes of stats to retrieve (overrides -d)
        #[arg(short, long)]
        minutes: Option<u32>,
    },
}

#[derive(Subcommand)]
enum LsResource {
    /// List DNS query logs (all or filtered by status)
    Logs {
        #[command(subcommand)]
        filter: Option<LogFilter>,

        /// Number of days of logs to retrieve (default: 1 = today only)
        #[arg(short, long, default_value_t = 1)]
        days: u32,

        /// Number of minutes of logs to retrieve (overrides -d)
        #[arg(short, long)]
        minutes: Option<u32>,
    },
    /// List available profiles
    Profiles,
    /// List the allowlist
    Allow,
    /// List the denylist
    Deny,
}

#[derive(Subcommand)]
enum LogFilter {
    /// Show only blocked/denied queries
    Deny {
        /// Number of days of logs to retrieve (default: 1 = today only)
        #[arg(short, long, default_value_t = 1)]
        days: u32,

        /// Number of minutes of logs to retrieve (overrides -d)
        #[arg(short, long)]
        minutes: Option<u32>,
    },
    /// Show only allowed queries
    Allow {
        /// Number of days of logs to retrieve (default: 1 = today only)
        #[arg(short, long, default_value_t = 1)]
        days: u32,

        /// Number of minutes of logs to retrieve (overrides -d)
        #[arg(short, long)]
        minutes: Option<u32>,
    },
}

#[derive(Subcommand)]
enum RmResource {
    /// Remove a domain from the allowlist
    Allow {
        /// Domain to remove
        domain: String,
    },
    /// Remove a domain from the denylist
    Deny {
        /// Domain to remove
        domain: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let overrides = ConfigOverrides {
        api_key: cli.api_key,
        profile: cli.profile,
    };

    match cli.command {
        Commands::Ls { resource } => match resource {
            LsResource::Logs { filter, days, minutes } => match filter {
                None => cmd_logs(days, minutes, None, &overrides),
                Some(LogFilter::Deny { days: d, minutes: m }) => cmd_logs(d, m, Some("blocked"), &overrides),
                Some(LogFilter::Allow { days: d, minutes: m }) => cmd_logs(d, m, Some("default"), &overrides),
            },
            LsResource::Profiles => cmd_profiles(&overrides),
            LsResource::Allow => cmd_list_entries("allowlist", &overrides),
            LsResource::Deny => cmd_list_entries("denylist", &overrides),
        },
        Commands::Update => {
            update::run_update(VERSION);
            return;
        }
        Commands::Allow { domain } => cmd_add_entry("allowlist", &domain, &overrides),
        Commands::Deny { domain } => cmd_add_entry("denylist", &domain, &overrides),
        Commands::Rm { resource } => match resource {
            RmResource::Allow { domain } => cmd_remove_entry("allowlist", &domain, &overrides),
            RmResource::Deny { domain } => cmd_remove_entry("denylist", &domain, &overrides),
        },
        Commands::Stat { days, minutes } => cmd_stat(days, minutes, &overrides),
    }
}

struct ConfigOverrides {
    api_key: Option<String>,
    profile: Option<String>,
}

fn init_client(overrides: &ConfigOverrides) -> (config::Config, api::NextDnsClient, String) {
    let mut cfg = if overrides.api_key.is_some() {
        // If API key is provided via CLI, config file is not required
        config::Config {
            api_key: String::new(),
            profile: None,
        }
    } else {
        match config::load_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Apply CLI overrides
    if let Some(ref key) = overrides.api_key {
        cfg.api_key = key.clone();
    }
    if let Some(ref profile) = overrides.profile {
        cfg.profile = Some(profile.clone());
    }

    let client = api::NextDnsClient::new(&cfg.api_key);

    let profile_id = match client.resolve_profile(cfg.profile.as_deref()) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    (cfg, client, profile_id)
}

fn cmd_logs(days: u32, minutes: Option<u32>, status_filter: Option<&str>, overrides: &ConfigOverrides) {
    let (_cfg, client, profile_id) = init_client(overrides);

    let from = if let Some(m) = minutes {
        format!("-{}m", m)
    } else {
        format!("-{}d", days)
    };

    let period = if let Some(m) = minutes {
        format!("{} minute{}", m, if m == 1 { "" } else { "s" })
    } else {
        format!("{} day{}", days, if days == 1 { "" } else { "s" })
    };

    let filter_label = match status_filter {
        Some(s) => format!(" [{}]", s),
        None => String::new(),
    };

    eprintln!(
        "Fetching logs for profile {} ({}){}...",
        profile_id, period, filter_label
    );

    out!(
        "{:<28} {:<8} {:<40} {}",
        "TIMESTAMP", "STATUS", "DOMAIN", "DEVICE"
    );
    out!("{}", "-".repeat(100));

    let total = match client.stream_logs(&profile_id, &from, status_filter, |batch| {
        for entry in batch {
            let timestamp = &entry.timestamp;
            let status = entry.status.as_deref().unwrap_or("-");
            let domain = entry.domain.as_deref().unwrap_or("-");
            let device = entry
                .device
                .as_ref()
                .and_then(|d| d.name.as_deref())
                .unwrap_or("-");

            out!("{:<28} {:<8} {:<40} {}", timestamp, status, domain, device);
        }
    }) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if total == 0 {
        out!("No logs found for the specified period.");
    }

    eprintln!("\nTotal: {} log entries", total);
}

fn cmd_profiles(overrides: &ConfigOverrides) {
    let (cfg, client, _profile_id) = init_client(overrides);

    let profiles = match client.list_profiles() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if profiles.is_empty() {
        out!("No profiles found.");
        return;
    }

    out!("{:<12} {}", "ID", "NAME");
    out!("{}", "-".repeat(40));

    for p in &profiles {
        let marker = if cfg.profile.as_deref() == Some(&p.id) {
            " *"
        } else {
            ""
        };
        out!("{:<12} {}{}", p.id, p.name, marker);
    }
}

fn cmd_list_entries(list: &str, overrides: &ConfigOverrides) {
    let (_cfg, client, profile_id) = init_client(overrides);

    let entries = match client.list_entries(&profile_id, list) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if entries.is_empty() {
        out!("No entries in {}.", list);
        return;
    }

    out!("{:<40} {}", "DOMAIN", "ACTIVE");
    out!("{}", "-".repeat(50));

    for entry in &entries {
        let status = if entry.active { "yes" } else { "no" };
        out!("{:<40} {}", entry.id, status);
    }

    eprintln!("\nTotal: {} entries", entries.len());
}

fn cmd_add_entry(list: &str, domain: &str, overrides: &ConfigOverrides) {
    let (_cfg, client, profile_id) = init_client(overrides);

    let opposite = if list == "allowlist" {
        "denylist"
    } else {
        "allowlist"
    };

    // Check if domain already exists in the target list
    let entries = match client.list_entries(&profile_id, list) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if entries.iter().any(|e| e.id == domain) {
        eprintln!("Error: '{}' already exists in the {}.", domain, list);
        std::process::exit(1);
    }

    // Check if domain exists in the opposite list
    let opposite_entries = match client.list_entries(&profile_id, opposite) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if opposite_entries.iter().any(|e| e.id == domain) {
        eprintln!(
            "Error: '{}' already exists in the {}. Remove it first with:\n\n  nextdns-cli rm {} {}",
            domain, opposite,
            if opposite == "allowlist" { "allow" } else { "deny" },
            domain
        );
        std::process::exit(1);
    }

    // Add the entry
    match client.add_entry(&profile_id, list, domain) {
        Ok(()) => out!("Added '{}' to {}.", domain, list),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_remove_entry(list: &str, domain: &str, overrides: &ConfigOverrides) {
    let (_cfg, client, profile_id) = init_client(overrides);

    match client.remove_entry(&profile_id, list, domain) {
        Ok(()) => out!("Removed '{}' from {}.", domain, list),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

const MAX_STAT_DAYS: u32 = 90;

fn cmd_stat(days: u32, minutes: Option<u32>, overrides: &ConfigOverrides) {
    // Validate 90-day maximum
    let exceeds = if let Some(m) = minutes {
        m > MAX_STAT_DAYS * 24 * 60
    } else {
        days > MAX_STAT_DAYS
    };

    if exceeds {
        eprintln!("Error: Maximum range is {} days (3 months).", MAX_STAT_DAYS);
        std::process::exit(1);
    }

    // Build time parameters
    let from = if let Some(m) = minutes {
        format!("-{}m", m)
    } else {
        format!("-{}d", days)
    };

    let period = if let Some(m) = minutes {
        format!("{} minute{}", m, if m == 1 { "" } else { "s" })
    } else {
        format!("{} day{}", days, if days == 1 { "" } else { "s" })
    };

    let (_cfg, client, profile_id) = init_client(overrides);

    eprintln!("Fetching stats for profile {} ({})...", profile_id, period);

    // --- Section 1: Query Overview ---
    let statuses = match client.get_analytics_status(&profile_id, &from) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let total: u64 = statuses.iter().map(|s| s.queries).sum();
    let blocked: u64 = statuses
        .iter()
        .filter(|s| s.status == "blocked")
        .map(|s| s.queries)
        .sum();
    let allowed: u64 = total - blocked;

    let blocked_pct = if total > 0 {
        (blocked as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let allowed_pct = if total > 0 {
        (allowed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    out!("");
    out!("  Query Overview ({})", period);
    out!("  {}", "-".repeat(40));
    out!("  Total Queries:    {}", format_number(total));
    out!(
        "  Blocked Queries:  {} ({:.1}%)",
        format_number(blocked),
        blocked_pct
    );
    out!(
        "  Allowed Queries:  {} ({:.1}%)",
        format_number(allowed),
        allowed_pct
    );

    // --- Section 2: Top Allowed Domains ---
    let top_allowed = match client.get_analytics_domains(
        &profile_id, &from, Some("default"), false, 10,
    ) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error fetching allowed domains: {}", e);
            std::process::exit(1);
        }
    };

    print_domain_table("Top Allowed Domains", &top_allowed);

    // --- Section 3: Top Blocked Domains ---
    let top_blocked = match client.get_analytics_domains(
        &profile_id, &from, Some("blocked"), false, 10,
    ) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error fetching blocked domains: {}", e);
            std::process::exit(1);
        }
    };

    print_domain_table("Top Blocked Domains", &top_blocked);

    // --- Section 4: Top Root Domains ---
    let top_root = match client.get_analytics_domains(
        &profile_id, &from, None, true, 10,
    ) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error fetching root domains: {}", e);
            std::process::exit(1);
        }
    };

    print_domain_table("Top Root Domains", &top_root);
    out!("");
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn print_domain_table(title: &str, data: &[api::DomainEntry]) {
    out!("");
    out!("  {}", title);
    out!("  {}", "-".repeat(58));
    out!("  {:<4} {:<40} {:>12}", "#", "DOMAIN", "QUERIES");
    out!("  {}", "-".repeat(58));

    if data.is_empty() {
        out!("  No data.");
    } else {
        for (i, entry) in data.iter().enumerate() {
            let domain = truncate(&entry.domain, 40);
            out!(
                "  {:<4} {:<40} {:>12}",
                format!("{}.", i + 1),
                domain,
                format_number(entry.queries)
            );
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
