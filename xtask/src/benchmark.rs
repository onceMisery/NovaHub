use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::resource::{ResourceSnapshot, collect_resource_snapshot};

const DEFAULT_SAMPLES: usize = 20;
const DEFAULT_WARMUP: usize = 3;
const DEFAULT_RESOURCE_HOLD_MS: u64 = 15_000;
const MAX_SAMPLES: usize = 100;
const MAX_WARMUP: usize = 20;
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_DIAGNOSTIC_TEXT: usize = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "debug" => Ok(Self::Debug),
            "release" => Ok(Self::Release),
            _ => Err("--profile must be debug or release".to_owned()),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    const fn cargo_flag(self) -> Option<&'static str> {
        match self {
            Self::Debug => None,
            Self::Release => Some("--release"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Scenario {
    Headless,
    Shell,
    OnePlugin,
    FourPlugins,
}

impl Scenario {
    fn parse_many(value: &str) -> Result<Vec<Self>, String> {
        if value == "all" {
            return Ok(vec![
                Self::Headless,
                Self::Shell,
                Self::OnePlugin,
                Self::FourPlugins,
            ]);
        }
        let mut scenarios = value
            .split(',')
            .map(|value| match value {
                "headless" => Ok(Self::Headless),
                "shell" => Ok(Self::Shell),
                "one-plugin" => Ok(Self::OnePlugin),
                "four-plugins" => Ok(Self::FourPlugins),
                _ => Err(format!("unknown reference scenario: {value}")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        scenarios.sort_unstable();
        scenarios.dedup();
        if scenarios.is_empty() {
            return Err("--scenario must not be empty".to_owned());
        }
        Ok(scenarios)
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Headless => "headless-bootstrap",
            Self::Shell => "shell-first-frame",
            Self::OnePlugin => "one-plugin",
            Self::FourPlugins => "four-plugins",
        }
    }

    const fn marker(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Headless => None,
            Self::Shell => Some(("NOVAHUB_SMOKE ", "first_frame")),
            Self::OnePlugin | Self::FourPlugins => Some(("NOVAHUB_HEADLESS ", "sessions_ready")),
        }
    }

    const fn plugin_sessions(self) -> Option<usize> {
        match self {
            Self::OnePlugin => Some(1),
            Self::FourPlugins => Some(4),
            Self::Headless | Self::Shell => None,
        }
    }

    const fn supports_resource_probe(self) -> bool {
        !matches!(self, Self::Headless)
    }
}

#[derive(Debug)]
struct BenchmarkConfig {
    profile: BuildProfile,
    scenarios: Vec<Scenario>,
    samples: usize,
    warmup: usize,
    resource_hold: Duration,
    skip_build: bool,
    output: Option<PathBuf>,
}

impl BenchmarkConfig {
    fn parse(args: &[String]) -> Result<Self, String> {
        let profile = option_value(args, "--profile")
            .as_deref()
            .map(BuildProfile::parse)
            .transpose()?
            .unwrap_or(BuildProfile::Release);
        let scenarios = option_value(args, "--scenario")
            .as_deref()
            .map(Scenario::parse_many)
            .transpose()?
            .unwrap_or_else(|| Scenario::parse_many("all").expect("all scenarios are valid"));
        let samples = parse_bounded_usize(args, "--samples", DEFAULT_SAMPLES, 1, MAX_SAMPLES)?;
        let warmup = parse_bounded_usize(args, "--warmup", DEFAULT_WARMUP, 0, MAX_WARMUP)?;
        let resource_hold_ms = parse_bounded_u64(
            args,
            "--resource-hold-ms",
            DEFAULT_RESOURCE_HOLD_MS,
            1_000,
            60_000,
        )?;
        Ok(Self {
            profile,
            scenarios,
            samples,
            warmup,
            resource_hold: Duration::from_millis(resource_hold_ms),
            skip_build: args.iter().any(|arg| arg == "--skip-build"),
            output: option_value(args, "--output").map(PathBuf::from),
        })
    }
}

#[derive(Debug)]
struct Artifacts {
    app: PathBuf,
    host: PathBuf,
    component: PathBuf,
    data_dir: PathBuf,
}

#[derive(Debug)]
struct TimingSample {
    measured_ms: f64,
    total_ms: f64,
    marker: Option<serde_json::Value>,
    stderr: String,
}

impl TimingSample {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "measured_ms": self.measured_ms,
            "total_ms": self.total_ms,
            "marker": self.marker,
            "stderr": self.stderr,
        })
    }
}

