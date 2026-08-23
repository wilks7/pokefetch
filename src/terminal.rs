use std::io::{self, IsTerminal, Write};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::config::{cache_dir, Alignment, DisplayConfig};
use crate::palette::{Color, SIZE as PALETTE_SIZE};
use crate::pokemon::Pokemon;

pub fn print_greeting(
    png: &[u8],
    pokemon: &Pokemon,
    variant: &str,
    palette: &[Color; PALETTE_SIZE],
    display: &DisplayConfig,
    force_kitty: bool,
) -> Result<()> {
    let mut output = io::stdout().lock();
    let terminal = io::stdout().is_terminal();
    let supports_images = should_render_image(force_kitty);
    let lines = information_lines(pokemon, variant);
    let layout = greeting_layout(lines.len(), display)?;

    if supports_images {
        for _ in 0..layout.image_offset {
            write!(output, "\r\n")?;
        }
        transmit_kitty(&mut output, png, display.columns(), display.size)?;
        if layout.image_offset > 0 {
            write!(output, "\x1b[{}A", layout.image_offset)?;
        }
        for _ in 0..layout.text_offset {
            write!(output, "\r\n")?;
        }
        for (line, color) in lines.iter().zip(palette.iter().cycle().take(lines.len())) {
            write!(
                output,
                "\r\x1b[{}C\x1b[38;2;{};{};{}m{}\x1b[0m\r\n",
                display.columns() + u32::from(display.gap),
                color.red,
                color.green,
                color.blue,
                line
            )?;
        }
        let occupied = layout.text_offset + lines.len();
        for _ in occupied..layout.height {
            write!(output, "\r\n")?;
        }
    } else {
        for (line, color) in lines.iter().zip(palette.iter().cycle().take(lines.len())) {
            if terminal {
                writeln!(
                    output,
                    "\x1b[38;2;{};{};{}m{}\x1b[0m",
                    color.red, color.green, color.blue, line
                )?;
            } else {
                writeln!(output, "{line}")?;
            }
        }
    }
    output.flush().context("flushing greeting")
}

#[derive(Debug, Eq, PartialEq)]
struct GreetingLayout {
    height: usize,
    image_offset: usize,
    text_offset: usize,
}

fn greeting_layout(line_count: usize, display: &DisplayConfig) -> Result<GreetingLayout> {
    anyhow::ensure!(
        (1..=PALETTE_SIZE).contains(&line_count),
        "greeting needs between 1 and {PALETTE_SIZE} information lines"
    );
    let image_height = usize::from(display.size);
    let height = image_height.max(line_count);
    let (image_offset, text_offset) = match display.alignment {
        Alignment::Top => (0, 0),
        Alignment::Center => ((height - image_height) / 2, (height - line_count) / 2),
    };
    Ok(GreetingLayout {
        height,
        image_offset,
        text_offset,
    })
}

pub fn supports_kitty_graphics() -> bool {
    matches!(
        std::env::var("TERM_PROGRAM").as_deref(),
        Ok("ghostty") | Ok("kitty") | Ok("WezTerm")
    ) || std::env::var_os("KITTY_WINDOW_ID").is_some()
}

pub fn should_render_image(force_kitty: bool) -> bool {
    force_kitty || (io::stdout().is_terminal() && supports_kitty_graphics())
}

pub fn is_local_ghostty() -> bool {
    std::env::var("TERM_PROGRAM").as_deref() == Ok("ghostty")
        && std::env::var_os("SSH_CONNECTION").is_none()
        && std::env::var_os("SSH_TTY").is_none()
}

#[derive(Debug, Deserialize, Serialize)]
struct SystemSnapshot {
    system: String,
    hardware: String,
    hardware_updated: u64,
    packages: String,
    packages_updated: u64,
}

fn system_snapshot() -> SystemSnapshot {
    let path = cache_dir().join("system.toml");
    let now = unix_timestamp();
    let mut snapshot = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| toml::from_str::<SystemSnapshot>(&text).ok())
        .unwrap_or_else(|| SystemSnapshot {
            system: String::new(),
            hardware: String::new(),
            hardware_updated: 0,
            packages: String::new(),
            packages_updated: 0,
        });

    if !is_fresh(
        snapshot.hardware_updated,
        now,
        Duration::from_secs(24 * 60 * 60),
    ) {
        let profile = mac_profile();
        snapshot.system = system_label(&profile);
        snapshot.hardware = hardware_label(&profile);
        snapshot.hardware_updated = now;
    }
    if !is_fresh(snapshot.packages_updated, now, Duration::from_secs(60 * 60)) {
        snapshot.packages = package_summary();
        snapshot.packages_updated = now;
    }

    if let Ok(text) = toml::to_string(&snapshot) {
        let _ = std::fs::create_dir_all(cache_dir());
        let _ = std::fs::write(path, text);
    }
    snapshot
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_fresh(timestamp: u64, now: u64, lifetime: Duration) -> bool {
    timestamp > 0 && now.saturating_sub(timestamp) < lifetime.as_secs()
}

