mod api;
mod config;

use clap::{Parser, Subcommand};
use std::io::Write;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
)]
struct Cli {
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

    match cli.command {
        Commands::Ls { resource } => match resource {
            LsResource::Logs { filter, days, minutes } => match filter {
                None => cmd_logs(days, minutes, None),
                Some(LogFilter::Deny { days: d, minutes: m }) => cmd_logs(d, m, Some("blocked")),
                Some(LogFilter::Allow { days: d, minutes: m }) => cmd_logs(d, m, Some("default")),
            },
            LsResource::Profiles => cmd_profiles(),
            LsResource::Allow => cmd_list_entries("allowlist"),
            LsResource::Deny => cmd_list_entries("denylist"),
        },
        Commands::Allow { domain } => cmd_add_entry("allowlist", &domain),
        Commands::Deny { domain } => cmd_add_entry("denylist", &domain),
        Commands::Rm { resource } => match resource {
            RmResource::Allow { domain } => cmd_remove_entry("allowlist", &domain),
            RmResource::Deny { domain } => cmd_remove_entry("denylist", &domain),
        },
    }
}

fn init_client() -> (config::Config, api::NextDnsClient, String) {
    let cfg = match config::load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

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

fn cmd_logs(days: u32, minutes: Option<u32>, status_filter: Option<&str>) {
    let (_cfg, client, profile_id) = init_client();

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

fn cmd_profiles() {
    let (cfg, client, _profile_id) = init_client();

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

fn cmd_list_entries(list: &str) {
    let (_cfg, client, profile_id) = init_client();

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

fn cmd_add_entry(list: &str, domain: &str) {
    let (_cfg, client, profile_id) = init_client();

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

fn cmd_remove_entry(list: &str, domain: &str) {
    let (_cfg, client, profile_id) = init_client();

    match client.remove_entry(&profile_id, list, domain) {
        Ok(()) => out!("Removed '{}' from {}.", domain, list),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
