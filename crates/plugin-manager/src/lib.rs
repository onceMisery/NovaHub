#![forbid(unsafe_code)]

mod permissions;

pub use permissions::{
    AuthorizedCapability, Capability, CapabilityBroker, CapabilityDenied, CapabilityRequest,
    DeclaredPermission, DeclaredPermissions, EffectiveGrant, FileSource, HostFileHandle,
    PermissionChange, PermissionDiff, PermissionScope, PluginIdentity, diff_permissions,
};

use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::permissions::parse_permissions;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub version: semver::Version,
    pub host_api: semver::VersionReq,
    kind: PluginKind,
    declared_permissions: DeclaredPermissions,
    commands: Vec<PluginCommand>,
}

/// A bounded command summary exposed to installation and diagnostic surfaces.
/// The executable behavior remains owned by the plugin Component; this type
/// only carries metadata needed by the host's command index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub interaction: PluginInteraction,
}

/// Describes whether a command owns a host-rendered view or returns one
/// bounded result and exits. Background commands are intentionally absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginInteraction {
    View,
    OneShot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginKind {
    Command,
    DesktopPet,
}

#[derive(Deserialize)]
struct RawManifest {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    #[serde(alias = "author")]
    publisher: Option<String>,
    version: String,
    #[serde(alias = "plugin_api")]
    host_api: String,
    #[serde(default = "empty_permissions_value")]
    permissions: toml::Value,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    commands: Vec<RawCommand>,
}

fn empty_permissions_value() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

#[derive(Default, Deserialize)]
struct RawCommand {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    subtitle: String,
    #[serde(default)]
    interaction: Option<String>,
}

impl TryFrom<RawCommand> for PluginCommand {
    type Error = String;

