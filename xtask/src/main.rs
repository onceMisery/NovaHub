#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use novahub_plugin_manager::{ArchiveLimits, pack_directory};

mod benchmark;
mod resource;

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let root = project_root();
    match command.as_str() {
        "release-check" => release_check(&root),
        "resource-report" => {
            resource::resource_report(&std::env::args().skip(2).collect::<Vec<_>>())
        }
        "reference-benchmark" => {
            benchmark::reference_benchmark(&root, &std::env::args().skip(2).collect::<Vec<_>>())
        }
        "build-examples" => build_examples(&root),
        "capability-e2e" => capability_e2e(&root),
        _ => {
            eprintln!(
                "usage: cargo run -p xtask -- release-check|resource-report|reference-benchmark|build-examples|capability-e2e"
            );
            ExitCode::from(2)
        }
    }
}

fn capability_e2e(root: &Path) -> ExitCode {
    let fixture_manifest = root.join("tests/fixtures/component-plugin/Cargo.toml");
    let fixture_built = std::process::Command::new("cargo")
        .current_dir(root)
        .args(["build", "--manifest-path"])
        .arg(&fixture_manifest)
        .args(["--target", "wasm32-wasip2", "--offline"])
        .status()
        .is_ok_and(|status| status.success());
    if !fixture_built {
        eprintln!("capability E2E failed to build the Component fixture");
        return ExitCode::from(1);
    }

    let host_built = std::process::Command::new("cargo")
        .current_dir(root)
        .args(["build", "-p", "novahub-plugin-host", "--offline"])
        .status()
        .is_ok_and(|status| status.success());
    if !host_built {
        eprintln!("capability E2E failed to build Plugin Host");
        return ExitCode::from(1);
    }

    let host = root.join("target/debug").join(format!(
        "novahub-plugin-host{}",
        std::env::consts::EXE_SUFFIX
    ));
    let fixture = root.join("target/wasm32-wasip2/debug/novahub_component_fixture.wasm");
    if !host.is_file() || !fixture.is_file() {
        eprintln!("capability E2E artifacts were not produced at the expected paths");
        return ExitCode::from(1);
    }

    let passed = std::process::Command::new("cargo")
        .current_dir(root)
        .args([
            "test",
            "-p",
            "novahub-app",
            "--offline",
            "capability_component_",
            "--",
            "--test-threads=1",
        ])
        .env("NOVAHUB_PLUGIN_HOST", host)
        .env("NOVAHUB_COMPONENT_FIXTURE", fixture)
        .status()
        .is_ok_and(|status| status.success());
    if passed {
        println!("capability-e2e: PASS");
        ExitCode::SUCCESS
    } else {
        eprintln!("capability-e2e: FAIL");
        ExitCode::from(1)
    }
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is below the project root")
        .to_path_buf()
}

