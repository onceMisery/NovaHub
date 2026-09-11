use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use novahub_migration::{SourceKind, parse_export};
use novahub_plugin_manager::{
    ArchiveLimits, DeclaredPermissions, PermissionChange, PermissionScope, PluginInteraction,
    PluginManifestSummary, UninstallOutcome, diff_declared_permissions, inspect_archive,
    install_archive, install_archive_verified, is_safe_plugin_id, pack_directory,
    read_active_manifest, retry_pending_delete_ids, rollback_plugin, set_plugin_enabled,
    sign_archive, uninstall_plugin_with_pending_delete,
};
use novahub_storage::{MigrationRecord, Storage};

const DEFAULT_HOST_VERSION: &str = "1.6.0";
const PLUGIN_WIT: &str = include_str!("../../../wit/novahub-plugin/plugin.wit");

fn main() -> ExitCode {
    let args: Vec<_> = env::args().collect();
    if args.get(1).map(String::as_str) != Some("migrate") {
        return if args.get(1).map(String::as_str) == Some("plugin") {
            plugin_command(&args)
        } else {
            println!("NovaHub CLI: migrate or plugin install");
            ExitCode::SUCCESS
        };
    }

    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let apply = args.iter().any(|arg| arg == "--apply");
    if dry_run == apply {
        eprintln!("usage: novahub migrate --dry-run|--apply <export.json>");
        return ExitCode::from(2);
    }
    let source = match option_value(&args, "--source").as_deref() {
        Some("utools") => SourceKind::UTools,
        Some("raycast") | None => SourceKind::Raycast,
        Some(value) => {
            eprintln!("unsupported migration source: {value}");
            return ExitCode::from(2);
        }
    };
    let path = migration_input_path(&args);
    let Some(path) = path else {
        eprintln!("usage: novahub migrate --dry-run|--apply <export.json>");
        return ExitCode::from(2);
    };
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("failed to read {path}: {error}");
            return ExitCode::from(1);
        }
    };
    match parse_export(&input, source) {
        Ok(report) => {
            if dry_run {
                println!(
                    "dry-run: accepted={}, rejected={}, writes=0",
                    report.accepted.len(),
                    report.rejected.len()
                );
            } else {
                let db_path =
                    option_value(&args, "--db").unwrap_or_else(|| "novahub.sqlite3".to_owned());
                let storage = match Storage::open(&db_path) {
                    Ok(storage) => storage,
                    Err(error) => {
                        eprintln!("failed to open migration database {db_path}: {error}");
                        return ExitCode::from(1);
                    }
                };
                let records = report
                    .accepted
                    .iter()
                    .enumerate()
                    .map(|(index, item)| MigrationRecord {
                        kind: item.kind.clone(),
                        id: migration_id(&item.kind, &item.title, index),
                        title: item.title.clone(),
                        content: item.content.clone(),
                    })
                    .collect::<Vec<_>>();
                match storage.import_migration_records(&records) {
                    Ok(writes) => println!(
                        "apply: accepted={}, rejected={}, writes={}, db={db_path}",
                        report.accepted.len(),
                        report.rejected.len(),
                        writes,
                    ),
                    Err(error) => {
                        eprintln!("migration transaction rolled back: {error}");
                        return ExitCode::from(1);
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("invalid export JSON: {error}");
            ExitCode::from(1)
        }
    }
}

fn migration_id(kind: &str, title: &str, index: usize) -> String {
    let mut slug = title
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if slug.is_empty() {
        slug = "item".into();
    }
    format!("migration-{kind}-{index}-{slug}")
}

#[allow(clippy::too_many_lines)]
fn plugin_command(args: &[String]) -> ExitCode {
    let Some(operation) = args.get(2).map(String::as_str) else {
        print_plugin_usage();
        return ExitCode::from(2);
    };
    let root = option_value(args, "--root").unwrap_or_else(|| "plugins".into());
    if operation == "new" {
        return plugin_new(args);
    }
    if operation == "dev" {
        return plugin_dev(args);
    }
    if operation == "check" {
        return plugin_check(args, false);
    }
    if operation == "test" {
        return plugin_check(args, true);
    }
    if operation == "pack" {
        let Some(source) = positional_arg(args, 0) else {
            eprintln!("missing plugin source directory");
            return ExitCode::from(2);
        };
        let output =
            option_value(args, "--output").unwrap_or_else(|| format!("{source}.novahub-plugin"));
        return match pack_directory(source, output, ArchiveLimits::default()) {
            Ok(()) => {
                println!("packed plugin archive");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("plugin pack failed: {error:?}");
                ExitCode::from(1)
            }
        };
    }
    if operation == "sign" {
        let Some(archive_path) = positional_arg(args, 0) else {
            eprintln!("missing plugin archive path");
            return ExitCode::from(2);
        };
        let Some(seed_path) = option_value(args, "--seed") else {
            eprintln!("missing --seed file containing exactly 32 raw bytes");
            return ExitCode::from(2);
        };
        let archive = match fs::read(&archive_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("failed to read plugin archive: {error}");
                return ExitCode::from(1);
            }
        };
        let seed = match fs::read(seed_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("failed to read signing seed: {error}");
                return ExitCode::from(1);
            }
        };
        let Ok(seed) = <[u8; 32]>::try_from(seed) else {
            eprintln!("signing seed must contain exactly 32 raw bytes");
            return ExitCode::from(2);
        };
        return match sign_archive(&archive, &seed) {
            Ok((signature, public_key)) => {
                let signature_path = format!("{archive_path}.ed25519");
                let public_key_path = format!("{archive_path}.pub");
                if let Err(error) = fs::write(&signature_path, signature)
                    .and_then(|()| fs::write(&public_key_path, public_key))
                {
                    eprintln!("failed to write signature files: {error}");
                    ExitCode::from(1)
                } else {
                    println!("signed plugin: {signature_path}, {public_key_path}");
                    ExitCode::SUCCESS
                }
            }
            Err(error) => {
                eprintln!("plugin signing failed: {error:?}");
                ExitCode::from(1)
            }
        };
    }
    if matches!(operation, "enable" | "disable") {
        let Some(plugin_id) = positional_arg(args, 0) else {
            eprintln!("missing plugin id");
            return ExitCode::from(2);
        };
        let enabled = operation == "enable";
        return match set_plugin_enabled(&root, &plugin_id, enabled) {
            Ok(active) => {
                println!("{} {} {}", operation, active.plugin_id, active.version);
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("plugin {operation} failed: {error:?}");
                ExitCode::from(1)
            }
        };
    }
    if operation == "retry-delete" {
        let db_path = plugin_database_path(args, Path::new(&root));
        let storage = match Storage::open(&db_path) {
            Ok(storage) => storage,
            Err(error) => {
                eprintln!(
                    "failed to open plugin authorization database {}: {error}",
                    db_path.display()
                );
                return ExitCode::from(1);
            }
        };
        let removed_ids = match retry_pending_deletes_with_authorization(Path::new(&root), &storage)
        {
            Ok(ids) => ids,
            Err(error) => {
                eprintln!("pending plugin deletion retry failed: {error}");
                return ExitCode::from(1);
            }
        };
        println!("retried {} pending plugin deletions", removed_ids.len());
        return ExitCode::SUCCESS;
    }
    if operation == "rollback" {
        let Some(plugin_id) = positional_arg(args, 0) else {
            eprintln!("missing plugin id");
            return ExitCode::from(2);
        };
        return match rollback_plugin(&root, &plugin_id) {
            Ok(active) => {
                println!("rolled back {} to {}", active.plugin_id, active.version);
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("plugin rollback failed: {error:?}");
                ExitCode::from(1)
            }
        };
    }
    if operation == "uninstall" {
        let Some(plugin_id) = positional_arg(args, 0) else {
            eprintln!("missing plugin id");
            return ExitCode::from(2);
        };
        let db_path = plugin_database_path(args, Path::new(&root));
        let storage = match Storage::open(&db_path) {
            Ok(storage) => storage,
            Err(error) => {
                eprintln!(
                    "failed to open plugin authorization database {}: {error}",
                    db_path.display()
                );
                return ExitCode::from(1);
            }
        };
        return match uninstall_plugin_with_authorization(Path::new(&root), &storage, &plugin_id) {
            Ok(UninstallOutcome::Removed) => {
                println!("uninstalled {plugin_id}");
                ExitCode::SUCCESS
            }
            Ok(UninstallOutcome::PendingDelete { marker }) => {
                println!(
                    "uninstall pending for {plugin_id}; retry marker={}",
                    marker.display()
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("plugin uninstall failed: {error}");
                ExitCode::from(1)
            }
        };
    }
    if !matches!(operation, "install" | "update") {
        print_plugin_usage();
        return ExitCode::from(2);
    }
    let Some(archive_path) = positional_arg(args, 0) else {
        eprintln!("missing plugin archive path");
        return ExitCode::from(2);
    };
    let host_version =
        option_value(args, "--host-version").unwrap_or_else(|| DEFAULT_HOST_VERSION.to_owned());
    let expected_hash = option_value(args, "--sha256");
    let signature_path = option_value(args, "--signature");
    let public_key_path = option_value(args, "--public-key");
    let allow_unsigned = args.iter().any(|arg| arg == "--allow-unsigned");
    let signature_files = match (signature_path, public_key_path) {
        (Some(signature_path), Some(public_key_path)) => {
            let signature = match fs::read(&signature_path) {
                Ok(bytes) if bytes.len() == 64 => bytes,
                Ok(bytes) => {
                    eprintln!(
                        "plugin signature must contain exactly 64 raw bytes (got {})",
                        bytes.len()
                    );
                    return ExitCode::from(2);
                }
                Err(error) => {
                    eprintln!("failed to read plugin signature: {error}");
                    return ExitCode::from(1);
                }
            };
            let public_key = match fs::read(&public_key_path) {
                Ok(bytes) if bytes.len() == 32 => bytes,
                Ok(bytes) => {
                    eprintln!(
                        "plugin public key must contain exactly 32 raw bytes (got {})",
                        bytes.len()
                    );
                    return ExitCode::from(2);
                }
                Err(error) => {
                    eprintln!("failed to read plugin public key: {error}");
                    return ExitCode::from(1);
                }
            };
            Some((public_key, signature))
        }
        (None, None) if allow_unsigned => None,
        (None, None) => {
            eprintln!(
                "plugin install requires --signature and --public-key; use --allow-unsigned only for development"
            );
            return ExitCode::from(2);
        }
        _ => {
            eprintln!("--signature and --public-key must be supplied together");
            return ExitCode::from(2);
        }
    };
    let archive = match fs::read(archive_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("failed to read plugin archive: {error}");
            return ExitCode::from(1);
        }
    };
    let preview = match inspect_archive(&archive, &host_version, ArchiveLimits::default()) {
        Ok(summary) => summary,
        Err(error) => {
            eprintln!("plugin install preview rejected: {error:?}");
            return ExitCode::from(1);
        }
    };
    let previous = match read_active_manifest(&root, &preview.id) {
        Ok(previous) => previous,
        Err(error) => {
            eprintln!("plugin install preview could not read active version: {error:?}");
            return ExitCode::from(1);
        }
    };
    let permission_diff = diff_declared_permissions(
        previous
            .as_ref()
            .map(novahub_plugin_manager::PluginManifest::declared_permissions),
        &preview.declared_permissions,
    );
    let permission_diff = permission_diff
        .iter()
        .map(|entry| {
            format!(
                "{}={} before={} after={} reason={}",
                entry.capability.as_str(),
                permission_change_label(entry.change),
                permission_scope_label(entry.before.as_ref()),
                permission_scope_label(entry.after.as_ref()),
                entry.reason.as_deref().unwrap_or("-")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let permissions = preview.permissions.join(",");
    let commands = preview
        .commands
        .iter()
        .map(|command| format!("{}:{:?}", command.id, command.interaction))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "install preview: name={:?} publisher={:?} version={} kind={:?} permissions={:?} permission_diff={:?} commands={:?}",
        preview.name,
        preview.publisher.as_deref().unwrap_or("unknown"),
        preview.version,
        preview.kind,
        permissions,
        permission_diff,
        commands,
    );
    let signature = signature_files
        .as_ref()
        .map(|(public_key, signature)| (public_key.as_slice(), signature.as_slice()));
    match install_archive_verified(
        &archive,
        root,
        &host_version,
        expected_hash.as_deref(),
        signature,
        ArchiveLimits::default(),
    ) {
        Ok(receipt) => {
            let permissions = receipt.summary.permissions.join(",");
            let commands = receipt
                .summary
                .commands
                .iter()
                .map(|command| format!("{}:{:?}", command.id, command.interaction))
                .collect::<Vec<_>>()
                .join(",");
            let publisher = receipt.summary.publisher.as_deref().unwrap_or("unknown");
            println!(
                "{} {} {} name={:?} publisher={:?} kind={:?} permissions={:?} commands={:?} sha256={} active={}",
                operation,
                receipt.plugin_id,
                receipt.version,
                receipt.summary.name,
                publisher,
                receipt.summary.kind,
                permissions,
                commands,
                receipt.archive_sha256,
                receipt.active_pointer.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("plugin install rejected: {error:?}");
            ExitCode::from(1)
        }
    }
}

fn permission_change_label(change: PermissionChange) -> &'static str {
    match change {
        PermissionChange::Added => "added",
        PermissionChange::Expanded => "expanded",
        PermissionChange::Narrowed => "narrowed",
        PermissionChange::Removed => "removed",
        PermissionChange::Unchanged => "unchanged",
    }
}

fn permission_scope_label(scope: Option<&PermissionScope>) -> String {
    match scope {
        None => "none".to_owned(),
        Some(PermissionScope::Clipboard { read, write }) => {
            format!("clipboard(read={read},write={write})")
        }
        Some(PermissionScope::Files {
            read,
            write,
            source,
        }) => format!("files(read={read},write={write},source={source:?})"),
        Some(PermissionScope::Http { origins }) => {
            format!(
                "http(origins={})",
                origins.iter().cloned().collect::<Vec<_>>().join("|")
            )
        }
        Some(PermissionScope::Logging) => "logging".to_owned(),
        Some(PermissionScope::Notification) => "notification".to_owned(),
        Some(PermissionScope::Storage { quota_bytes }) => {
            format!("storage(quota_bytes={quota_bytes})")
        }
    }
}

fn print_plugin_usage() {
    eprintln!(
        "usage: novahub plugin new [--id <id>] [--interaction view|one-shot]|dev|check|test|pack|sign|install|update|rollback|enable|disable|uninstall|retry-delete ..."
    );
}

fn plugin_new(args: &[String]) -> ExitCode {
    let Some(target) = positional_arg(args, 0) else {
        eprintln!("missing plugin directory");
        return ExitCode::from(2);
    };
    let interaction = match option_value(args, "--interaction")
        .as_deref()
        .unwrap_or("view")
    {
        "view" => PluginInteraction::View,
        "one-shot" => PluginInteraction::OneShot,
        value => {
            eprintln!("unsupported plugin interaction: {value}; expected view or one-shot");
            return ExitCode::from(2);
        }
    };
    let target = PathBuf::from(target);
    if let Err(error) = prepare_scaffold_directory(&target) {
        eprintln!("cannot create plugin scaffold: {error}");
        return ExitCode::from(1);
    }
    let id = option_value(args, "--id").unwrap_or_else(|| default_plugin_id(&target));
    let crate_name = id.replace(['.', '-'], "_");
    let interaction_name = match interaction {
        PluginInteraction::View => "view",
        PluginInteraction::OneShot => "one-shot",
    };
    let source = match interaction {
        PluginInteraction::View => view_plugin_template(),
        PluginInteraction::OneShot => one_shot_plugin_template(),
    };
    let files = [
        (
            "novahub.toml",
            format!(
                "id = \"{id}\"\nname = \"NovaHub Plugin\"\nversion = \"0.1.0\"\nhost_api = \">=1.1, <2.0\"\npermissions = []\n\n[[commands]]\nid = \"hello\"\ntitle = \"Hello\"\nsubtitle = \"Example NovaHub command\"\ninteraction = \"{interaction_name}\"\n"
            ),
        ),
        (
            "Cargo.toml",
            format!(
                "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\nwit-bindgen = \"0.57.1\"\n"
            ),
        ),
        ("src/lib.rs", source),
        ("wit/novahub-plugin/plugin.wit", PLUGIN_WIT.to_owned()),
    ];
    for (relative, contents) in files {
        let path = target.join(relative);
        if let Some(parent) = path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!("cannot create {}: {error}", parent.display());
            return ExitCode::from(1);
        }
        if let Err(error) = fs::write(&path, contents) {
            eprintln!("cannot write {}: {error}", path.display());
            return ExitCode::from(1);
        }
    }
    println!("created plugin scaffold: {} (id={id})", target.display());
    ExitCode::SUCCESS
}

fn view_plugin_template() -> String {
    r#"#![no_main]

wit_bindgen::generate!({
    world: "plugin",
    path: "wit/novahub-plugin",
});

struct ExamplePlugin;

impl Guest for ExamplePlugin {
    fn initialize() -> Result<(), String> {
        Ok(())
    }

    fn open_view() -> Result<novahub::plugin::types::View, String> {
        Ok(novahub::plugin::types::View::Empty(
            novahub::plugin::types::EmptyView {
                title: "Hello NovaHub".into(),
                description: "Replace this view with your plugin UI.".into(),
            },
        ))
    }

    fn update(_input: String) -> Result<novahub::plugin::types::View, String> {
        Self::open_view()
    }

    fn run(
        context: novahub::plugin::types::CommandContext,
    ) -> Result<novahub::plugin::types::CommandResult, String> {
        Ok(novahub::plugin::types::CommandResult {
            text: format!("{}: {}", context.command_id, context.input),
            copy_text: None,
        })
    }

    fn close_plugin() {}
}

export!(ExamplePlugin);
"#
    .to_owned()
}

fn one_shot_plugin_template() -> String {
    r#"#![no_main]

wit_bindgen::generate!({
    world: "plugin",
    path: "wit/novahub-plugin",
});

struct ExamplePlugin;

impl Guest for ExamplePlugin {
    fn initialize() -> Result<(), String> {
        Ok(())
    }

    fn open_view() -> Result<novahub::plugin::types::View, String> {
        Ok(novahub::plugin::types::View::Empty(
            novahub::plugin::types::EmptyView {
                title: "One-shot command".into(),
                description: "This command returns a bounded result and exits.".into(),
            },
        ))
    }

    fn update(_input: String) -> Result<novahub::plugin::types::View, String> {
        Self::open_view()
    }

    fn run(
        context: novahub::plugin::types::CommandContext,
    ) -> Result<novahub::plugin::types::CommandResult, String> {
        Ok(novahub::plugin::types::CommandResult {
            text: format!("{}: {}", context.command_id, context.input),
            copy_text: None,
        })
    }

    fn close_plugin() {}
}

export!(ExamplePlugin);
"#
    .to_owned()
}

fn plugin_dev(args: &[String]) -> ExitCode {
    let Some(source) = positional_arg(args, 0) else {
        eprintln!("missing plugin source directory");
        return ExitCode::from(2);
    };
    let output = option_value(args, "--output").map_or_else(
        || PathBuf::from(format!("{source}.novahub-plugin")),
        PathBuf::from,
    );
    match pack_directory(&source, &output, ArchiveLimits::default()) {
        Ok(()) => match validate_archive_file(&output, args) {
            Ok(summary) => {
                println!(
                    "dev package ready: {} {} commands={} output={}",
                    summary.name,
                    summary.version,
                    summary.commands.len(),
                    output.display()
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("plugin dev validation failed: {error}");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            eprintln!("plugin dev pack failed: {error:?}");
            ExitCode::from(1)
        }
    }
}

fn plugin_check(args: &[String], package_test: bool) -> ExitCode {
    let Some(input) = positional_arg(args, 0) else {
        eprintln!("missing plugin directory or archive");
        return ExitCode::from(2);
    };
    match validate_plugin_input(Path::new(&input), args) {
        Ok(summary) => {
            let action = if package_test {
                "package test passed"
            } else {
                "check passed"
            };
            println!(
                "{action}: name={:?} version={} kind={:?} permissions={} commands={}",
                summary.name,
                summary.version,
                summary.kind,
                summary.permissions.len(),
                summary.commands.len()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "plugin {} failed: {error}",
                if package_test { "test" } else { "check" }
            );
            ExitCode::from(1)
        }
    }
}

fn prepare_scaffold_directory(path: &Path) -> Result<(), String> {
    if path.exists() {
        if !path.is_dir() {
            return Err(format!("{} is not a directory", path.display()));
        }
        let mut entries = fs::read_dir(path).map_err(|error| error.to_string())?;
        if entries.next().is_some() {
            return Err(format!("{} is not empty", path.display()));
        }
    } else {
        fs::create_dir_all(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn default_plugin_id(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("plugin");
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!(
        "com.example.{}",
        if slug.is_empty() { "plugin" } else { &slug }
    )
}

fn validate_archive_file(path: &Path, args: &[String]) -> Result<PluginManifestSummary, String> {
    let archive = fs::read(path).map_err(|error| error.to_string())?;
    validate_archive_bytes(&archive, args)
}

fn validate_plugin_input(path: &Path, args: &[String]) -> Result<PluginManifestSummary, String> {
    if path.is_dir() {
        let temporary = TemporaryArchive::new("input");
        pack_directory(path, temporary.path(), ArchiveLimits::default())
            .map_err(|error| format!("failed to package source directory: {error:?}"))?;
        validate_archive_file(temporary.path(), args)
    } else {
        validate_archive_file(path, args)
    }
}

fn validate_archive_bytes(
    archive: &[u8],
    args: &[String],
) -> Result<PluginManifestSummary, String> {
    let host_version =
        option_value(args, "--host-version").unwrap_or_else(|| DEFAULT_HOST_VERSION.to_owned());
    let summary = inspect_archive(archive, &host_version, ArchiveLimits::default())
        .map_err(|error| format!("preview rejected: {error:?}"))?;
    let destination = temporary_directory("validation");
    let result = install_archive(
        archive,
        &destination,
        &host_version,
        None,
        ArchiveLimits::default(),
    )
    .map(|_| summary)
    .map_err(|error| format!("package validation rejected: {error:?}"));
    let _ = fs::remove_dir_all(destination);
    result
}

fn temporary_directory(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("novahub-cli-{label}-{}", unique_suffix()))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

struct TemporaryArchive {
    path: PathBuf,
}

impl TemporaryArchive {
    fn new(label: &str) -> Self {
        Self {
            path: std::env::temp_dir().join(format!(
                "novahub-cli-{label}-{}.novahub-plugin",
                unique_suffix()
            )),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryArchive {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn plugin_database_path(args: &[String], plugin_root: &Path) -> PathBuf {
    option_value(args, "--db").map_or_else(
        || {
            plugin_root
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("novahub.sqlite3")
        },
        PathBuf::from,
    )
}

fn persist_empty_plugin_grant(storage: &Storage, plugin_id: &str) -> Result<(), String> {
    let document = serde_json::to_string(&DeclaredPermissions::default())
        .map_err(|error| format!("failed to serialize empty grant: {error}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_secs();
    let timestamp = i64::try_from(timestamp)
        .map_err(|_| "system clock timestamp does not fit in SQLite integer".to_owned())?;
    storage
        .set_plugin_user_grant(plugin_id, &document, timestamp)
        .map_err(|error| error.to_string())
}

fn uninstall_plugin_with_authorization(
    plugin_root: &Path,
    storage: &Storage,
    plugin_id: &str,
) -> Result<UninstallOutcome, String> {
    if !is_safe_plugin_id(plugin_id) {
        return Err("invalid plugin id".to_owned());
    }
    persist_empty_plugin_grant(storage, plugin_id)?;
    let outcome = uninstall_plugin_with_pending_delete(plugin_root, plugin_id)
        .map_err(|error| format!("{error:?}"))?;
    if matches!(&outcome, UninstallOutcome::Removed) {
        storage.remove_plugin_state(plugin_id).map_err(|error| {
            format!("plugin was removed, but its authorization could not be cleaned: {error}")
        })?;
    }
    Ok(outcome)
}

fn retry_pending_deletes_with_authorization(
    plugin_root: &Path,
    storage: &Storage,
) -> Result<Vec<String>, String> {
    let removed_ids =
        retry_pending_delete_ids(plugin_root).map_err(|error| format!("{error:?}"))?;
    for plugin_id in &removed_ids {
        storage
            .remove_plugin_state(plugin_id)
            .map_err(|error| {
                format!(
                    "plugin {plugin_id} was removed, but its authorization could not be cleaned: {error}"
                )
            })?;
    }
    Ok(removed_ids)
}

fn migration_input_path(args: &[String]) -> Option<String> {
    let mut arguments = args.iter().skip(2);
    while let Some(argument) = arguments.next() {
        if matches!(argument.as_str(), "--source" | "--db") {
            let _ = arguments.next();
            continue;
        }
        if argument.starts_with('-') {
            continue;
        }
        return Some(argument.clone());
    }
    None
}

fn positional_arg(args: &[String], wanted_index: usize) -> Option<String> {
    let value_options = [
        "--root",
        "--output",
        "--seed",
        "--signature",
        "--public-key",
        "--host-version",
        "--sha256",
        "--id",
        "--db",
    ];
    let mut positionals = Vec::new();
    let mut skip_value = false;
    for argument in args.iter().skip(3) {
        if skip_value {
            skip_value = false;
            continue;
        }
        if value_options.contains(&argument.as_str()) {
            skip_value = true;
            continue;
        }
        if argument.starts_with('-') {
            continue;
        }
        positionals.push(argument.clone());
    }
    positionals.get(wanted_index).cloned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use novahub_storage::Storage;

    use super::{
        migration_input_path, persist_empty_plugin_grant, plugin_database_path, positional_arg,
        retry_pending_deletes_with_authorization, temporary_directory,
        uninstall_plugin_with_authorization,
    };

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn migration_path_skips_source_and_database_option_values() {
        let args = arguments(&[
            "novahub",
            "migrate",
            "--dry-run",
            "--source",
            "utools",
            "--db",
            "state.sqlite3",
            "export.json",
        ]);
        assert_eq!(migration_input_path(&args).as_deref(), Some("export.json"));
    }

    #[test]
    fn migration_path_accepts_a_path_before_optional_database_flag() {
        let args = arguments(&[
            "novahub",
            "migrate",
            "--apply",
            "export.json",
            "--db",
            "state.sqlite3",
        ]);
        assert_eq!(migration_input_path(&args).as_deref(), Some("export.json"));
    }

    #[test]
    fn plugin_positionals_ignore_value_options_in_any_order() {
        let args = arguments(&[
            "novahub",
            "plugin",
            "install",
            "--root",
            "plugins",
            "--signature",
            "plugin.sig",
            "package.novahub-plugin",
            "--public-key",
            "publisher.pub",
        ]);
        assert_eq!(
            positional_arg(&args, 0).as_deref(),
            Some("package.novahub-plugin")
        );
    }

    #[test]
    fn plugin_database_defaults_to_the_plugin_root_parent() {
        let args = arguments(&["novahub", "plugin", "uninstall", "demo"]);
        assert_eq!(
            plugin_database_path(&args, Path::new("state/plugins")),
            Path::new("state/novahub.sqlite3")
        );
    }

    #[test]
    fn plugin_database_flag_overrides_the_default() {
        let args = arguments(&[
            "novahub",
            "plugin",
            "retry-delete",
            "--root",
            "state/plugins",
            "--db",
            "custom.sqlite3",
        ]);
        assert_eq!(
            plugin_database_path(&args, Path::new("state/plugins")),
            Path::new("custom.sqlite3")
        );
    }

    #[test]
    fn uninstall_freezes_authorization_as_an_explicit_empty_grant() {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        persist_empty_plugin_grant(&storage, "demo").expect("persist empty grant");
        let document = storage
            .plugin_user_grant("demo")
            .expect("read grant")
            .expect("grant row");
        assert_eq!(document, r#"{"entries":{},"display_names":[]}"#);
    }

    #[test]
    fn terminal_uninstall_removes_grant_and_same_id_starts_without_old_approval() {
        let root = temporary_directory("uninstall-auth");
        std::fs::create_dir_all(root.join("demo")).expect("plugin directory");
        let storage = Storage::open_in_memory().expect("in-memory storage");
        storage
            .set_plugin_user_grant("demo", r#"{"legacy":true}"#, 1)
            .expect("legacy grant");
        storage
            .set_plugin_kv("demo", "cached", b"value", 64, 1)
            .expect("plugin KV state");

        let outcome = uninstall_plugin_with_authorization(&root, &storage, "demo")
            .expect("uninstall with authorization");
        assert!(matches!(
            outcome,
            novahub_plugin_manager::UninstallOutcome::Removed
        ));
        assert!(!root.join("demo").exists());
        assert!(
            storage
                .plugin_user_grant("demo")
                .expect("read removed grant")
                .is_none()
        );
        assert!(
            storage
                .plugin_kv("demo", "cached")
                .expect("read removed plugin KV")
                .is_none()
        );

        std::fs::create_dir_all(root.join("demo")).expect("reinstall directory");
        assert!(
            storage
                .plugin_user_grant("demo")
                .expect("read grant before reinstall")
                .is_none()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retry_delete_removes_only_the_completed_plugin_grant() {
        let root = temporary_directory("retry-auth");
        let marker_root = root.join(".pending-delete");
        let plugin_path = root.join("demo");
        std::fs::create_dir_all(&marker_root).expect("marker directory");
        std::fs::create_dir_all(&plugin_path).expect("plugin directory");
        std::fs::write(
            marker_root.join("demo.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "demo",
                "path": plugin_path,
            }))
            .expect("marker JSON"),
        )
        .expect("marker file");
        let storage = Storage::open_in_memory().expect("in-memory storage");
        storage
            .set_plugin_user_grant("demo", r#"{"entries":{}}"#, 1)
            .expect("empty pending grant");
        storage
            .set_plugin_kv("demo", "pending", b"value", 64, 1)
            .expect("pending plugin KV state");

        assert_eq!(
            retry_pending_deletes_with_authorization(&root, &storage).expect("retry"),
            vec!["demo"]
        );
        assert!(
            storage
                .plugin_user_grant("demo")
                .expect("read cleaned grant")
                .is_none()
        );
        assert!(
            storage
                .plugin_kv("demo", "pending")
                .expect("read cleaned plugin KV")
                .is_none()
        );
        assert!(!plugin_path.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
