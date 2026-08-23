//! Machine facts for the greeting, cached so shells start fast.
//!
//! Every line this module produces comes from shelling out to a system tool,
//! and `system_profiler` alone takes the better part of a second. A greeting
//! runs on *every new terminal*, so probing on each run would make the shell
//! feel broken. Instead results land in a TOML file under the cache directory
//! and are refreshed on a timer.
//!
//! This is the module to read if you want to see why Pokefetch is structured
//! the way it is: the entire caching layer exists to protect shell startup.
//!
//! # Rust concepts on display
//!
//! - **Graceful degradation over `?`**: nothing here returns [`Result`]. A
//!   missing `brew` is not an error worth aborting a greeting over, so every
//!   probe falls back to a placeholder string.
//! - **`let _ =`**: deliberately discarding a [`Result`] you have decided to
//!   ignore. Writing `let _ = ...` is how you tell the compiler *and the next
//!   reader* that the failure is intentional, rather than silently unhandled.
//! - **`cfg!` vs `#[cfg]`**: `cfg!` is a runtime-looking boolean the optimizer
//!   folds away, so both branches must still compile on every platform.

use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::cache_dir;

/// How long hardware facts stay fresh. Cores and RAM effectively never change.
const HARDWARE_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

/// How long the package count stays fresh. Installs happen; an hour is a
/// compromise between an accurate number and re-listing every Homebrew formula.
const PACKAGE_LIFETIME: Duration = Duration::from_secs(60 * 60);

/// The cached machine description, as stored in `system.toml`.
///
/// The two `*_updated` fields are Unix timestamps rather than
/// [`SystemTime`]s because they have to survive a round trip through TOML.
/// Each has its own timestamp so the cheap-but-volatile package count can
/// refresh without re-running the expensive hardware probe.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SystemSnapshot {
    /// OS name and version, e.g. `macOS 15.3 · Apple M2`.
    pub system: String,
    /// CPU, GPU, and memory summary.
    pub hardware: String,
    /// When `system` and `hardware` were last probed (Unix seconds).
    pub hardware_updated: u64,
    /// Installed package count, e.g. `214 Brew`.
    pub packages: String,
    /// When `packages` was last probed (Unix seconds).
    pub packages_updated: u64,
}

/// Loads the cached machine description, refreshing whichever half is stale.
///
/// Cache read, write, and directory-creation failures are all ignored: a
/// greeting on a read-only home directory should still print, just slower.
pub fn snapshot() -> SystemSnapshot {
    let path = cache_dir().join("system.toml");
    let now = unix_timestamp();
    let mut snapshot = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| toml::from_str::<SystemSnapshot>(&text).ok())
        .unwrap_or_default();

    if !is_fresh(snapshot.hardware_updated, now, HARDWARE_LIFETIME) {
        let profile = mac_profile();
        snapshot.system = system_label(&profile);
        snapshot.hardware = hardware_label(&profile);
        snapshot.hardware_updated = now;
    }
    if !is_fresh(snapshot.packages_updated, now, PACKAGE_LIFETIME) {
        snapshot.packages = package_summary();
        snapshot.packages_updated = now;
    }

    if let Ok(text) = toml::to_string(&snapshot) {
        let _ = std::fs::create_dir_all(cache_dir());
        let _ = std::fs::write(path, text);
    }
    snapshot
}

/// Seconds since the Unix epoch, or 0 if the clock predates it.
fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Reports whether `timestamp` is set and younger than `lifetime`.
///
/// `saturating_sub` matters here: if the clock moved backwards, a naive
/// subtraction would underflow and panic in debug builds.
fn is_fresh(timestamp: u64, now: u64, lifetime: Duration) -> bool {
    timestamp > 0 && now.saturating_sub(timestamp) < lifetime.as_secs()
}