pub fn reference_benchmark(root: &Path, args: &[String]) -> ExitCode {
    let config = match BenchmarkConfig::parse(args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("reference-benchmark configuration failed: {error}");
            return ExitCode::from(2);
        }
    };
    if !config.skip_build
        && let Err(error) = build_artifacts(root, &config)
    {
        eprintln!("reference-benchmark build failed: {error}");
        return ExitCode::from(1);
    }
    let artifacts = artifact_paths(root, config.profile);
    if let Err(error) = validate_artifacts(&artifacts, &config.scenarios) {
        eprintln!("reference-benchmark artifacts are unavailable: {error}");
        return ExitCode::from(1);
    }
    if let Err(error) = std::fs::create_dir_all(&artifacts.data_dir) {
        eprintln!("reference-benchmark data directory failed: {error}");
        return ExitCode::from(1);
    }

    let mut scenario_reports = serde_json::Map::new();
    for scenario in &config.scenarios {
        println!("reference-benchmark: running {}", scenario.name());
        let report = match run_scenario(*scenario, &config, &artifacts) {
            Ok(report) => report,
            Err(error) => {
                eprintln!("reference-benchmark {} failed: {error}", scenario.name());
                return ExitCode::from(1);
            }
        };
        scenario_reports.insert(scenario.name().to_owned(), report);
    }

    let report = serde_json::json!({
        "schema_version": 2,
        "generated_unix_ms": unix_milliseconds(),
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "profile": config.profile.name(),
        "samples": config.samples,
        "warmup": config.warmup,
        "rendering": rendering_metadata(),
        "source": source_metadata(root),
        "toolchain": toolchain_metadata(),
        "artifacts": artifact_metadata(&artifacts),
        "scenarios": scenario_reports,
    });
    let output = config.output.unwrap_or_else(|| {
        root.join("target/reference-benchmark").join(format!(
            "{}-{}-{}.json",
            std::env::consts::OS,
            std::env::consts::ARCH,
            config.profile.name()
        ))
    });
    if let Err(error) = write_report(&output, &report) {
        eprintln!("reference-benchmark report failed: {error}");
        return ExitCode::from(1);
    }
    println!("reference-benchmark: PASS ({})", output.display());
    ExitCode::SUCCESS
}

fn run_scenario(
    scenario: Scenario,
    config: &BenchmarkConfig,
    artifacts: &Artifacts,
) -> Result<serde_json::Value, String> {
    for _ in 0..config.warmup {
        run_timing_sample(scenario, artifacts)?;
    }
    let samples = (0..config.samples)
        .map(|_| run_timing_sample(scenario, artifacts))
        .collect::<Result<Vec<_>, _>>()?;
    let measured = samples
        .iter()
        .map(|sample| sample.measured_ms)
        .collect::<Vec<_>>();
    let resource_probe = if scenario.supports_resource_probe() {
        Some(run_resource_probe(
            scenario,
            artifacts,
            config.resource_hold,
        )?)
    } else {
        None
    };
    Ok(serde_json::json!({
        "timing": timing_statistics(&measured),
        "sample_results": samples.iter().map(TimingSample::to_json).collect::<Vec<_>>(),
        "resource_probe": resource_probe,
    }))
}