    fn try_from(command: RawCommand) -> Result<Self, Self::Error> {
        let interaction = match command.interaction.as_deref().unwrap_or("view") {
            "view" => PluginInteraction::View,
            "one-shot" => PluginInteraction::OneShot,
            other => return Err(format!("unsupported plugin command interaction: {other}")),
        };
        Ok(Self {
            id: command.id,
            title: command.title,
            subtitle: command.subtitle,
            interaction,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct PetDescriptor {
    #[serde(rename = "petId")]
    pub pet_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "fallbackPetId")]
    pub fallback_pet_id: String,
    pub frames: Vec<String>,
    #[serde(rename = "recommendedActions", default)]
    pub recommended_actions: Vec<String>,
}

const MAX_PET_FRAME_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PET_TOTAL_FRAME_BYTES: u64 = 8 * 1024 * 1024;

/// A validated frame loaded from an installed desktop-pet package.
///
/// The bytes are copied only after the active pointer, path boundary and size
/// limits have been checked. The UI crate receives the bytes as an opaque
/// host-owned image and never receives the package path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PetFrameAsset {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Validated active desktop-pet metadata and bounded frame resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledPet {
    pub plugin_id: String,
    pub version: semver::Version,
    pub descriptor: PetDescriptor,
    pub frames: Vec<PetFrameAsset>,
}

/// Parses and bounds a declarative desktop-pet descriptor.
///
/// # Errors
///
/// Returns an error for malformed JSON, missing identity, or resource/action
/// limits that could make a pet residently expensive.
pub fn parse_pet_descriptor(input: &str) -> Result<PetDescriptor, String> {
    let descriptor: PetDescriptor =
        serde_json::from_str(input).map_err(|error| error.to_string())?;
    if descriptor.pet_id.trim().is_empty()
        || descriptor.display_name.trim().is_empty()
        || descriptor.frames.is_empty()
        || descriptor.frames.len() > 240
        || descriptor.recommended_actions.len() > 3
        || descriptor.fallback_pet_id.trim().is_empty()
        || descriptor.pet_id.len() > 128
        || descriptor.display_name.len() > 256
        || descriptor
            .frames
            .iter()
            .any(|frame| !is_safe_descriptor_value(frame, 256))
        || descriptor
            .recommended_actions
            .iter()
            .any(|action| !is_safe_descriptor_value(action, 128))
    {
        return Err("desktop-pet descriptor exceeds identity or resource limits".into());
    }
    Ok(descriptor)
}

fn is_safe_descriptor_value(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(char::is_control)
        && !value.starts_with('/')
        && !value.contains(':')
        && !value.split(['/', '\\']).any(|part| part == "..")
}

/// Parses the host-owned `novahub.toml` manifest without executing plugin code.
///
/// # Errors
///
/// Returns a diagnostic for malformed TOML or invalid `SemVer` fields.
pub fn parse_manifest_toml(input: &str) -> Result<PluginManifest, String> {
    let raw: RawManifest = toml::from_str(input).map_err(|error| error.to_string())?;
    semver::Version::parse(&raw.version).map_err(|error| error.to_string())?;
    semver::VersionReq::parse(&raw.host_api).map_err(|error| error.to_string())?;
    let permissions = parse_permissions(&raw.permissions)?;
    let mut manifest = PluginManifest::new(raw.id, raw.version, raw.host_api)
        .with_name(raw.name)
        .with_publisher(raw.publisher)
        .with_declared_permissions(permissions)
        .with_commands(
            raw.commands
                .into_iter()
                .map(PluginCommand::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        );
    if let Some(kind) = raw.kind {
        manifest.kind = match kind.as_str() {
            "command" => PluginKind::Command,
            "desktop-pet" => PluginKind::DesktopPet,
            _ => return Err("unsupported plugin kind".into()),
        };
    }
    manifest.validate_metadata()?;
    Ok(manifest)
}

/// Rejects archive paths that could escape the staging directory.
///
/// # Errors
///
/// Returns an error for absolute paths, parent traversal, oversized names, or
/// more than 2,048 entries.
pub fn validate_archive_paths<I, S>(paths: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut count = 0_usize;
    for path in paths {
        count += 1;
        let value = path.as_ref().replace('\\', "/");
        let value = value.trim_end_matches('/');
        if count > 2_048
            || value.is_empty()
            || value.starts_with('/')
            || value.contains(':')
            || value.split('/').any(|part| part == ".." || part.is_empty())
            || value.len() > 256
        {
            return Err("archive path is unsafe or exceeds limits".into());
        }
    }
    Ok(())
}

const MAX_ARCHIVE_ENTRIES: usize = 2_048;
const MAX_COMPRESSED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_UNCOMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLimits {
    pub max_entries: usize,
    pub max_compressed_bytes: u64,
    pub max_uncompressed_bytes: u64,
    pub max_entry_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: MAX_ARCHIVE_ENTRIES,
            max_compressed_bytes: MAX_COMPRESSED_BYTES,
            max_uncompressed_bytes: MAX_UNCOMPRESSED_BYTES,
            max_entry_bytes: MAX_ENTRY_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallReceipt {
    pub plugin_id: String,
    pub version: semver::Version,
    pub archive_sha256: String,
    pub active_pointer: PathBuf,
    pub summary: PluginManifestSummary,
}

/// Metadata retained in an install receipt for CLI output and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginManifestSummary {
    pub id: String,
    pub version: semver::Version,
    pub name: String,
    pub publisher: Option<String>,
    pub kind: PluginKind,
    pub permissions: Vec<String>,
    pub declared_permissions: DeclaredPermissions,
    pub commands: Vec<PluginCommand>,
}

/// Computes the user-visible permission change for an install or update.
#[must_use]
pub fn diff_declared_permissions(
    before: Option<&DeclaredPermissions>,
    after: &DeclaredPermissions,
) -> Vec<PermissionDiff> {
    let empty = DeclaredPermissions::default();
    diff_permissions(before.unwrap_or(&empty), after)
}

/// Reads and validates only the manifest from an archive before installation.
/// No plugin code is loaded and no files are extracted, so callers can show a
/// trustworthy install preview without changing the active plugin set.
///
/// # Errors
///
/// Returns an archive, compatibility, or manifest validation error.
pub fn inspect_archive(
    archive: &[u8],
    host_version: &str,
    limits: ArchiveLimits,
) -> Result<PluginManifestSummary, ArchiveInstallError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    validate_archive_metadata(&mut zip, limits)?;
    let mut manifest_entry = zip
        .by_name("novahub.toml")
        .map_err(|_| ArchiveInstallError::MissingEntry("novahub.toml"))?;
    let mut manifest_bytes =
        Vec::with_capacity(usize::try_from(manifest_entry.size()).unwrap_or(0));
    manifest_entry
        .read_to_end(&mut manifest_bytes)
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    let manifest_text = std::str::from_utf8(&manifest_bytes)
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    let manifest =
        parse_manifest_toml(manifest_text).map_err(ArchiveInstallError::InvalidArchive)?;
    if !manifest.supports(host_version) {
        return Err(ArchiveInstallError::IncompatibleHost);
    }
    Ok(manifest.summary())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivePlugin {
    pub plugin_id: String,
    pub version: semver::Version,
    pub path: PathBuf,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UninstallOutcome {
    Removed,
    PendingDelete { marker: PathBuf },
}

/// Switches the active pointer to the newest installed version other than the
/// current one. Version directories are immutable after installation, so a
/// rollback only changes the small pointer file and never copies plugin data.
///
/// # Errors
///
/// Returns an archive or filesystem error when the current pointer is invalid,
/// no previous version is installed, or the pointer cannot be replaced.
pub fn rollback_plugin(
    root: impl AsRef<Path>,
    plugin_id: &str,
) -> Result<ActivePlugin, ArchiveInstallError> {
    if !is_safe_plugin_id(plugin_id) {
        return Err(ArchiveInstallError::UnsafePluginId);
    }
    let root = root.as_ref();
    let current = read_active_plugin(root, plugin_id)?;
    let previous_pointer = root.join(plugin_id).join("previous.json");
    let historical = previous_pointer
        .exists()
        .then(|| read_pointer(root, plugin_id, &previous_pointer));
    let historical = match historical {
        Some(Ok(previous)) if previous.version != current.version => Some(previous),
        Some(Err(error)) => return Err(error),
        _ => None,
    };
    if let Some(previous) = historical {
        let pointer = root.join(plugin_id).join("active.json");
        let next = pointer.with_file_name("active.json.next");
        let value = serde_json::json!({
            "plugin_id": plugin_id,
            "version": previous.version.to_string(),
            "path": previous.path,
            "enabled": current.enabled,
        });
        fs::write(&next, serde_json::to_vec_pretty(&value).unwrap_or_default())?;
        activate_pointer(&next, &pointer)?;
        return read_active_plugin(root, plugin_id);
    }
    let versions_root = root.join(plugin_id).join("versions");
    let mut candidates = fs::read_dir(&versions_root)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let version =
                semver::Version::parse(entry.file_name().to_string_lossy().as_ref()).ok()?;
            if version == current.version {
                return None;
            }
            Some((version, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    let (version, path) = candidates
        .pop()
        .ok_or_else(|| ArchiveInstallError::InvalidArchive("no previous plugin version".into()))?;
    let pointer = root.join(plugin_id).join("active.json");
    let next = pointer.with_file_name("active.json.next");
    let value = serde_json::json!({
        "plugin_id": plugin_id,
        "version": version.to_string(),
        "path": path,
        "enabled": current.enabled,
    });
    fs::write(&next, serde_json::to_vec_pretty(&value).unwrap_or_default())?;
    activate_pointer(&next, &pointer)?;
    read_active_plugin(root, plugin_id)
}

#[derive(Debug, Eq, PartialEq)]
pub enum ArchiveInstallError {
    InvalidArchive(String),
    Io(String),
    IncompatibleHost,
    MissingEntry(&'static str),
    HashMismatch,
    UnsafePluginId,
    InvalidSignature,
}

fn validate_component(bytes: &[u8]) -> Result<(), ArchiveInstallError> {
    if !wasmparser::Parser::is_component(bytes) {
        return Err(ArchiveInstallError::InvalidArchive(
            "plugin.wasm is not a WebAssembly Component".into(),
        ));
    }
    wasmparser::Validator::new()
        .validate_all(bytes)
        .map(|_| ())
        .map_err(|error| {
            ArchiveInstallError::InvalidArchive(format!(
                "plugin.wasm is not a valid WebAssembly Component: {error}"
            ))
        })
}

impl From<std::io::Error> for ArchiveInstallError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

/// Installs a `.novahub-plugin` ZIP into a staging directory and atomically
/// updates the host-owned active pointer after all validation succeeds.
///
/// The archive is fully bounded before extraction, and no plugin code is
/// executed during installation.
///
/// # Errors
///
/// Returns an archive, compatibility, hash, or filesystem error.
pub fn install_archive(
    archive: &[u8],
    destination: impl AsRef<Path>,
    host_version: &str,
    expected_sha256: Option<&str>,
    limits: ArchiveLimits,
) -> Result<InstallReceipt, ArchiveInstallError> {
    install_archive_verified(
        archive,
        destination,
        host_version,
        expected_sha256,
        None,
        limits,
    )
}

/// Verifies an optional Ed25519 signature over the exact archive bytes before
/// extraction. Signature verification is skipped only when `signature` is
/// explicitly `None` (developer-mode caller responsibility).
///
/// # Errors
///
/// Returns an archive, compatibility, hash, signature, or filesystem error.
#[allow(clippy::too_many_lines)]
pub fn install_archive_verified(
    archive: &[u8],
    destination: impl AsRef<Path>,
    host_version: &str,
    expected_sha256: Option<&str>,
    signature: Option<(&[u8], &[u8])>,
    limits: ArchiveLimits,
) -> Result<InstallReceipt, ArchiveInstallError> {
    let actual_hash = Sha256::digest(archive);
    let actual_hash = format_digest(&actual_hash);
    if expected_sha256.is_some_and(|expected| !expected.eq_ignore_ascii_case(&actual_hash)) {
        return Err(ArchiveInstallError::HashMismatch);
    }
    if let Some((public_key, signature)) = signature {
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
            .verify(archive, signature)
            .map_err(|_| ArchiveInstallError::InvalidSignature)?;
    }

    let mut zip = zip::ZipArchive::new(Cursor::new(archive))
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    validate_archive_metadata(&mut zip, limits)?;

    let mut entries = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
        let name = entry.name().replace('\\', "/");
        let normalized = name.trim_end_matches('/').to_owned();
        if entry.is_dir() {
            continue;
        }
        let mut data = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
        entry
            .read_to_end(&mut data)
            .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
        entries.push((normalized, data));
    }

    let manifest = entries
        .iter()
        .find(|(name, _)| name == "novahub.toml")
        .ok_or(ArchiveInstallError::MissingEntry("novahub.toml"))?;
    let manifest_text = std::str::from_utf8(&manifest.1)
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    let manifest =
        parse_manifest_toml(manifest_text).map_err(ArchiveInstallError::InvalidArchive)?;
    if !manifest.supports(host_version) {
        return Err(ArchiveInstallError::IncompatibleHost);
    }
    let required_entry = match manifest.kind() {
        PluginKind::Command => {
            let component = manifest_entry(&entries, "plugin.wasm");
            if let Some(bytes) = component {
                validate_component(bytes)?;
            }
            component
        }
        PluginKind::DesktopPet => {
            let descriptor = manifest_entry(&entries, "pet.json")
                .ok_or(ArchiveInstallError::MissingEntry("pet.json"))?;
            let descriptor_text = std::str::from_utf8(descriptor)
                .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
            let descriptor = parse_pet_descriptor(descriptor_text)
                .map_err(ArchiveInstallError::InvalidArchive)?;
            for frame in &descriptor.frames {
                if manifest_entry(&entries, frame).is_none() {
                    return Err(ArchiveInstallError::InvalidArchive(format!(
                        "desktop-pet frame asset is missing: {frame}"
                    )));
                }
            }
            Some(descriptor_text.as_bytes())
        }
    };
    if required_entry.is_none() {
        return Err(ArchiveInstallError::MissingEntry(match manifest.kind() {
            PluginKind::Command => "plugin.wasm",
            PluginKind::DesktopPet => "pet.json",
        }));
    }
    let manifest_summary = manifest.summary();
    if !is_safe_plugin_id(&manifest.id) {
        return Err(ArchiveInstallError::UnsafePluginId);
    }

    let destination = destination.as_ref();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let staging = destination
        .join(".staging")
        .join(format!("{}-{}", manifest.id, suffix));
    let version_dir = destination
        .join(&manifest.id)
        .join("versions")
        .join(manifest.version.to_string());
    let active_pointer = destination.join(&manifest.id).join("active.json");

    let result = (|| -> Result<InstallReceipt, ArchiveInstallError> {
        fs::create_dir_all(&staging)?;
        for (name, data) in &entries {
            let output = staging.join(name);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::File::create(output)?;
            file.write_all(data)?;
        }
        if version_dir.exists() {
            fs::remove_dir_all(&version_dir)?;
        }
        if let Some(parent) = version_dir.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&staging, &version_dir)?;
        let pointer_parent = active_pointer
            .parent()
            .ok_or_else(|| ArchiveInstallError::Io("active pointer has no parent".into()))?;
        fs::create_dir_all(pointer_parent)?;
        let next_pointer = pointer_parent.join("active.json.next");
        let pointer = serde_json::json!({
            "plugin_id": manifest.id,
            "version": manifest.version.to_string(),
            "path": version_dir.to_string_lossy(),
            "enabled": true,
        });
        fs::write(
            &next_pointer,
            serde_json::to_vec_pretty(&pointer).unwrap_or_default(),
        )?;
        activate_pointer(&next_pointer, &active_pointer)?;
        Ok(InstallReceipt {
            plugin_id: manifest.id,
            version: manifest.version,
            archive_sha256: actual_hash.clone(),
            active_pointer,
            summary: manifest_summary,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

/// Verifies an Ed25519 signature for host-side package or update metadata.
///
/// # Errors
///
/// Returns `InvalidSignature` when the public key or signature is not valid.
pub fn verify_ed25519(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), ArchiveInstallError> {
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
        .verify(message, signature)
        .map_err(|_| ArchiveInstallError::InvalidSignature)
}

/// Packs a plugin directory into a deterministic, bounded ZIP archive.
///
/// # Errors
///
/// Returns an archive or filesystem error when the source contains unsafe
/// entries or exceeds the configured limits.
pub fn pack_directory(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
    limits: ArchiveLimits,
) -> Result<(), ArchiveInstallError> {
    let source = source.as_ref();
    let mut files = Vec::new();
    collect_files(source, source, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    validate_archive_paths(files.iter().map(|(path, _)| path.as_str()))
        .map_err(ArchiveInstallError::InvalidArchive)?;
    if files.len() > limits.max_entries {
        return Err(ArchiveInstallError::InvalidArchive(
            "directory has too many files".into(),
        ));
    }
    let total = files.iter().try_fold(0_u64, |total, (_, data)| {
        let size = u64::try_from(data.len()).unwrap_or(u64::MAX);
        let next = total.saturating_add(size);
        if size > limits.max_entry_bytes || next > limits.max_uncompressed_bytes {
            Err(ArchiveInstallError::InvalidArchive(
                "directory exceeds package size limits".into(),
            ))
        } else {
            Ok(next)
        }
    })?;
    let _ = total;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(output)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (path, data) in files {
        writer
            .start_file(path, options)
            .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
        writer.write_all(&data)?;
    }
    writer
        .finish()
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    Ok(())
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), ArchiveInstallError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_symlink() {
            return Err(ArchiveInstallError::InvalidArchive(
                "plugin package cannot contain symlinks".into(),
            ));
        }
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(ArchiveInstallError::InvalidArchive(
                "plugin package contains unsupported filesystem entry".into(),
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        files.push((relative, fs::read(path)?));
    }
    Ok(())
}

/// Signs the exact archive bytes with an Ed25519 seed and returns the raw
/// 64-byte signature plus the 32-byte public key.
///
/// # Errors
///
/// Returns `InvalidSignature` when the seed cannot initialize an Ed25519 key.
pub fn sign_archive(
    archive: &[u8],
    seed: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), ArchiveInstallError> {
    use ring::signature::KeyPair;

    let key_pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(seed)
        .map_err(|_| ArchiveInstallError::InvalidSignature)?;
    Ok((
        key_pair.sign(archive).as_ref().to_vec(),
        key_pair.public_key().as_ref().to_vec(),
    ))
}

/// Reads the host-owned active pointer without loading plugin code.
///
/// # Errors
///
/// Returns an archive or filesystem error when the pointer is missing or
/// points outside an installed version.
pub fn read_active_plugin(
    root: impl AsRef<Path>,
    plugin_id: &str,
) -> Result<ActivePlugin, ArchiveInstallError> {
    if !is_safe_plugin_id(plugin_id) {
        return Err(ArchiveInstallError::UnsafePluginId);
    }
    let pointer_path = root.as_ref().join(plugin_id).join("active.json");
    read_pointer(root, plugin_id, &pointer_path)
}

/// Reads the manifest referenced by the active pointer without loading plugin
/// code. A missing pointer means this is a first install.
///
/// # Errors
///
/// Returns an archive, pointer, filesystem, or manifest validation error when
/// the active pointer or its manifest cannot be read safely.
pub fn read_active_manifest(
    root: impl AsRef<Path>,
    plugin_id: &str,
) -> Result<Option<PluginManifest>, ArchiveInstallError> {
    if !is_safe_plugin_id(plugin_id) {
        return Err(ArchiveInstallError::UnsafePluginId);
    }
    let pointer = root.as_ref().join(plugin_id).join("active.json");
    if !pointer.exists() {
        return Ok(None);
    }
    let active = read_active_plugin(root, plugin_id)?;
    let manifest_text = fs::read_to_string(active.path.join("novahub.toml"))?;
    parse_manifest_toml(&manifest_text)
        .map(Some)
        .map_err(ArchiveInstallError::InvalidArchive)
}

/// Loads a validated active desktop-pet package without executing plugin code.
///
/// The package must be the active enabled version, contain a valid
/// `desktop-pet` manifest and expose only bounded SVG frames under its version
/// directory. This is the sole path by which custom pet resources enter the
/// host renderer.
///
/// # Errors
///
/// Returns an archive, path, size, or filesystem error when the active package
/// is missing, disabled, malformed, or outside the validated resource budget.
pub fn read_active_pet(
    root: impl AsRef<Path>,
    plugin_id: &str,
) -> Result<InstalledPet, ArchiveInstallError> {
    let root = root.as_ref();
    let active = read_active_plugin(root, plugin_id)?;
    if !active.enabled {
        return Err(ArchiveInstallError::InvalidArchive(
            "desktop-pet is disabled".into(),
        ));
    }
    let manifest_text = fs::read_to_string(active.path.join("novahub.toml"))?;
    let manifest =
        parse_manifest_toml(&manifest_text).map_err(ArchiveInstallError::InvalidArchive)?;
    if manifest.kind() != PluginKind::DesktopPet {
        return Err(ArchiveInstallError::InvalidArchive(
            "active plugin is not a desktop-pet".into(),
        ));
    }
    let descriptor_text = fs::read_to_string(active.path.join("pet.json"))?;
    let descriptor =
        parse_pet_descriptor(&descriptor_text).map_err(ArchiveInstallError::InvalidArchive)?;
    let version_root = active
        .path
        .canonicalize()
        .map_err(ArchiveInstallError::from)?;
    let mut total_bytes = 0_u64;
    let mut frames = Vec::with_capacity(descriptor.frames.len());
    for name in &descriptor.frames {
        let path = active.path.join(name);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() {
            return Err(ArchiveInstallError::InvalidArchive(format!(
                "desktop-pet frame is not a regular file: {name}"
            )));
        }
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
        {
            return Err(ArchiveInstallError::InvalidArchive(
                "desktop-pet frames must use SVG resources".into(),
            ));
        }
        let canonical = path.canonicalize().map_err(ArchiveInstallError::from)?;
        if !canonical.starts_with(&version_root) {
            return Err(ArchiveInstallError::InvalidArchive(
                "desktop-pet frame escapes its installed version".into(),
            ));
        }
        let size = metadata.len();
        total_bytes = total_bytes.saturating_add(size);
        if size > MAX_PET_FRAME_BYTES || total_bytes > MAX_PET_TOTAL_FRAME_BYTES {
            return Err(ArchiveInstallError::InvalidArchive(
                "desktop-pet frames exceed resource limits".into(),
            ));
        }
        frames.push(PetFrameAsset {
            name: name.clone(),
            bytes: fs::read(canonical)?,
        });
    }
    Ok(InstalledPet {
        plugin_id: active.plugin_id,
        version: active.version,
        descriptor,
        frames,
    })
}

/// Validates ZIP metadata without reading or extracting entry bodies.
///
/// Installation calls this check before reading entries, while preview uses
/// it to guarantee that a package cannot pass the read-only preview and only
/// fail later because of a path, duplicate, compression, or size limit.
fn validate_archive_metadata<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    limits: ArchiveLimits,
) -> Result<(), ArchiveInstallError> {
    if zip.len() > limits.max_entries {
        return Err(ArchiveInstallError::InvalidArchive(
            "archive has too many entries".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut compressed_total = 0_u64;
    let mut uncompressed_total = 0_u64;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
        let name = entry.name().replace('\\', "/");
        validate_archive_paths([name.as_str()]).map_err(ArchiveInstallError::InvalidArchive)?;
        let normalized = name.trim_end_matches('/').to_owned();
        if !seen.insert(normalized) {
            return Err(ArchiveInstallError::InvalidArchive(
                "archive contains duplicate paths".into(),
            ));
        }
        compressed_total = compressed_total.saturating_add(entry.compressed_size());
        uncompressed_total = uncompressed_total.saturating_add(entry.size());
        if compressed_total > limits.max_compressed_bytes
            || uncompressed_total > limits.max_uncompressed_bytes
            || entry.size() > limits.max_entry_bytes
        {
            return Err(ArchiveInstallError::InvalidArchive(
                "archive exceeds compressed or extracted size limits".into(),
            ));
        }
    }
    Ok(())
}

fn read_pointer(
    root: impl AsRef<Path>,
    plugin_id: &str,
    pointer_path: &Path,
) -> Result<ActivePlugin, ArchiveInstallError> {
    let input = fs::read_to_string(pointer_path)?;
    let value: serde_json::Value = serde_json::from_str(&input)
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    let id = value
        .get("plugin_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ArchiveInstallError::InvalidArchive("active pointer has no plugin_id".into())
        })?;
    let version_text = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ArchiveInstallError::InvalidArchive("active pointer has no version".into())
        })?;
    let path = value
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| ArchiveInstallError::InvalidArchive("active pointer has no path".into()))?;
    let versions_root = root
        .as_ref()
        .join(plugin_id)
        .join("versions")
        .canonicalize()
        .map_err(ArchiveInstallError::from)?;
    let canonical_path = path.canonicalize().map_err(ArchiveInstallError::from)?;
    if id != plugin_id || !canonical_path.starts_with(&versions_root) || !canonical_path.is_dir() {
        return Err(ArchiveInstallError::InvalidArchive(
            "active pointer target is invalid".into(),
        ));
    }
    let version = semver::Version::parse(version_text)
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    let enabled = value
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    Ok(ActivePlugin {
        plugin_id: id.to_owned(),
        version,
        path: canonical_path,
        enabled,
    })
}

/// Enables or disables an installed plugin by atomically updating its pointer.
/// Existing pointers without `enabled` remain enabled for compatibility.
///
/// # Errors
///
/// Returns an archive or filesystem error when the active pointer is invalid
/// or cannot be replaced atomically.
pub fn set_plugin_enabled(
    root: impl AsRef<Path>,
    plugin_id: &str,
    enabled: bool,
) -> Result<ActivePlugin, ArchiveInstallError> {
    if !is_safe_plugin_id(plugin_id) {
        return Err(ArchiveInstallError::UnsafePluginId);
    }
    let root = root.as_ref();
    let pointer = root.join(plugin_id).join("active.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&pointer)?)
        .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
    let active = read_active_plugin(root, plugin_id)?;
    value["enabled"] = serde_json::Value::Bool(enabled);
    let next = pointer.with_file_name("active.json.next");
    fs::write(&next, serde_json::to_vec_pretty(&value).unwrap_or_default())?;
    activate_pointer(&next, &pointer)?;
    Ok(ActivePlugin { enabled, ..active })
}

/// Removes all installed versions and the active pointer for a plugin.
///
/// # Errors
///
/// Returns an unsafe-ID or filesystem error.
pub fn uninstall_plugin(
    root: impl AsRef<Path>,
    plugin_id: &str,
) -> Result<(), ArchiveInstallError> {
    let _ = uninstall_plugin_with_pending_delete(root, plugin_id)?;
    Ok(())
}

/// Removes a plugin or records a retry marker when the OS still holds a file.
///
/// # Errors
///
/// Returns an unsafe-ID or filesystem error when the marker cannot be written.
pub fn uninstall_plugin_with_pending_delete(
    root: impl AsRef<Path>,
    plugin_id: &str,
) -> Result<UninstallOutcome, ArchiveInstallError> {
    if !is_safe_plugin_id(plugin_id) {
        return Err(ArchiveInstallError::UnsafePluginId);
    }
    let root = root.as_ref();
    let path = root.join(plugin_id);
    if path.exists() {
        match fs::remove_dir_all(&path) {
            Ok(()) => return Ok(UninstallOutcome::Removed),
            Err(error) => {
                let marker_root = root.join(".pending-delete");
                fs::create_dir_all(&marker_root)?;
                let marker = marker_root.join(format!("{plugin_id}.json"));
                let payload = serde_json::json!({
                    "plugin_id": plugin_id,
                    "path": path,
                });
                fs::write(&marker, serde_json::to_vec(&payload).unwrap_or_default())?;
                let _ = error;
                return Ok(UninstallOutcome::PendingDelete { marker });
            }
        }
    }
    Ok(UninstallOutcome::Removed)
}

/// Retries pending plugin deletions and removes successful markers.
///
/// # Errors
///
/// Returns a filesystem or malformed-marker error.
pub fn retry_pending_deletes(root: impl AsRef<Path>) -> Result<usize, ArchiveInstallError> {
    Ok(retry_pending_delete_ids(root)?.len())
}

/// Retries pending plugin deletions and returns the IDs whose markers were
/// removed successfully. The IDs let the host clean up related authorization
/// rows only after the filesystem deletion is complete.
///
/// # Errors
///
/// Returns a filesystem or malformed-marker error.
pub fn retry_pending_delete_ids(
    root: impl AsRef<Path>,
) -> Result<Vec<String>, ArchiveInstallError> {
    let marker_root = root.as_ref().join(".pending-delete");
    if !marker_root.exists() {
        return Ok(Vec::new());
    }
    let mut removed = Vec::new();
    for entry in fs::read_dir(&marker_root)? {
        let entry = entry?;
        let marker = entry.path();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&marker)?)
            .map_err(|error| ArchiveInstallError::InvalidArchive(error.to_string()))?;
        let plugin_id = value
            .get("plugin_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ArchiveInstallError::InvalidArchive("pending marker has no plugin_id".into())
            })?;
        let path = value
            .get("path")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| {
                ArchiveInstallError::InvalidArchive("pending marker has no path".into())
            })?;
        if !is_safe_plugin_id(plugin_id) || !path.starts_with(root.as_ref()) {
            return Err(ArchiveInstallError::InvalidArchive(
                "pending marker target is invalid".into(),
            ));
        }
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::remove_file(marker)?;
        removed.push(plugin_id.to_owned());
    }
    if fs::read_dir(&marker_root)?.next().is_none() {
        fs::remove_dir(&marker_root)?;
    }
    Ok(removed)
}

fn activate_pointer(next: &Path, active: &Path) -> Result<(), ArchiveInstallError> {
    let backup = active.with_extension("json.rollback");
    let previous = active.with_file_name("previous.json");
    if backup.exists() {
        fs::remove_file(&backup)?;
    }
    if active.exists() {
        fs::rename(active, &backup)?;
    }
    match fs::rename(next, active) {
        Ok(()) => {
            if backup.exists() {
                if previous.exists() {
                    fs::remove_file(&previous)?;
                }
                if let Err(error) = fs::rename(&backup, &previous) {
                    let _ = fs::remove_file(active);
                    let _ = fs::rename(&backup, active);
                    return Err(ArchiveInstallError::Io(error.to_string()));
                }
            }
            Ok(())
        }
        Err(error) => {
            if backup.exists() {
                let _ = fs::rename(&backup, active);
            }
            Err(ArchiveInstallError::Io(error.to_string()))
        }
    }
}

/// Returns whether a plugin ID is safe to use as a managed directory name.
///
/// The CLI uses the same predicate before changing persisted authorization,
/// so invalid IDs cannot leave behind grant rows.
#[must_use]
pub fn is_safe_plugin_id(plugin_id: &str) -> bool {
    !plugin_id.trim().is_empty()
        && plugin_id.len() <= 128
        && !plugin_id.contains('/')
        && !plugin_id.contains('\\')
        && !plugin_id.contains(':')
        && !plugin_id.contains("..")
        && !plugin_id.chars().any(char::is_control)
}

fn manifest_entry<'a>(entries: &'a [(String, Vec<u8>)], name: &str) -> Option<&'a [u8]> {
    entries
        .iter()
        .find(|(entry_name, _)| entry_name == name)
        .map(|(_, data)| data.as_slice())
}

fn format_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

impl PluginManifest {
    /// Creates a manifest from already validated `SemVer` strings.
    ///
    /// # Panics
    ///
    /// Panics when `version` or `host_api` is not valid `SemVer` syntax. Package
    /// parsing code should validate external input before constructing this
    /// value.
    pub fn new(id: impl Into<String>, version: impl AsRef<str>, host_api: impl AsRef<str>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            publisher: None,
            version: semver::Version::parse(version.as_ref())
                .expect("plugin manifest version must be valid SemVer"),
            host_api: semver::VersionReq::parse(host_api.as_ref())
                .expect("plugin host API range must be valid SemVer"),
            kind: PluginKind::Command,
            declared_permissions: DeclaredPermissions::default(),
            commands: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name.unwrap_or_else(|| self.id.clone());
        self
    }

    #[must_use]
    pub fn with_publisher(mut self, publisher: Option<String>) -> Self {
        self.publisher = publisher;
        self
    }

    ///
    /// # Panics
    ///
    /// Panics when a caller supplies an unknown legacy capability name. The
    /// builder is reserved for trusted in-tree construction; manifest parsing
    /// remains fallible and rejects unknown names.
    #[must_use]
    pub fn with_permissions<I, S>(mut self, permissions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        // This builder is used by trusted in-tree fixtures. Parsed manifests
        // use the fallible `DeclaredPermissions` path and never reach here.
        let names = permissions.into_iter().map(Into::into).collect::<Vec<_>>();
        self.declared_permissions = DeclaredPermissions::from_legacy_names(names)
            .expect("plugin builder permissions must use known capabilities");
        self
    }

    #[must_use]
    pub fn with_declared_permissions(mut self, permissions: DeclaredPermissions) -> Self {
        self.declared_permissions = permissions;
        self
    }

    #[must_use]
    pub fn with_commands<I>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = PluginCommand>,
    {
        self.commands = commands.into_iter().collect();
        self
    }

    #[must_use]
    pub const fn kind(&self) -> PluginKind {
        self.kind
    }

    #[must_use]
    pub fn has_permission(&self, permission: impl AsRef<str>) -> bool {
        self.declared_permissions
            .names()
            .any(|name| name == permission.as_ref())
    }

    #[must_use = "iterate over the declared plugin permissions"]
    pub fn permissions(&self) -> impl Iterator<Item = &str> {
        self.declared_permissions.names()
    }

    #[must_use]
    pub fn declared_permissions(&self) -> &DeclaredPermissions {
        &self.declared_permissions
    }

    #[must_use]
    pub fn commands(&self) -> &[PluginCommand] {
        &self.commands
    }

    #[must_use]
    pub fn summary(&self) -> PluginManifestSummary {
        PluginManifestSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            name: self.name.clone(),
            publisher: self.publisher.clone(),
            kind: self.kind,
            permissions: self.permissions().map(ToOwned::to_owned).collect(),
            declared_permissions: self.declared_permissions.clone(),
            commands: self.commands.clone(),
        }
    }

    #[must_use]
    pub fn supports(&self, host_version: impl AsRef<str>) -> bool {
        semver::Version::parse(host_version.as_ref())
            .map(|version| self.host_api.matches(&version))
            .unwrap_or(false)
    }

    fn validate_metadata(&self) -> Result<(), String> {
        if !is_safe_plugin_id(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.chars().any(char::is_control)
            || self.publisher.as_deref().is_some_and(|publisher| {
                publisher.len() > 256 || publisher.chars().any(char::is_control)
            })
            || self.declared_permissions.len() > 32
            || self.commands.len() > 128
            || self.commands.iter().any(|command| {
                command.id.trim().is_empty()
                    || command.id.len() > 128
                    || command.id.chars().any(char::is_control)
                    || command.title.len() > 256
                    || command.subtitle.len() > 512
            })
        {
            return Err("plugin manifest metadata exceeds host limits".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallState {
    Staging,
    Active,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallError {
    NotStaging,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallTransaction {
    plugin_id: String,
    state: InstallState,
}

impl InstallTransaction {
    #[must_use]
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            state: InstallState::Staging,
        }
    }

    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    #[must_use]
    pub const fn state(&self) -> InstallState {
        self.state
    }

    /// Atomically promotes staging to the active version pointer.
    ///
    /// # Errors
    ///
    /// Returns `NotStaging` when this transaction was already committed or failed.
    pub fn commit(&mut self) -> Result<(), InstallError> {
        if self.state != InstallState::Staging {
            return Err(InstallError::NotStaging);
        }
        self.state = InstallState::Active;
        Ok(())
    }

    pub fn fail(&mut self) {
        self.state = InstallState::Failed;
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ArchiveInstallError, ArchiveLimits, InstallState, InstallTransaction, PluginInteraction,
        PluginKind, PluginManifest, UninstallOutcome, inspect_archive, install_archive,
        install_archive_verified, pack_directory, parse_manifest_toml, parse_pet_descriptor,
        read_active_pet, read_active_plugin, retry_pending_delete_ids, rollback_plugin,
        set_plugin_enabled, sign_archive, uninstall_plugin, uninstall_plugin_with_pending_delete,
        validate_archive_paths,
    };

    const MINIMAL_COMPONENT: &[u8] = b"\0asm\x0d\0\x01\0";

    #[test]
    fn manifest_matches_host_version_range() {
        let manifest = PluginManifest::new("demo", "1.2.0", ">=1.0, <2.0");
        assert!(manifest.supports("1.8.1"));
        assert!(!manifest.supports("2.0.0"));
    }

    #[test]
    fn manifest_permissions_are_explicit_and_install_commit_is_atomic() {
        let manifest =
            PluginManifest::new("demo", "1.2.0", ">=1.0, <2.0").with_permissions(["storage"]);
        assert!(manifest.has_permission("storage"));
        let pet = parse_manifest_toml(
            "id = \"pet\"\nversion = \"0.1.0\"\nhost_api = \">=1.0, <2.0\"\nkind = \"desktop-pet\"\n",
        )
        .expect("pet manifest");
        assert_eq!(pet.kind(), PluginKind::DesktopPet);
        let descriptor = parse_pet_descriptor(
            r#"{"petId":"nova","displayName":"Nova","fallbackPetId":"nova","frames":["idle-0"],"recommendedActions":[]}"#,
        )
        .expect("pet descriptor");
        assert_eq!(descriptor.pet_id, "nova");
        assert!(!manifest.has_permission("process"));

        let mut install = InstallTransaction::new("demo");
        assert_eq!(install.state(), InstallState::Staging);
        install.commit().expect("staging can become active");
        assert_eq!(install.state(), InstallState::Active);
    }

    #[test]
    fn pet_descriptor_rejects_unsafe_paths_and_too_many_recommendations() {
        let unsafe_frame = parse_pet_descriptor(
            r#"{"petId":"nova","displayName":"Nova","fallbackPetId":"nova","frames":["../idle"],"recommendedActions":[]}"#,
        );
        assert!(unsafe_frame.is_err());

        let too_many_actions = parse_pet_descriptor(
            r#"{"petId":"nova","displayName":"Nova","fallbackPetId":"nova","frames":["idle"],"recommendedActions":["a","b","c","d"]}"#,
        );
        assert!(too_many_actions.is_err());
    }

    #[test]
    fn manifest_parser_and_archive_path_guard_reject_escape_inputs() {
        let manifest = parse_manifest_toml(
            "id = \"demo\"\nversion = \"1.2.0\"\nhost_api = \">=1.0, <2.0\"\npermissions = [\"storage\"]\n",
        )
        .expect("valid manifest");
        assert!(manifest.has_permission("storage"));
        assert!(validate_archive_paths(["novahub.toml", "plugin.wasm"]).is_ok());
        assert!(validate_archive_paths(["../outside.txt"]).is_err());
        assert!(validate_archive_paths(["C:/outside.txt"]).is_err());
        assert!(
            parse_manifest_toml(
                "id = \"bad:plugin\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n"
            )
            .is_err()
        );
    }

    #[test]
    fn manifest_parser_keeps_legacy_fields_and_reads_summary_metadata() {
        let manifest = parse_manifest_toml(
            r#"
id = "com.example.translate"
name = "Translate"
publisher = "Example Labs"
version = "1.2.0"
plugin_api = ">=1.1, <2.0"

[[commands]]
id = "translate"
title = "Translate text"
subtitle = "Translate selected text"

[permissions]
network = ["https://api.example.com"]
clipboard = ["read"]
"#,
        )
        .expect("documented manifest format parses");

        assert_eq!(manifest.name, "Translate");
        assert_eq!(manifest.publisher.as_deref(), Some("Example Labs"));
        assert_eq!(
            manifest.permissions().collect::<Vec<_>>(),
            ["clipboard", "network"]
        );
        assert_eq!(manifest.commands()[0].id, "translate");
        assert_eq!(manifest.commands()[0].interaction, PluginInteraction::View);

        let one_shot = parse_manifest_toml(
            r#"
id = "com.example.oneshot"
version = "1.0.0"
host_api = ">=1.1, <2.0"

[[commands]]
id = "format"
title = "Format"
interaction = "one-shot"
"#,
        )
        .expect("one-shot command parses");
        assert_eq!(
            one_shot.commands()[0].interaction,
            PluginInteraction::OneShot
        );
        assert!(parse_manifest_toml(
            "id = \"bad-command\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n[[commands]]\nid = \"format\"\ninteraction = \"background\"\n"
        )
        .is_err());

        let legacy = parse_manifest_toml(
            "id = \"legacy\"\nversion = \"0.1.0\"\nhost_api = \">=1.0, <2.0\"\npermissions = [\"storage\"]\n",
        )
        .expect("legacy manifest remains compatible");
        assert_eq!(legacy.name, "legacy");
        assert!(legacy.has_permission("storage"));
    }

    #[test]
    fn archive_install_extracts_to_version_and_updates_active_pointer() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("novahub.toml", options)
            .expect("manifest entry");
        writer
            .write_all(b"id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n")
            .expect("manifest bytes");
        writer
            .start_file("plugin.wasm", options)
            .expect("wasm entry");
        writer
            .write_all(MINIMAL_COMPONENT)
            .expect("component bytes");
        let archive = writer.finish().expect("zip finishes").into_inner();

        let preview =
            inspect_archive(&archive, "1.1.0", ArchiveLimits::default()).expect("archive preview");
        assert_eq!(preview.name, "demo");
        assert_eq!(preview.version, semver::Version::new(1, 0, 0));
        assert!(matches!(
            inspect_archive(&archive, "2.0.0", ArchiveLimits::default()),
            Err(ArchiveInstallError::IncompatibleHost)
        ));
        assert!(matches!(
            inspect_archive(b"not a zip", "1.1.0", ArchiveLimits::default()),
            Err(ArchiveInstallError::InvalidArchive(_))
        ));

        let root = unique_temp_dir("install");
        let receipt = install_archive(&archive, &root, "1.1.0", None, ArchiveLimits::default())
            .expect("archive installs");
        assert_eq!(receipt.plugin_id, "demo");
        assert!(receipt.active_pointer.is_file());
        assert!(root.join("demo/versions/1.0.0/plugin.wasm").is_file());
        let active = read_active_plugin(&root, "demo").expect("active pointer parses");
        assert_eq!(active.version.to_string(), "1.0.0");
        assert!(active.enabled);
        let disabled = set_plugin_enabled(&root, "demo", false).expect("disable plugin");
        assert!(!disabled.enabled);
        assert!(
            !read_active_plugin(&root, "demo")
                .expect("disabled pointer")
                .enabled
        );
        assert!(
            set_plugin_enabled(&root, "demo", true)
                .expect("enable plugin")
                .enabled
        );
        let pointer = std::fs::read_to_string(receipt.active_pointer).expect("pointer readable");
        assert!(pointer.contains("1.0.0"));
        uninstall_plugin(&root, "demo").expect("plugin uninstall");
        assert!(!root.join("demo").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn archive_preview_applies_the_same_metadata_limits_as_install() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("novahub.toml", options)
            .expect("manifest entry");
        writer
            .write_all(b"id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n")
            .expect("manifest bytes");
        writer
            .start_file("plugin.wasm", options)
            .expect("component entry");
        writer
            .write_all(MINIMAL_COMPONENT)
            .expect("component bytes");
        let archive = writer.finish().expect("zip finishes").into_inner();
        let limits = ArchiveLimits {
            max_entries: 4,
            max_compressed_bytes: 1,
            max_uncompressed_bytes: 1024,
            max_entry_bytes: 1024,
        };

        assert!(matches!(
            inspect_archive(&archive, "1.1.0", limits),
            Err(ArchiveInstallError::InvalidArchive(message))
                if message.contains("compressed or extracted size limits")
        ));
    }

    #[test]
    fn archive_preview_rejects_duplicate_paths_before_manifest_parsing() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("novahub.toml", options)
            .expect("manifest entry");
        writer
            .write_all(b"id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n")
            .expect("manifest bytes");
        for path in ["assets/item", "assets\\item"] {
            writer.start_file(path, options).expect("path entry");
            writer.write_all(b"duplicate").expect("entry bytes");
        }
        let archive = writer.finish().expect("zip finishes").into_inner();

        assert!(matches!(
            inspect_archive(&archive, "1.1.0", ArchiveLimits::default()),
            Err(ArchiveInstallError::InvalidArchive(message))
                if message == "archive contains duplicate paths"
        ));
    }

    #[test]
    fn active_desktop_pet_loads_only_bounded_svg_frames() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("novahub.toml", options)
            .expect("manifest entry");
        writer
            .write_all(
                b"id = \"custom.pet\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\nkind = \"desktop-pet\"\n",
            )
            .expect("manifest bytes");
        writer
            .start_file("pet.json", options)
            .expect("descriptor entry");
        writer
            .write_all(
                br#"{"petId":"custom","displayName":"Custom","fallbackPetId":"nova","frames":["assets/idle.svg","assets/success.svg"],"recommendedActions":[]}"#,
            )
            .expect("descriptor bytes");
        writer
            .start_file("assets/idle.svg", options)
            .expect("idle entry");
        writer
            .write_all(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>")
            .expect("idle bytes");
        writer
            .start_file("assets/success.svg", options)
            .expect("success entry");
        writer
            .write_all(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>")
            .expect("success bytes");
        let archive = writer.finish().expect("zip finishes").into_inner();

        let root = unique_temp_dir("active-pet");
        install_archive(&archive, &root, "1.1.0", None, ArchiveLimits::default())
            .expect("pet archive installs");
        let pet = read_active_pet(&root, "custom.pet").expect("active pet loads");
        assert_eq!(pet.descriptor.display_name, "Custom");
        assert_eq!(pet.frames.len(), 2);
        assert!(pet.frames.iter().all(|frame| !frame.bytes.is_empty()));
        set_plugin_enabled(&root, "custom.pet", false).expect("disable pet");
        assert!(matches!(
            read_active_pet(&root, "custom.pet"),
            Err(ArchiveInstallError::InvalidArchive(message)) if message == "desktop-pet is disabled"
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pending_delete_markers_are_retried_and_active_paths_cannot_escape_versions() {
        let root = unique_temp_dir("pending-delete");
        let marker_root = root.join(".pending-delete");
        std::fs::create_dir_all(&marker_root).expect("marker root");
        let target = root.join("demo");
        std::fs::create_dir_all(&target).expect("plugin directory");
        std::fs::write(
            marker_root.join("demo.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "demo",
                "path": target,
            }))
            .expect("marker JSON"),
        )
        .expect("marker");
        assert_eq!(
            retry_pending_delete_ids(&root).expect("retry ids"),
            vec!["demo"]
        );
        assert!(!marker_root.exists());
        assert!(!root.join("demo").exists());

        std::fs::create_dir_all(root.join("outside")).expect("outside");
        std::fs::create_dir_all(root.join("demo/versions/1.0.0")).expect("version");
        std::fs::write(
            root.join("demo/active.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "demo",
                "version": "1.0.0",
                "path": root.join("outside"),
            }))
            .expect("pointer JSON"),
        )
        .expect("pointer");
        assert!(read_active_plugin(&root, "demo").is_err());
        assert_eq!(
            uninstall_plugin_with_pending_delete(&root, "missing").expect("uninstall"),
            UninstallOutcome::Removed
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollback_selects_the_newest_non_active_version_and_preserves_enabled_state() {
        fn archive(version: &str) -> Vec<u8> {
            let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            writer
                .start_file("novahub.toml", options)
                .expect("manifest entry");
            writer
                .write_all(
                    format!(
                        "id = \"rollback\"\nversion = \"{version}\"\nhost_api = \">=1.0, <2.0\"\n"
                    )
                    .as_bytes(),
                )
                .expect("manifest bytes");
            writer
                .start_file("plugin.wasm", options)
                .expect("wasm entry");
            writer
                .write_all(MINIMAL_COMPONENT)
                .expect("component bytes");
            writer.finish().expect("zip finishes").into_inner()
        }

        let root = unique_temp_dir("rollback");
        install_archive(
            &archive("1.0.0"),
            &root,
            "1.1.0",
            None,
            ArchiveLimits::default(),
        )
        .expect("first version installs");
        install_archive(
            &archive("1.2.0"),
            &root,
            "1.1.0",
            None,
            ArchiveLimits::default(),
        )
        .expect("second version installs");
        set_plugin_enabled(&root, "rollback", false).expect("disable active version");
        let active = rollback_plugin(&root, "rollback").expect("rollback succeeds");
        assert_eq!(active.version, semver::Version::new(1, 0, 0));
        assert!(!active.enabled);
        let restored = rollback_plugin(&root, "rollback").expect("second rollback toggles back");
        assert_eq!(restored.version, semver::Version::new(1, 2, 0));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn verified_archive_install_rejects_tampering_and_accepts_signature() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("novahub.toml", options)
            .expect("manifest entry");
        writer
            .write_all(b"id = \"signed\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n")
            .expect("manifest bytes");
        writer
            .start_file("plugin.wasm", options)
            .expect("wasm entry");
        writer
            .write_all(MINIMAL_COMPONENT)
            .expect("component bytes");
        let archive = writer.finish().expect("zip finishes").into_inner();
        let (signature, public_key) = sign_archive(&archive, &[3_u8; 32]).expect("sign");
        let root = unique_temp_dir("verified-install");
        install_archive_verified(
            &archive,
            &root,
            "1.1.0",
            None,
            Some((&public_key, &signature)),
            ArchiveLimits::default(),
        )
        .expect("signed archive installs");

        let mut tampered = archive.clone();
        tampered[0] ^= 1;
        let error = install_archive_verified(
            &tampered,
            &root,
            "1.1.0",
            None,
            Some((&public_key, &signature)),
            ArchiveLimits::default(),
        )
        .expect_err("tampered archive must be rejected");
        assert_eq!(error, ArchiveInstallError::InvalidSignature);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn archive_install_rejects_missing_entry_and_zip_slip() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("novahub.toml", options)
            .expect("manifest entry");
        writer
            .write_all(b"id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n")
            .expect("manifest bytes");
        writer
            .start_file("../escape.wasm", options)
            .expect("unsafe entry");
        writer.write_all(b"bad").expect("unsafe bytes");
        let archive = writer.finish().expect("zip finishes").into_inner();
        let error = install_archive(
            &archive,
            unique_temp_dir("reject"),
            "1.1.0",
            None,
            ArchiveLimits::default(),
        )
        .expect_err("zip slip must be rejected");
        assert!(matches!(error, ArchiveInstallError::InvalidArchive(_)));
    }

    #[test]
    fn archive_install_rejects_a_core_module_disguised_as_a_component() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("novahub.toml", options)
            .expect("manifest entry");
        writer
            .write_all(b"id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n")
            .expect("manifest bytes");
        writer
            .start_file("plugin.wasm", options)
            .expect("component entry");
        writer.write_all(b"\0asm\x01\0\0\0").expect("module bytes");
        let archive = writer.finish().expect("zip finishes").into_inner();

        let error = install_archive(
            &archive,
            unique_temp_dir("module-disguised-component"),
            "1.1.0",
            None,
            ArchiveLimits::default(),
        )
        .expect_err("core module must not pass Component validation");
        assert!(
            matches!(error, ArchiveInstallError::InvalidArchive(message) if message.contains("WebAssembly Component"))
        );
    }

    #[test]
    fn ed25519_verifier_accepts_signed_archive_bytes_and_rejects_tampering() {
        use ring::signature::{Ed25519KeyPair, KeyPair};

        let key_pair = Ed25519KeyPair::from_seed_unchecked(&[7_u8; 32]).expect("test key");
        let message = b"signed package bytes";
        let signature = key_pair.sign(message);
        super::verify_ed25519(key_pair.public_key().as_ref(), message, signature.as_ref())
            .expect("signature verifies");
        assert!(
            super::verify_ed25519(
                key_pair.public_key().as_ref(),
                b"tampered package bytes",
                signature.as_ref()
            )
            .is_err()
        );
    }

    #[test]
    fn pack_directory_is_bounded_and_signed_archive_roundtrips() {
        let root = unique_temp_dir("pack");
        std::fs::create_dir_all(root.join("assets")).expect("source directory");
        std::fs::write(
            root.join("novahub.toml"),
            "id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n",
        )
        .expect("manifest");
        std::fs::write(root.join("plugin.wasm"), MINIMAL_COMPONENT).expect("component");
        std::fs::write(root.join("assets/icon.txt"), b"icon").expect("asset");
        let archive_path = root.with_extension("novahub-plugin");
        pack_directory(&root, &archive_path, ArchiveLimits::default()).expect("package");
        let archive = std::fs::read(&archive_path).expect("archive");
        let (signature, public_key) = sign_archive(&archive, &[9_u8; 32]).expect("sign");
        super::verify_ed25519(&public_key, &archive, &signature).expect("verify package");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(archive_path);
    }

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("novahub-plugin-manager-{label}-{suffix}"))
    }
}
