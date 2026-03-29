fn main() {
    // Re-run if the env var changes between builds.
    println!("cargo:rerun-if-env-changed=NEXTDNS_VERSION");

    // If NEXTDNS_VERSION is set (e.g. by CI from the git tag), expose it as
    // a compile-time variable. Otherwise fall back to CARGO_PKG_VERSION.
    if let Ok(version) = std::env::var("NEXTDNS_VERSION") {
        let version = version.strip_prefix('v').unwrap_or(&version);
        println!("cargo:rustc-env=NEXTDNS_VERSION={}", version);
    } else {
        println!(
            "cargo:rustc-env=NEXTDNS_VERSION={}",
            std::env::var("CARGO_PKG_VERSION").unwrap()
        );
    }
}