fn run_timing_sample(scenario: Scenario, artifacts: &Artifacts) -> Result<TimingSample, String> {
    let mut command = app_command(scenario, artifacts, Duration::ZERO);
    let started = Instant::now();
    let output = command.output().map_err(|error| error.to_string())?;
    let total_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = bounded_text(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        return Err(format!(
            "process exited with {:?}; stdout: {}; stderr: {}",
            output.status.code(),
            bounded_text(&stdout),
            stderr,
        ));
    }
    if scenario != Scenario::Shell && !stdout.contains("NovaHub host bootstrap") {
        return Err("process output did not contain the bootstrap marker".to_owned());
    }
    let marker = scenario
        .marker()
        .map(|(prefix, event)| parse_marker(&stdout, prefix, event))
        .transpose()?;
    let measured_ms = marker
        .as_ref()
        .and_then(|marker| marker.get("elapsed_ms"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(total_ms);
    Ok(TimingSample {
        measured_ms,
        total_ms,
        marker,
        stderr,
    })
}

fn rendering_metadata() -> serde_json::Value {
    let requested =
        std::env::var_os("SLINT_BACKEND").map(|value| value.to_string_lossy().into_owned());
    let effective = requested.clone().unwrap_or_else(|| {
        if cfg!(windows) {
            "winit-software".to_owned()
        } else {
            "slint-default".to_owned()
        }
    });
    serde_json::json!({
        "requested_slint_backend": requested,
        "effective_policy": effective,
        "default_policy": if cfg!(windows) { "winit-software" } else { "slint-default" },
        "note": "SLINT_BACKEND overrides the default policy for diagnostic comparison"
    })
}

fn run_resource_probe(
    scenario: Scenario,
    artifacts: &Artifacts,
    hold: Duration,
) -> Result<serde_json::Value, String> {
    let (prefix, event) = scenario
        .marker()
        .ok_or_else(|| format!("{} has no ready marker", scenario.name()))?;
    let mut command = app_command(scenario, artifacts, hold);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "reference process stdout was not piped".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "reference process stderr was not piped".to_owned())?;
    let (marker_sender, marker_receiver) = mpsc::channel();
    let prefix = prefix.to_owned();
    let stdout_reader = std::thread::spawn(move || {
        let mut lines = Vec::new();
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.starts_with(&prefix) {
                let _ = marker_sender.send(line.clone());
            }
            lines.push(line);
        }
        lines
    });
    let stderr_reader = std::thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>()
    });

    let marker_line = marker_receiver
        .recv_timeout(READY_TIMEOUT)
        .map_err(|error| format!("ready marker timed out: {error}"))?;
    let marker = parse_marker(&marker_line, &prefix_for_scenario(scenario), event)?;
    let snapshot = collect_resource_snapshot(child.id())?;
    let status = child.wait().map_err(|error| error.to_string())?;
    let stdout_lines = stdout_reader
        .join()
        .map_err(|_| "reference stdout reader panicked".to_owned())?;
    let stderr_lines = stderr_reader
        .join()
        .map_err(|_| "reference stderr reader panicked".to_owned())?;
    if !status.success() {
        return Err(format!(
            "resource probe exited with {:?}: {}",
            status.code(),
            bounded_text(&stdout_lines.join("\n"))
        ));
    }
    Ok(resource_probe_json(
        &marker,
        &snapshot,
        &bounded_text(&stderr_lines.join("\n")),
    ))
}

fn resource_probe_json(
    marker: &serde_json::Value,
    snapshot: &ResourceSnapshot,
    stderr: &str,
) -> serde_json::Value {
    serde_json::json!({
        "ready_marker": marker,
        "snapshot": snapshot.to_json(),
        "stderr": stderr,
    })
}