fn transmit_kitty(writer: &mut impl Write, png: &[u8], columns: u32, rows: u16) -> Result<()> {
    let encoded = STANDARD.encode(png);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(4096).collect();
    for (index, chunk) in chunks.iter().enumerate() {
        let more = usize::from(index + 1 < chunks.len());
        if index == 0 {
            write!(
                writer,
                "\x1b_Ga=T,f=100,q=2,C=1,c={columns},r={rows},m={more};"
            )?;
        } else {
            write!(writer, "\x1b_Gm={more};")?;
        }
        writer.write_all(chunk)?;
        write!(writer, "\x1b\\")?;
    }
    Ok(())
}

fn information_lines(pokemon: &Pokemon, variant: &str) -> Vec<String> {
    let user = std::env::var("USER")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| capitalize(&value))
        .unwrap_or_else(|| "Trainer".to_string());
    let host = hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let shell = std::env::var("SHELL")
        .ok()
        .and_then(|value| value.rsplit('/').next().map(str::to_owned))
        .map(|value| capitalize(&value))
        .unwrap_or_else(|| "Shell".to_string());
    let terminal = std::env::var("TERM_PROGRAM")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| capitalize(&value))
        .unwrap_or_else(|| "Terminal".to_string());
    let snapshot = system_snapshot();

    vec![
        format!("{user} @ {host}"),
        snapshot.system,
        snapshot.hardware,
        format!("{shell} · {terminal} · {}", snapshot.packages),
        format!("{} · {variant}", pokemon.label()),
    ]
}

fn system_label(mac_profile: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("macOS {} · {}", macos_version(), mac_chip(mac_profile))
    } else {
        format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

fn hardware_label(mac_profile: &str) -> String {
    if !cfg!(target_os = "macos") {
        return format!(
            "{} · {}",
            std::env::consts::ARCH,
            total_memory_label(mac_profile)
        );
    }

    let cpu = hardware_value(mac_profile, "Total Number of Cores")
        .and_then(|value| value.split_whitespace().next().map(str::to_owned))
        .map(|value| format!("{value}C CPU"))
        .unwrap_or_else(|| "CPU".to_string());
    let gpu = hardware_values(mac_profile, "Total Number of Cores")
        .nth(1)
        .map(|value| format!("{value}C GPU"))
        .unwrap_or_else(|| "GPU".to_string());

    format!("{cpu} · {gpu} · {} RAM", total_memory_label(mac_profile))
}

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

fn macos_version() -> String {
    command_output("/usr/bin/sw_vers", &["-productVersion"])
        .trim()
        .to_string()
}

fn mac_chip(profile: &str) -> String {
    hardware_value(profile, "Chip")
        .or_else(|| hardware_value(profile, "Processor Name"))
        .unwrap_or_else(|| "Apple Silicon".to_string())
}

fn total_memory_label(profile: &str) -> String {
    hardware_value(profile, "Memory")
        .map(|value| value.replace(" GB", "GB"))
        .unwrap_or_else(|| "RAM".to_string())
}

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

fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

fn hardware_value(output: &str, key: &str) -> Option<String> {
    hardware_values(output, key).next()
}

fn hardware_values<'a>(output: &'a str, key: &'a str) -> impl Iterator<Item = String> + 'a {
    output.lines().filter_map(move |line| {
        let (label, value) = line.split_once(':')?;
        (label.trim() == key).then(|| value.trim().to_string())
    })
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{greeting_layout, GreetingLayout};
    use crate::config::{Alignment, DisplayConfig};

    #[test]
    fn supports_between_one_and_eight_information_lines() {
        let display = DisplayConfig::default();
        assert!(greeting_layout(1, &display).is_ok());
        assert!(greeting_layout(8, &display).is_ok());
        assert!(greeting_layout(0, &display).is_err());
        assert!(greeting_layout(9, &display).is_err());
    }

    #[test]
    fn centers_shorter_text_or_image_by_terminal_row() {
        let display = DisplayConfig::default();
        assert_eq!(
            greeting_layout(5, &display).unwrap(),
            GreetingLayout {
                height: 8,
                image_offset: 0,
                text_offset: 1,
            }
        );

        let display = DisplayConfig {
            size: 2,
            ..DisplayConfig::default()
        };
        assert_eq!(
            greeting_layout(6, &display).unwrap(),
            GreetingLayout {
                height: 6,
                image_offset: 2,
                text_offset: 0,
            }
        );
    }

    #[test]
    fn top_alignment_never_adds_an_offset() {
        let display = DisplayConfig {
            size: 2,
            alignment: Alignment::Top,
            ..DisplayConfig::default()
        };
        assert_eq!(
            greeting_layout(6, &display).unwrap(),
            GreetingLayout {
                height: 6,
                image_offset: 0,
                text_offset: 0,
            }
        );
    }
}
