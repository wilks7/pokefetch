//! End-to-end tests that run the real `pokefetch` executable.
//!
//! Cargo builds the binary before running these and exposes its path as
//! `CARGO_BIN_EXE_<name>`, so no path guessing or `cargo build` shelling is
//! needed.
//!
//! These deliberately stick to subcommands that never touch the network, so
//! the suite passes on a machine with no connection.

use std::process::Command;

/// Runs the executable with `args` and returns (exit ok, stdout, stderr).
fn run(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_pokefetch"))
        .args(args)
        // Point every XDG path at a directory that does not exist, so the test
        // can never read or write the developer's real config and cache.
        .env("XDG_CONFIG_HOME", "/nonexistent/pokefetch-test")
        .env("XDG_CACHE_HOME", "/nonexistent/pokefetch-test")
        .env("XDG_STATE_HOME", "/nonexistent/pokefetch-test")
        .output()
        .expect("failed to run the pokefetch executable");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn help_lists_every_subcommand() {
    let (ok, stdout, _) = run(&["--help"]);
    assert!(ok);
    for command in [
        "greet", "show", "palette", "icon", "sprite", "render", "bundle",
    ] {
        assert!(stdout.contains(command), "--help omitted {command}");
    }
}

#[test]
fn every_global_flag_is_documented_in_help() {
    // clap builds help text from the doc comments in src/cli.rs. A flag with
    // no description is a flag nobody can discover.
    let (ok, stdout, _) = run(&["--help"]);
    assert!(ok);
    let lines = stdout.lines().collect::<Vec<_>>();
    for flag in [
        "--game",
        "--variant",
        "--size",
        "--alignment",
        "--background",
    ] {
        let index = lines
            .iter()
            .position(|line| line.trim_start().starts_with(flag))
            .unwrap_or_else(|| panic!("{flag} missing from --help"));
        // Doc comments with more than one paragraph make clap use its long
        // layout, where the description sits on the following line.
        let description = lines.get(index + 1).copied().unwrap_or_default();
        assert!(!description.trim().is_empty(), "{flag} has no help text");
    }
}

#[test]
fn version_matches_the_package_version() {
    let (ok, stdout, _) = run(&["--version"]);
    assert!(ok);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn bundle_reports_the_compiled_profile() {
    let (ok, stdout, _) = run(&["bundle"]);
    assert!(ok);
    // Without a bundle feature this is the string build.rs emits for stubs.
    assert!(!stdout.trim().is_empty());
}

#[test]
fn an_unknown_subcommand_fails_with_a_message() {
    let (ok, _, stderr) = run(&["definitely-not-a-command"]);
    assert!(!ok, "an unknown subcommand must not exit 0");
    assert!(!stderr.is_empty());
}

#[test]
fn an_out_of_range_size_is_rejected_before_any_work() {
    let (ok, _, stderr) = run(&["--size", "99", "sprite", "pikachu"]);
    assert!(!ok);
    assert!(
        stderr.contains("display.size"),
        "expected a size error, got: {stderr}"
    );
}

#[test]
fn an_unknown_game_names_the_offending_setting() {
    let (ok, _, stderr) = run(&["--game", "not-a-game", "sprite", "pikachu"]);
    assert!(!ok);
    assert!(
        stderr.contains("sprites.game"),
        "expected a game error, got: {stderr}"
    );
}

#[test]
fn conflicting_flags_are_rejected_by_the_parser() {
    let (ok, _, stderr) = run(&["--icon", "--no-icon"]);
    assert!(!ok);
    assert!(stderr.contains("cannot be used with"));
}