fn app_command(scenario: Scenario, artifacts: &Artifacts, hold: Duration) -> Command {
    let mut command = Command::new(&artifacts.app);
    command.current_dir(
        artifacts
            .app
            .ancestors()
            .nth(3)
            .unwrap_or_else(|| Path::new(".")),
    );
    match scenario {
        Scenario::Headless => {
            command.env("NOVAHUB_HEADLESS", "1");
        }
        Scenario::Shell => {
            command
                .env("NOVAHUB_DATA_DIR", &artifacts.data_dir)
                .env("NOVAHUB_SMOKE_HOLD_MS", hold.as_millis().to_string());
        }
        Scenario::OnePlugin | Scenario::FourPlugins => {
            command
                .env("NOVAHUB_HEADLESS", "1")
                .env("NOVAHUB_DATA_DIR", &artifacts.data_dir)
                .env("NOVAHUB_PLUGIN_HOST", &artifacts.host)
                .env("NOVAHUB_COMPONENT_FIXTURE", &artifacts.component)
                .env(
                    "NOVAHUB_HEADLESS_PLUGIN_SESSIONS",
                    scenario
                        .plugin_sessions()
                        .expect("plugin scenario has session count")
                        .to_string(),
                )
                .env("NOVAHUB_HEADLESS_HOLD_MS", hold.as_millis().to_string());
        }
    }
    command
}

fn parse_marker(
    output: &str,
    prefix: &str,
    expected_event: &str,
) -> Result<serde_json::Value, String> {
    let marker = output
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .ok_or_else(|| format!("missing {expected_event} marker"))?;
    let value: serde_json::Value =
        serde_json::from_str(marker).map_err(|error| error.to_string())?;
    if value.get("event").and_then(serde_json::Value::as_str) != Some(expected_event) {
        return Err(format!("unexpected marker event: {value}"));
    }
    Ok(value)
}

fn prefix_for_scenario(scenario: Scenario) -> String {
    scenario
        .marker()
        .map_or_else(String::new, |(prefix, _)| prefix.to_owned())
}

fn timing_statistics(samples: &[f64]) -> serde_json::Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let sample_count = u32::try_from(sorted.len()).expect("sample count is bounded");
    let mean = sorted.iter().sum::<f64>() / f64::from(sample_count);
    serde_json::json!({
        "count": sorted.len(),
        "min_ms": sorted[0],
        "mean_ms": mean,
        "p50_ms": nearest_rank(&sorted, 50, 100),
        "p95_ms": nearest_rank(&sorted, 95, 100),
        "p99_ms": nearest_rank(&sorted, 99, 100),
        "max_ms": sorted[sorted.len() - 1],
    })
}

fn nearest_rank(sorted: &[f64], numerator: usize, denominator: usize) -> f64 {
    let rank = sorted.len().saturating_mul(numerator).div_ceil(denominator);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn build_artifacts(root: &Path, config: &BenchmarkConfig) -> Result<(), String> {
    let mut app_build = Command::new("cargo");
    app_build.current_dir(root).args([
        "build",
        "-p",
        "novahub-app",
        "-p",
        "novahub-plugin-host",
        "--offline",
    ]);
    if let Some(flag) = config.profile.cargo_flag() {
        app_build.arg(flag);
    }
    run_command(&mut app_build, "App and Plugin Host build")?;

    if config
        .scenarios
        .iter()
        .any(|scenario| scenario.plugin_sessions().is_some())
    {
        let mut component_build = Command::new("cargo");
        component_build
            .current_dir(root)
            .args(["build", "--manifest-path"])
            .arg(root.join("tests/fixtures/component-plugin/Cargo.toml"))
            .args(["--target", "wasm32-wasip2", "--offline"]);
        if let Some(flag) = config.profile.cargo_flag() {
            component_build.arg(flag);
        }
        run_command(&mut component_build, "Component fixture build")?;
    }
    Ok(())
}

fn run_command(command: &mut Command, label: &str) -> Result<(), String> {
    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} exited with {:?}", status.code()))
    }
}