/// Builds the OS line, e.g. `macOS 15.3 · Apple M2`.
fn system_label(mac_profile: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("macOS {} · {}", macos_version(), mac_chip(mac_profile))
    } else {
        format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

/// Builds the hardware line, e.g. `8C CPU · 10C GPU · 16GB RAM`.
fn hardware_label(mac_profile: &str) -> String {
    if !cfg!(target_os = "macos") {
        return format!(
            "{} · {}",
            std::env::consts::ARCH,
            total_memory_label(mac_profile)
        );
    }

    // system_profiler reports "Total Number of Cores" twice: once under
    // hardware (CPU) and once under displays (GPU). Position disambiguates.
    let cpu = hardware_value(mac_profile, "Total Number of Cores")
        .and_then(|value| value.split_whitespace().next().map(str::to_owned))
        .map_or_else(|| "CPU".to_string(), |value| format!("{value}C CPU"));
    let gpu = hardware_values(mac_profile, "Total Number of Cores")
        .nth(1)
        .map_or_else(|| "GPU".to_string(), |value| format!("{value}C GPU"));

    format!("{cpu} · {gpu} · {} RAM", total_memory_label(mac_profile))
}

/// Runs `system_profiler` once and returns its combined report.
///
/// Both data types are requested in a single invocation because each spawn
/// costs hundreds of milliseconds.
fn mac_profile() -> String {
    if cfg!(target_os = "macos") {
        command_output(
            "/usr/sbin/system_profiler",
            &["SPHardwareDataType", "SPDisplaysDataType"],
        )
    } else {
        String::new()
    }
}

/// Reads the marketing version, e.g. `15.3`.
fn macos_version() -> String {
    command_output("/usr/bin/sw_vers", &["-productVersion"])
        .trim()
        .to_string()
}

/// Extracts the chip name, preferring Apple Silicon's `Chip` over Intel's
/// `Processor Name`.
fn mac_chip(profile: &str) -> String {
    hardware_value(profile, "Chip")
        .or_else(|| hardware_value(profile, "Processor Name"))
        .unwrap_or_else(|| "Apple Silicon".to_string())
}

/// Extracts installed memory, tightening `16 GB` to `16GB`.
fn total_memory_label(profile: &str) -> String {
    hardware_value(profile, "Memory")
        .map_or_else(|| "RAM".to_string(), |value| value.replace(" GB", "GB"))
}

/// Counts installed packages using the first package manager that answers.
///
/// The array pairs a program with its "list everything" arguments and a label.
/// `&["list", "--formula"][..]` coerces the fixed-size array to a slice so all
/// entries share one type despite differing argument counts — without the
/// `[..]`, `&["a"]` and `&["a", "b"]` would be `&[&str; 1]` and `&[&str; 2]`,
/// which are genuinely different types and cannot sit in the same array.
fn package_summary() -> String {
    let managers = [
        ("/opt/homebrew/bin/brew", &["list", "--formula"][..], "Brew"),
        ("/usr/local/bin/brew", &["list", "--formula"][..], "Brew"),
        (
            "dpkg-query",
            &["-W", "-f", "${binary:Package}\\n"][..],
            "Packages",
        ),
        ("rpm", &["-qa"][..], "Packages"),
        ("pacman", &["-Qq"][..], "Packages"),
        ("apk", &["info"][..], "Packages"),
    ];

    managers
        .iter()
        .find_map(|(program, args, label)| {
            let output = command_output(program, args);
            let count = output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
            (count > 0).then(|| format!("{count} {label}"))
        })
        .unwrap_or_else(|| "Packages".to_string())
}

/// Runs a command and returns its stdout, or an empty string on any failure.
///
/// A missing binary, a non-zero exit, and invalid UTF-8 are all treated the
/// same way, because the caller's response to each is identical.
fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// Returns the first `Key: Value` match from a `system_profiler` report.
fn hardware_value(output: &str, key: &str) -> Option<String> {
    hardware_values(output, key).next()
}

/// Returns every `Key: Value` match, in report order.
///
/// The `'a` lifetime says the returned iterator borrows `output` and `key`, so
/// the compiler will not let either be dropped while iteration is in progress.
/// `move` transfers the two references into the closure; without it the
/// closure would borrow local variables that vanish when this function returns.
fn hardware_values<'a>(output: &'a str, key: &'a str) -> impl Iterator<Item = String> + 'a {
    output.lines().filter_map(move |line| {
        let (label, value) = line.split_once(':')?;
        (label.trim() == key).then(|| value.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{hardware_value, hardware_values, is_fresh};

    const PROFILE: &str = "      Chip: Apple M2\n      Total Number of Cores: 8 (4 performance and 4 efficiency)\n      Memory: 16 GB\n      Total Number of Cores: 10\n";

    #[test]
    fn reads_the_first_matching_hardware_key() {
        assert_eq!(hardware_value(PROFILE, "Chip").unwrap(), "Apple M2");
        assert_eq!(hardware_value(PROFILE, "Memory").unwrap(), "16 GB");
        assert_eq!(hardware_value(PROFILE, "Nonexistent"), None);
    }

    #[test]
    fn distinguishes_repeated_keys_by_position() {
        let cores = hardware_values(PROFILE, "Total Number of Cores").collect::<Vec<_>>();
        assert_eq!(cores.len(), 2);
        assert!(cores[0].starts_with('8'));
        assert_eq!(cores[1], "10");
    }

    #[test]
    fn treats_unset_and_expired_timestamps_as_stale() {
        let lifetime = Duration::from_secs(100);
        assert!(!is_fresh(0, 1_000, lifetime), "unset is never fresh");
        assert!(is_fresh(950, 1_000, lifetime), "within the lifetime");
        assert!(!is_fresh(800, 1_000, lifetime), "past the lifetime");
        assert!(is_fresh(2_000, 1_000, lifetime), "clock moved backwards");
    }
}