fn release_check(root: &Path) -> ExitCode {
    let required_files = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "apps/novahub-app/Cargo.toml",
        "apps/novahub-plugin-host/Cargo.toml",
        "crates/plugin-runtime/Cargo.toml",
        "crates/plugin-manager/Cargo.toml",
        "wit/novahub-plugin/plugin.wit",
        "plugins/official/json-toolkit/novahub.toml",
        "plugins/official/translate/novahub.toml",
        "plugins/official/pets/nova/novahub.toml",
        "plugins/official/pets/pixel/novahub.toml",
        "plugins/official/pets/waterman/novahub.toml",
        "plugins/official/pets/nova/pet.json",
        "plugins/official/pets/nova/assets/nova-idle.svg",
        "plugins/official/pets/pixel/pet.json",
        "plugins/official/pets/pixel/assets/pixel-idle.svg",
        "plugins/official/pets/waterman/pet.json",
        "plugins/official/pets/waterman/assets/waterman-idle.svg",
        "docs/08-quality-release.md",
        ".github/workflows/ci.yml",
    ];
    let mut failures = Vec::new();
    for relative in required_files {
        if root.join(relative).is_file() {
            println!("PASS file {relative}");
        } else {
            println!("FAIL file {relative}");
            failures.push(relative);
        }
    }

    let app_manifest = std::fs::read_to_string(root.join("apps/novahub-app/Cargo.toml"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    for forbidden in ["wasmtime", "tauri", "webview", "plugin-runtime"] {
        if app_manifest.contains(forbidden) {
            println!("FAIL app dependency boundary contains {forbidden}");
            failures.push(forbidden);
        }
    }
    if !app_manifest.is_empty() {
        println!("PASS app dependency boundary");
    }

    let ci = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap_or_default();
    let ci_has_cli_source =
        ci.contains("target/release/novahub-cli") && ci.contains("target/release/novahub-cli.exe");
    let ci_has_stale_cli_source = ci.contains("target/release/novahub dist/novahub/")
        || ci.contains("target/release/novahub.exe dist/novahub/");
    if ci_has_cli_source && !ci_has_stale_cli_source {
        println!("PASS release artifact names match novahub-cli");
    } else {
        println!("FAIL release artifact names do not match built CLI binaries");
        failures.push("release-artifact-binary");
    }
    if ci.contains("cargo run -p xtask -- capability-e2e") {
        println!("PASS CI runs the real capability Component E2E");
    } else {
        println!("FAIL CI does not run the real capability Component E2E");
        failures.push("capability-component-e2e");
    }

    for retired in [
        "docs/12-technical-review.md",
        "docs/13-technical-analysis-report.md",
    ] {
        if root.join(retired).exists() {
            println!("FAIL retired document still exists {retired}");
            failures.push(retired);
        } else {
            println!("PASS retired document absent {retired}");
        }
    }

    let strict = std::env::args().any(|arg| arg == "--strict-platform");
    let platform_evidence = std::env::var_os("NOVAHUB_RELEASE_PLATFORM_EVIDENCE").is_some();
    let resource_evidence = std::env::var_os("NOVAHUB_RELEASE_RESOURCE_EVIDENCE").is_some();
    if strict && !platform_evidence {
        println!("FAIL strict platform evidence is missing");
        failures.push("platform-evidence");
    } else {
        println!("INFO platform evidence deferred to reference-machine CI");
    }
    if strict && !resource_evidence {
        println!("FAIL strict resource evidence is missing");
        failures.push("resource-evidence");
    } else {
        println!("INFO RSS/GPU evidence deferred to reference-machine CI");
    }

    if failures.is_empty() {
        println!("release-check: PASS (static checks; platform/resource evidence may be deferred)");
        ExitCode::SUCCESS
    } else {
        eprintln!("release-check: FAIL ({} checks)", failures.len());
        ExitCode::from(1)
    }
}

fn build_examples(root: &Path) -> ExitCode {
    let examples = [
        (
            "json-toolkit",
            "plugins/official/json-toolkit",
            "novahub_official_json_toolkit_component.wasm",
            "official.json-toolkit",
        ),
        (
            "translate",
            "plugins/official/translate",
            "novahub_official_translate_component.wasm",
            "official.translate",
        ),
    ];
    let staging_root = root.join("target/novahub-example-packages");
    let output_root = root.join("dist/plugins");
    if let Err(error) = std::fs::create_dir_all(&staging_root) {
        eprintln!("failed to create example staging directory: {error}");
        return ExitCode::from(1);
    }
    if let Err(error) = std::fs::create_dir_all(&output_root) {
        eprintln!("failed to create example output directory: {error}");
        return ExitCode::from(1);
    }

    for (name, manifest_dir, artifact_name, package_id) in examples {
        let manifest_dir = root.join(manifest_dir);
        let manifest_path = manifest_dir.join("Cargo.toml");
        let status = std::process::Command::new("cargo")
            .args([
                "build",
                "--manifest-path",
                manifest_path.to_string_lossy().as_ref(),
                "--target",
                "wasm32-wasip2",
                "--offline",
            ])
            .status();
        if !matches!(status, Ok(status) if status.success()) {
            eprintln!("failed to build official example {name}");
            return ExitCode::from(1);
        }

        let staging = staging_root.join(package_id);
        if let Err(error) = std::fs::create_dir_all(&staging) {
            eprintln!("failed to create staging directory for {name}: {error}");
            return ExitCode::from(1);
        }
        let manifest = match std::fs::read(manifest_dir.join("novahub.toml")) {
            Ok(manifest) => manifest,
            Err(error) => {
                eprintln!("failed to read {name} manifest: {error}");
                return ExitCode::from(1);
            }
        };
        if let Err(error) = std::fs::write(staging.join("novahub.toml"), manifest) {
            eprintln!("failed to stage {name} manifest: {error}");
            return ExitCode::from(1);
        }
        let artifact = root
            .join("target/wasm32-wasip2/debug/deps")
            .join(artifact_name);
        if let Err(error) = std::fs::copy(&artifact, staging.join("plugin.wasm")) {
            eprintln!(
                "failed to stage {name} Component {}: {error}",
                artifact.display()
            );
            return ExitCode::from(1);
        }
        let archive = output_root.join(format!("{package_id}.novahub-plugin"));
        if let Err(error) = pack_directory(&staging, &archive, ArchiveLimits::default()) {
            eprintln!("failed to pack {name}: {error:?}");
            return ExitCode::from(1);
        }
        println!("built {name}: {}", archive.display());
    }

    for (name, package_dir, package_id) in [
        ("Nova", "plugins/official/pets/nova", "official.pet.nova"),
        ("Pixel", "plugins/official/pets/pixel", "official.pet.pixel"),
        (
            "Waterman",
            "plugins/official/pets/waterman",
            "official.pet.waterman",
        ),
    ] {
        let source = root.join(package_dir);
        let staging = staging_root.join(package_id);
        if let Err(error) = copy_directory(&source, &staging) {
            eprintln!("failed to stage pet {name}: {error}");
            return ExitCode::from(1);
        }
        let archive = output_root.join(format!("{package_id}.novahub-plugin"));
        if let Err(error) = pack_directory(&staging, &archive, ArchiveLimits::default()) {
            eprintln!("failed to pack pet {name}: {error:?}");
            return ExitCode::from(1);
        }
        println!("built pet {name}: {}", archive.display());
    }
    ExitCode::SUCCESS
}

fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}