fn artifact_paths(root: &Path, profile: BuildProfile) -> Artifacts {
    let executable_suffix = std::env::consts::EXE_SUFFIX;
    Artifacts {
        app: root
            .join("target")
            .join(profile.name())
            .join(format!("novahub-app{executable_suffix}")),
        host: root
            .join("target")
            .join(profile.name())
            .join(format!("novahub-plugin-host{executable_suffix}")),
        component: root
            .join("target/wasm32-wasip2")
            .join(profile.name())
            .join("novahub_component_fixture.wasm"),
        data_dir: root
            .join("target/reference-benchmark/data")
            .join(profile.name()),
    }
}

fn validate_artifacts(artifacts: &Artifacts, scenarios: &[Scenario]) -> Result<(), String> {
    if !artifacts.app.is_file() {
        return Err(format!("App is missing: {}", artifacts.app.display()));
    }
    if scenarios
        .iter()
        .any(|scenario| scenario.plugin_sessions().is_some())
    {
        if !artifacts.host.is_file() {
            return Err(format!(
                "Plugin Host is missing: {}",
                artifacts.host.display()
            ));
        }
        if !artifacts.component.is_file() {
            return Err(format!(
                "Component fixture is missing: {}",
                artifacts.component.display()
            ));
        }
    }
    Ok(())
}

fn artifact_metadata(artifacts: &Artifacts) -> serde_json::Value {
    serde_json::json!({
        "app": file_metadata(&artifacts.app),
        "plugin_host": file_metadata(&artifacts.host),
        "component_fixture": file_metadata(&artifacts.component),
    })
}

fn file_metadata(path: &Path) -> serde_json::Value {
    let metadata = std::fs::metadata(path).ok();
    serde_json::json!({
        "path": path,
        "bytes": metadata.as_ref().map(std::fs::Metadata::len),
        "modified_unix_ms": metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .and_then(|duration| u64::try_from(duration.as_millis()).ok()),
    })
}

fn source_metadata(root: &Path) -> serde_json::Value {
    let head = command_text(root, "git", &["rev-parse", "HEAD"]);
    let status = command_text(root, "git", &["status", "--porcelain"]);
    serde_json::json!({
        "git_head": head,
        "dirty": status.as_ref().is_none_or(|status| !status.is_empty()),
    })
}

fn toolchain_metadata() -> serde_json::Value {
    serde_json::json!({
        "rustc": command_text(Path::new("."), "rustc", &["--version"]),
        "cargo": command_text(Path::new("."), "cargo", &["--version"]),
    })
}

fn command_text(root: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn write_report(path: &Path, report: &serde_json::Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "reference report path has no parent".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn parse_bounded_usize(
    args: &[String],
    name: &str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize, String> {
    let Some(value) = option_value(args, name) else {
        return Ok(default);
    };
    let value = value
        .parse::<usize>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name} must be between {minimum} and {maximum}"));
    }
    Ok(value)
}

fn parse_bounded_u64(
    args: &[String],
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let Some(value) = option_value(args, name) else {
        return Ok(default);
    };
    let value = value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name} must be between {minimum} and {maximum}"));
    }
    Ok(value)
}

fn bounded_text(value: &str) -> String {
    value.chars().take(MAX_DIAGNOSTIC_TEXT).collect()
}

fn unix_milliseconds() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::{Scenario, nearest_rank, parse_marker};

    #[test]
    fn nearest_rank_uses_observed_samples() {
        let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((nearest_rank(&samples, 50, 100) - 3.0).abs() < f64::EPSILON);
        assert!((nearest_rank(&samples, 95, 100) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scenario_parser_deduplicates_requested_scenarios() {
        assert_eq!(
            Scenario::parse_many("shell,one-plugin,shell"),
            Ok(vec![Scenario::Shell, Scenario::OnePlugin])
        );
    }

    #[test]
    fn marker_parser_requires_the_expected_event() {
        let marker = parse_marker(
            "NOVAHUB_SMOKE {\"event\":\"first_frame\",\"elapsed_ms\":12}",
            "NOVAHUB_SMOKE ",
            "first_frame",
        )
        .expect("valid marker");
        assert_eq!(marker["elapsed_ms"], 12);
    }
}
