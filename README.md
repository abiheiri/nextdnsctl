# nextdnsctl

A command-line tool for managing your [NextDNS](https://nextdns.io) configuration and viewing analytics.

## Features

- View DNS query logs (all, allowed, or blocked)
- Manage allowlist and denylist domains
- View analytics and statistics (top domains, blocked queries, etc.)
- List and switch between profiles
- Self-update from GitHub releases

## Installation

Download the latest release for your platform from the [releases page](https://github.com/abiheiri/nextdnsctl/releases).

```sh
mkdir -p ~/.local/bin

# macOS (Apple Silicon)
curl -L https://github.com/abiheiri/nextdnsctl/releases/latest/download/nextdnsctl-mac-arm64.tar.gz | tar xz
mv nextdnsctl ~/.local/bin/

# macOS (Intel)
curl -L https://github.com/abiheiri/nextdnsctl/releases/latest/download/nextdnsctl-mac-x64.tar.gz | tar xz
mv nextdnsctl ~/.local/bin/

# Linux (x86_64)
curl -L https://github.com/abiheiri/nextdnsctl/releases/latest/download/nextdnsctl-linux-x64.tar.gz | tar xz
mv nextdnsctl ~/.local/bin/

# Linux (ARM64)
curl -L https://github.com/abiheiri/nextdnsctl/releases/latest/download/nextdnsctl-linux-arm64.tar.gz | tar xz
mv nextdnsctl ~/.local/bin/
```

Make sure `~/.local/bin` is in your `PATH`. Add this to your shell profile if needed:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

## Configuration

Create `~/.nextdns` with your API key:

```
api=YOUR_API_KEY
profile=PROFILE_ID_OR_NAME
```

Get your API key at: https://my.nextdns.io/account

The `profile` line is optional. If omitted and you only have one profile, it will be used automatically.

You can also pass credentials as flags, which override the config file:

```sh
nextdnsctl --api-key YOUR_KEY --profile YOUR_PROFILE ls logs
```

## Usage

```sh
# List DNS query logs (today)
nextdnsctl ls logs

# List only blocked queries from the last 7 days
nextdnsctl ls logs deny -d 7

# List available profiles
nextdnsctl ls profiles

# View analytics (last 30 days)
nextdnsctl stat

# View analytics for last 90 minutes
nextdnsctl stat -m 90

# Add a domain to the allowlist
nextdnsctl allow example.com

# Add a domain to the denylist
nextdnsctl deny ads.example.com

# Remove a domain from the denylist
nextdnsctl rm deny ads.example.com

# List allowlist entries
nextdnsctl ls allow

# Update to the latest version
nextdnsctl update
```

## Building from source

Requires [Rust](https://rustup.rs/).

```sh
git clone https://github.com/abiheiri/nextdnsctl.git
cd nextdnsctl
cargo build --release
```

The binary will be at `target/release/nextdnsctl`.

## License

Copyright 2025 AL Biheiri <al@forgottheaddress.com>

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
