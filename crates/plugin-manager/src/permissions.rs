use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

const DEFAULT_STORAGE_QUOTA_BYTES: u64 = 10 * 1024 * 1024;
const MAX_PERMISSION_ENTRIES: usize = 32;
const MAX_HTTP_ORIGINS: usize = 32;
const MAX_REASON_BYTES: usize = 512;
const MAX_STORAGE_KEY_BYTES: usize = 128;
const MAX_STORAGE_VALUE_BYTES: usize = 64 * 1024;

/// A capability name with security-sensitive behavior owned by the host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Clipboard,
    Files,
    Http,
    Logging,
    Notification,
    Storage,
}

impl Capability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clipboard => "clipboard",
            Self::Files => "files",
            Self::Http => "http",
            Self::Logging => "logging",
            Self::Notification => "notification",
            Self::Storage => "storage",
        }
    }
}

/// The only file scope exposed by the MVP. Paths are represented by host-issued
/// handles at runtime, never by strings from a manifest.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileSource {
    UserSelected,
}

/// A normalized, bounded scope for one declared capability.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PermissionScope {
    Clipboard {
        read: bool,
        write: bool,
    },
    Files {
        read: bool,
        write: bool,
        source: FileSource,
    },
    Http {
        origins: BTreeSet<String>,
    },
    Logging,
    Notification,
    Storage {
        quota_bytes: u64,
    },
}

impl PermissionScope {
    fn intersect(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (
                Self::Clipboard {
                    read: left_read,
                    write: left_write,
                },
                Self::Clipboard {
                    read: right_read,
                    write: right_write,
                },
            ) => Some(Self::Clipboard {
                read: *left_read && *right_read,
                write: *left_write && *right_write,
            }),
            (
                Self::Files {
                    read: left_read,
                    write: left_write,
                    source: left_source,
                },
                Self::Files {
                    read: right_read,
                    write: right_write,
                    source: right_source,
                },
            ) if left_source == right_source => Some(Self::Files {
                read: *left_read && *right_read,
                write: *left_write && *right_write,
                source: *left_source,
            }),
            (Self::Http { origins: left }, Self::Http { origins: right }) => Some(Self::Http {
                origins: left.intersection(right).cloned().collect(),
            }),
            (Self::Logging, Self::Logging) => Some(Self::Logging),
            (Self::Notification, Self::Notification) => Some(Self::Notification),
            (Self::Storage { quota_bytes: left }, Self::Storage { quota_bytes: right }) => {
                Some(Self::Storage {
                    quota_bytes: (*left).min(*right),
                })
            }
            _ => None,
        }
    }

    fn is_subset_of(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Clipboard {
                    read: left_read,
                    write: left_write,
                },
                Self::Clipboard {
                    read: right_read,
                    write: right_write,
                },
            ) => (!left_read || *right_read) && (!left_write || *right_write),
            (
                Self::Files {
                    read: left_read,
                    write: left_write,
                    source: left_source,
                },
                Self::Files {
                    read: right_read,
                    write: right_write,
                    source: right_source,
                },
            ) => {
                left_source == right_source
                    && (!left_read || *right_read)
                    && (!left_write || *right_write)
            }
            (Self::Http { origins: left }, Self::Http { origins: right }) => left.is_subset(right),
            (Self::Logging, Self::Logging) | (Self::Notification, Self::Notification) => true,
            (Self::Storage { quota_bytes: left }, Self::Storage { quota_bytes: right }) => {
                left <= right
            }
            _ => false,
        }
    }
}

/// A manifest-declared capability and its user-facing explanation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeclaredPermission {
    pub capability: Capability,
    pub scope: PermissionScope,
    pub reason: Option<String>,
}

/// Normalized permissions retained by the plugin manager.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeclaredPermissions {
    entries: BTreeMap<Capability, DeclaredPermission>,
    display_names: BTreeSet<String>,
}

impl DeclaredPermissions {
    pub fn iter(&self) -> impl Iterator<Item = &DeclaredPermission> {
        self.entries.values()
    }

    #[must_use]
    pub fn get(&self, capability: Capability) -> Option<&DeclaredPermission> {
        self.entries.get(&capability)
    }

    #[must_use]
    pub fn contains(&self, capability: Capability) -> bool {
        self.entries.contains_key(&capability)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.display_names.iter().map(String::as_str)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Intersects two policy layers. The returned set contains no publisher
    /// reason because effective authorization is a host decision, not UI copy.
    ///
    /// # Panics
    ///
    /// Panics only if an internal capability map violates its uniqueness
    /// invariant. The intersection iterates unique map keys, so valid inputs
    /// cannot trigger this condition.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        let mut result = Self::default();
        for (capability, left) in &self.entries {
            let Some(right) = other.entries.get(capability) else {
                continue;
            };
            let Some(scope) = left.scope.intersect(&right.scope) else {
                continue;
            };
            result
                .insert(
                    DeclaredPermission {
                        capability: *capability,
                        scope,
                        reason: None,
                    },
                    None,
                )
                .expect("intersected permission capabilities are unique and bounded");
        }
        result
    }

    /// Returns a normalized copy without one capability.
    ///
    /// This is used to narrow a persisted user grant. Publisher-facing reason
    /// text and legacy display aliases are intentionally not copied into the
    /// authorization record.
    #[must_use]
    pub fn without(&self, removed: Capability) -> Self {
        let mut result = Self::default();
        for permission in self.entries.values() {
            if permission.capability == removed {
                continue;
            }
            let capability = permission.capability;
            result.entries.insert(
                capability,
                DeclaredPermission {
                    capability,
                    scope: permission.scope.clone(),
                    reason: None,
                },
            );
            result.display_names.insert(capability.as_str().to_owned());
        }
        result
    }

    /// Builds the legacy capability-only form with conservative empty scopes.
    pub(crate) fn from_legacy_names<I, S>(names: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut result = Self::default();
        for name in names {
            let name = name.as_ref();
            let (display_name, permission) = legacy_permission(name)?;
            result.insert(permission, Some(display_name))?;
        }
        Ok(result)
    }

    pub(crate) fn insert(
        &mut self,
        permission: DeclaredPermission,
        display_name: Option<&str>,
    ) -> Result<(), String> {
        let canonical_name = permission.capability.as_str();
        if self.entries.len() >= MAX_PERMISSION_ENTRIES
            && !self.entries.contains_key(&permission.capability)
        {
            return Err("plugin manifest declares too many permissions".into());
        }
        if self.entries.contains_key(&permission.capability) {
            return Err(format!(
                "duplicate permission capability: {}",
                permission.capability.as_str()
            ));
        }
        self.entries.insert(permission.capability, permission);
        self.display_names
            .insert(display_name.unwrap_or(canonical_name).to_owned());
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The result of applying declaration, user grant and host policy layers.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EffectiveGrant {
    permissions: DeclaredPermissions,
}

impl EffectiveGrant {
    #[must_use]
    pub fn from_layers(
        declared: &DeclaredPermissions,
        user_grant: &DeclaredPermissions,
        host_policy: &DeclaredPermissions,
    ) -> Self {
        Self {
            permissions: declared.intersect(user_grant).intersect(host_policy),
        }
    }

    #[must_use]
    pub fn allows(&self, capability: Capability) -> bool {
        self.permissions.contains(capability)
    }

    #[must_use]
    pub fn permissions(&self) -> &DeclaredPermissions {
        &self.permissions
    }
}

/// Identity attached to every capability call. It is intentionally smaller
/// than a process identity so a sibling Host can validate it on each request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginIdentity {
    plugin_id: String,
    session_id: String,
}

impl PluginIdentity {
    #[must_use]
    pub fn new(plugin_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            session_id: session_id.into(),
        }
    }

    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Opaque host-issued file handle. A path never crosses the plugin boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostFileHandle {
    plugin_id: String,
    session_id: String,
    token: String,
}

impl HostFileHandle {
    /// Issues a handle after a host file picker has selected a resource.
    /// Callers must retain the actual OS handle in a host-owned table.
    #[must_use]
    pub fn issue(
        plugin_id: impl Into<String>,
        session_id: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            session_id: session_id.into(),
            token: token.into(),
        }
    }

    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    #[must_use]
    fn belongs_to(&self, identity: &PluginIdentity) -> bool {
        self.plugin_id == identity.plugin_id
            && self.session_id == identity.session_id
            && !self.token.is_empty()
    }
}

/// One host capability operation. Each variant is checked independently.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityRequest {
    ClipboardRead,
    ClipboardWrite,
    FilesSelect,
    FilesRead { handle: HostFileHandle },
    FilesWrite { handle: HostFileHandle },
    Http { origin: String },
    StorageWrite { total_bytes: u64 },
    StoragePut { key: String, value: Vec<u8> },
    StorageGet { key: String },
    Logging,
    Notification,
}

/// A capability call that passed the broker. The host adapter consumes this
/// value and performs the operation; plugins never receive it directly.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthorizedCapability {
    ClipboardRead,
    ClipboardWrite,
    FilesSelect,
    FilesRead {
        handle: HostFileHandle,
    },
    FilesWrite {
        handle: HostFileHandle,
    },
    Http {
        origin: String,
    },
    StorageWrite {
        total_bytes: u64,
    },
    StoragePut {
        key: String,
        value: Vec<u8>,
        quota_bytes: u64,
    },
    StorageGet {
        key: String,
    },
    Logging,
    Notification,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDenied {
    NotGranted,
    ScopeExceeded,
    HandleMismatch,
    InvalidOrigin,
    InvalidInput,
}

impl std::fmt::Display for CapabilityDenied {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NotGranted => "capability not granted",
            Self::ScopeExceeded => "capability scope exceeded",
            Self::HandleMismatch => "file handle does not belong to the session",
            Self::InvalidOrigin => "origin is invalid or not HTTPS",
            Self::InvalidInput => "capability input is invalid",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CapabilityDenied {}

/// Stateless broker used at every host capability boundary.
#[derive(Clone, Copy, Debug, Default)]
pub struct CapabilityBroker;

impl CapabilityBroker {
    /// Rechecks plugin identity, effective grant and operation arguments.
    ///
    /// # Errors
    ///
    /// Returns a deterministic denial when a grant, scope, origin, or file
    /// handle does not match the current request.
    pub fn authorize(
        &self,
        identity: &PluginIdentity,
        grant: &EffectiveGrant,
        request: CapabilityRequest,
    ) -> Result<AuthorizedCapability, CapabilityDenied> {
        match request {
            CapabilityRequest::ClipboardRead => {
                ensure_clipboard(grant, true)?;
                Ok(AuthorizedCapability::ClipboardRead)
            }
            CapabilityRequest::ClipboardWrite => {
                ensure_clipboard(grant, false)?;
                Ok(AuthorizedCapability::ClipboardWrite)
            }
            CapabilityRequest::FilesSelect => {
                ensure_file_selection(grant)?;
                Ok(AuthorizedCapability::FilesSelect)
            }
            CapabilityRequest::FilesRead { handle } => {
                ensure_file(identity, grant, &handle, true)?;
                Ok(AuthorizedCapability::FilesRead { handle })
            }
            CapabilityRequest::FilesWrite { handle } => {
                ensure_file(identity, grant, &handle, false)?;
                Ok(AuthorizedCapability::FilesWrite { handle })
            }
            CapabilityRequest::Http { origin } => {
                let normalized = normalize_runtime_https_origin(&origin)
                    .map_err(|_| CapabilityDenied::InvalidOrigin)?;
                let Some(DeclaredPermission {
                    scope: PermissionScope::Http { origins },
                    ..
                }) = grant.permissions().get(Capability::Http)
                else {
                    return Err(CapabilityDenied::NotGranted);
                };
                if !origins.contains(&normalized) {
                    return Err(CapabilityDenied::ScopeExceeded);
                }
                Ok(AuthorizedCapability::Http { origin: normalized })
            }
            CapabilityRequest::StorageWrite { total_bytes } => {
                ensure_storage(grant, total_bytes)?;
                Ok(AuthorizedCapability::StorageWrite { total_bytes })
            }
            CapabilityRequest::StoragePut { key, value } => {
                ensure_storage_key(&key)?;
                if value.len() > MAX_STORAGE_VALUE_BYTES {
                    return Err(CapabilityDenied::ScopeExceeded);
                }
                let quota_bytes = storage_quota(grant)?;
                Ok(AuthorizedCapability::StoragePut {
                    key,
                    value,
                    quota_bytes,
                })
            }
            CapabilityRequest::StorageGet { key } => {
                ensure_storage_key(&key)?;
                ensure_storage(grant, 0)?;
                Ok(AuthorizedCapability::StorageGet { key })
            }
            CapabilityRequest::Logging => {
                ensure_simple(grant, Capability::Logging)?;
                Ok(AuthorizedCapability::Logging)
            }
            CapabilityRequest::Notification => {
                ensure_simple(grant, Capability::Notification)?;
                Ok(AuthorizedCapability::Notification)
            }
        }
    }
}

fn ensure_clipboard(grant: &EffectiveGrant, read: bool) -> Result<(), CapabilityDenied> {
    let Some(DeclaredPermission {
        scope:
            PermissionScope::Clipboard {
                read: allowed_read,
                write: allowed_write,
            },
        ..
    }) = grant.permissions().get(Capability::Clipboard)
    else {
        return Err(CapabilityDenied::NotGranted);
    };
    let allowed = if read { *allowed_read } else { *allowed_write };
    allowed.then_some(()).ok_or(CapabilityDenied::ScopeExceeded)
}

fn ensure_file(
    identity: &PluginIdentity,
    grant: &EffectiveGrant,
    handle: &HostFileHandle,
    read: bool,
) -> Result<(), CapabilityDenied> {
    if !handle.belongs_to(identity) {
        return Err(CapabilityDenied::HandleMismatch);
    }
    let Some(DeclaredPermission {
        scope:
            PermissionScope::Files {
                read: allowed_read,
                write: allowed_write,
                source: FileSource::UserSelected,
            },
        ..
    }) = grant.permissions().get(Capability::Files)
    else {
        return Err(CapabilityDenied::NotGranted);
    };
    let allowed = if read { *allowed_read } else { *allowed_write };
    allowed.then_some(()).ok_or(CapabilityDenied::ScopeExceeded)
}

fn ensure_file_selection(grant: &EffectiveGrant) -> Result<(), CapabilityDenied> {
    let Some(DeclaredPermission {
        scope:
            PermissionScope::Files {
                read: true,
                source: FileSource::UserSelected,
                ..
            },
        ..
    }) = grant.permissions().get(Capability::Files)
    else {
        return Err(CapabilityDenied::NotGranted);
    };
    Ok(())
}

fn ensure_storage(grant: &EffectiveGrant, total_bytes: u64) -> Result<(), CapabilityDenied> {
    let quota_bytes = storage_quota(grant)?;
    if total_bytes > quota_bytes {
        return Err(CapabilityDenied::ScopeExceeded);
    }
    Ok(())
}

fn storage_quota(grant: &EffectiveGrant) -> Result<u64, CapabilityDenied> {
    let Some(DeclaredPermission {
        scope: PermissionScope::Storage { quota_bytes },
        ..
    }) = grant.permissions().get(Capability::Storage)
    else {
        return Err(CapabilityDenied::NotGranted);
    };
    Ok(*quota_bytes)
}

fn ensure_storage_key(key: &str) -> Result<(), CapabilityDenied> {
    if key.trim().is_empty()
        || key.len() > MAX_STORAGE_KEY_BYTES
        || key.chars().any(char::is_control)
    {
        return Err(CapabilityDenied::InvalidInput);
    }
    Ok(())
}

fn ensure_simple(grant: &EffectiveGrant, capability: Capability) -> Result<(), CapabilityDenied> {
    grant
        .permissions()
        .get(capability)
        .map(|_| ())
        .ok_or(CapabilityDenied::NotGranted)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionChange {
    Added,
    Expanded,
    Narrowed,
    Removed,
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionDiff {
    pub capability: Capability,
    pub change: PermissionChange,
    pub before: Option<PermissionScope>,
    pub after: Option<PermissionScope>,
    pub reason: Option<String>,
}

/// Computes the install/update diff used by preview and permission UX.
#[must_use]
pub fn diff_permissions(
    before: &DeclaredPermissions,
    after: &DeclaredPermissions,
) -> Vec<PermissionDiff> {
    let capabilities = before
        .entries
        .keys()
        .chain(after.entries.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    capabilities
        .into_iter()
        .map(|capability| {
            let previous = before.entries.get(&capability);
            let current = after.entries.get(&capability);
            let change = match (previous, current) {
                (None, Some(_)) => PermissionChange::Added,
                (Some(_), None) => PermissionChange::Removed,
                (Some(previous), Some(current)) if previous.scope == current.scope => {
                    PermissionChange::Unchanged
                }
                (Some(previous), Some(current)) if previous.scope.is_subset_of(&current.scope) => {
                    PermissionChange::Expanded
                }
                (Some(previous), Some(current)) if current.scope.is_subset_of(&previous.scope) => {
                    PermissionChange::Narrowed
                }
                // Incomparable scopes are treated as expansion so a preview
                // never silently hides a potentially broader request.
                (Some(_), Some(_)) => PermissionChange::Expanded,
                (None, None) => PermissionChange::Unchanged,
            };
            PermissionDiff {
                capability,
                change,
                before: previous.map(|permission| permission.scope.clone()),
                after: current.map(|permission| permission.scope.clone()),
                reason: current.and_then(|permission| permission.reason.clone()),
            }
        })
        .collect()
}

pub(crate) fn parse_permissions(value: &toml::Value) -> Result<DeclaredPermissions, String> {
    match value {
        toml::Value::Array(names) => {
            let names = names
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| "legacy permission names must be strings".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            DeclaredPermissions::from_legacy_names(names)
        }
        toml::Value::Table(entries) => parse_permission_table(entries),
        _ => Err("permissions must be an array or table".into()),
    }
}

fn parse_permission_table(
    entries: &toml::map::Map<String, toml::Value>,
) -> Result<DeclaredPermissions, String> {
    let mut result = DeclaredPermissions::default();
    for (name, value) in entries {
        let (display_name, permission) = match name.as_str() {
            "clipboard" => (name.as_str(), parse_clipboard(value)?),
            "files" => (name.as_str(), parse_files(value)?),
            "http" | "network" => (name.as_str(), parse_http(value)?),
            "logging" => (
                name.as_str(),
                parse_reason_only(value, Capability::Logging)?,
            ),
            "notification" => (
                name.as_str(),
                parse_reason_only(value, Capability::Notification)?,
            ),
            "storage" => (name.as_str(), parse_storage(value)?),
            _ => return Err(format!("unknown permission capability: {name}")),
        };
        result.insert(permission, Some(display_name))?;
    }
    Ok(result)
}

fn legacy_permission(name: &str) -> Result<(&str, DeclaredPermission), String> {
    let permission = match name {
        "clipboard" => DeclaredPermission {
            capability: Capability::Clipboard,
            scope: PermissionScope::Clipboard {
                read: true,
                write: true,
            },
            reason: None,
        },
        "files" => DeclaredPermission {
            capability: Capability::Files,
            scope: PermissionScope::Files {
                read: true,
                write: true,
                source: FileSource::UserSelected,
            },
            reason: None,
        },
        "http" | "network" => DeclaredPermission {
            capability: Capability::Http,
            scope: PermissionScope::Http {
                origins: BTreeSet::new(),
            },
            reason: None,
        },
        "logging" => DeclaredPermission {
            capability: Capability::Logging,
            scope: PermissionScope::Logging,
            reason: None,
        },
        "notification" => DeclaredPermission {
            capability: Capability::Notification,
            scope: PermissionScope::Notification,
            reason: None,
        },
        "storage" => DeclaredPermission {
            capability: Capability::Storage,
            scope: PermissionScope::Storage {
                quota_bytes: DEFAULT_STORAGE_QUOTA_BYTES,
            },
            reason: None,
        },
        _ => return Err(format!("unknown permission capability: {name}")),
    };
    Ok((name, permission))
}

fn parse_clipboard(value: &toml::Value) -> Result<DeclaredPermission, String> {
    if let Some(access) = value.as_array() {
        return parse_clipboard_access(access, None);
    }
    let table = value
        .as_table()
        .ok_or_else(|| "clipboard permission must be a table or access array".to_owned())?;
    reject_unknown_fields(table, &["access", "reason"])?;
    let access = table
        .get("access")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "clipboard.access must be an array".to_owned())?;
    parse_clipboard_access(access, parse_reason(table.get("reason"))?)
}

fn parse_clipboard_access(
    access: &[toml::Value],
    reason: Option<String>,
) -> Result<DeclaredPermission, String> {
    let mut read = false;
    let mut write = false;
    for value in access {
        match value.as_str() {
            Some("read") => read = true,
            Some("write") => write = true,
            Some(other) => return Err(format!("unknown clipboard access: {other}")),
            None => return Err("clipboard access values must be strings".into()),
        }
    }
    if !read && !write {
        return Err("clipboard.access must request read or write".into());
    }
    Ok(DeclaredPermission {
        capability: Capability::Clipboard,
        scope: PermissionScope::Clipboard { read, write },
        reason,
    })
}

fn parse_files(value: &toml::Value) -> Result<DeclaredPermission, String> {
    if let Some(access) = value.as_array() {
        let mut read = false;
        let mut write = false;
        for value in access {
            match value.as_str() {
                Some("read") => read = true,
                Some("write") => write = true,
                Some(other) => return Err(format!("unknown files access: {other}")),
                None => return Err("files access values must be strings".into()),
            }
        }
        return file_permission(read, write, None);
    }
    let table = value
        .as_table()
        .ok_or_else(|| "files permission must be a table or access array".to_owned())?;
    reject_unknown_fields(table, &["read", "write", "reason"])?;
    let read = parse_file_intent(table.get("read"))?;
    let write = parse_file_intent(table.get("write"))?;
    file_permission(read, write, parse_reason(table.get("reason"))?)
}

fn parse_file_intent(value: Option<&toml::Value>) -> Result<bool, String> {
    match value {
        None => Ok(false),
        Some(value) => match value.as_str() {
            Some("user-selected") => Ok(true),
            Some(other) => Err(format!("unsupported file source: {other}")),
            None => Err("file scope must be \"user-selected\"".into()),
        },
    }
}

fn file_permission(
    read: bool,
    write: bool,
    reason: Option<String>,
) -> Result<DeclaredPermission, String> {
    if !read && !write {
        return Err("files permission must request read or write".into());
    }
    Ok(DeclaredPermission {
        capability: Capability::Files,
        scope: PermissionScope::Files {
            read,
            write,
            source: FileSource::UserSelected,
        },
        reason,
    })
}

fn parse_http(value: &toml::Value) -> Result<DeclaredPermission, String> {
    if let Some(origins) = value.as_array() {
        return http_permission(origins, None);
    }
    let table = value
        .as_table()
        .ok_or_else(|| "http permission must be a table or origins array".to_owned())?;
    reject_unknown_fields(table, &["origins", "reason"])?;
    let origins = table
        .get("origins")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "http.origins must be an array".to_owned())?;
    http_permission(origins, parse_reason(table.get("reason"))?)
}

fn http_permission(
    origins: &[toml::Value],
    reason: Option<String>,
) -> Result<DeclaredPermission, String> {
    if origins.len() > MAX_HTTP_ORIGINS {
        return Err("http permission declares too many origins".into());
    }
    let mut normalized = BTreeSet::new();
    for origin in origins {
        let value = origin
            .as_str()
            .ok_or_else(|| "http origins must be strings".to_owned())?;
        normalized.insert(normalize_https_origin(value)?);
    }
    Ok(DeclaredPermission {
        capability: Capability::Http,
        scope: PermissionScope::Http {
            origins: normalized,
        },
        reason,
    })
}

fn parse_storage(value: &toml::Value) -> Result<DeclaredPermission, String> {
    if value.is_bool() {
        return storage_permission(DEFAULT_STORAGE_QUOTA_BYTES, None);
    }
    let table = value
        .as_table()
        .ok_or_else(|| "storage permission must be a table".to_owned())?;
    reject_unknown_fields(table, &["quota_bytes", "reason"])?;
    let quota = table
        .get("quota_bytes")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| "storage.quota_bytes must be a positive integer".to_owned())?;
    let quota = u64::try_from(quota).map_err(|_| "storage quota is invalid".to_owned())?;
    storage_permission(quota, parse_reason(table.get("reason"))?)
}

fn storage_permission(
    quota_bytes: u64,
    reason: Option<String>,
) -> Result<DeclaredPermission, String> {
    if quota_bytes == 0 || quota_bytes > 128 * 1024 * 1024 {
        return Err("storage quota is outside the host limit".into());
    }
    Ok(DeclaredPermission {
        capability: Capability::Storage,
        scope: PermissionScope::Storage { quota_bytes },
        reason,
    })
}

fn parse_reason(value: Option<&toml::Value>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let reason = value
        .as_str()
        .ok_or_else(|| "permission reason must be a string".to_owned())?;
    if reason.trim().is_empty()
        || reason.len() > MAX_REASON_BYTES
        || reason.chars().any(char::is_control)
    {
        return Err("permission reason is empty or exceeds the host limit".into());
    }
    Ok(Some(reason.to_owned()))
}

fn parse_reason_only(
    value: &toml::Value,
    capability: Capability,
) -> Result<DeclaredPermission, String> {
    if value.is_bool() {
        return Ok(DeclaredPermission {
            capability,
            scope: simple_scope(capability)?,
            reason: None,
        });
    }
    let table = value
        .as_table()
        .ok_or_else(|| "permission must be a table or true".to_owned())?;
    reject_unknown_fields(table, &["reason"])?;
    Ok(DeclaredPermission {
        capability,
        scope: simple_scope(capability)?,
        reason: parse_reason(table.get("reason"))?,
    })
}

fn simple_scope(capability: Capability) -> Result<PermissionScope, String> {
    match capability {
        Capability::Logging => Ok(PermissionScope::Logging),
        Capability::Notification => Ok(PermissionScope::Notification),
        _ => Err("permission does not support a simple scope".into()),
    }
}

fn reject_unknown_fields(
    table: &toml::map::Map<String, toml::Value>,
    allowed: &[&str],
) -> Result<(), String> {
    if let Some(unknown) = table
        .keys()
        .find(|key| !allowed.iter().any(|allowed| allowed == key))
    {
        return Err(format!("unknown permission field: {unknown}"));
    }
    Ok(())
}

fn normalize_https_origin(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() < "https://".len() || !value[.."https://".len()].eq_ignore_ascii_case("https://")
    {
        return Err("HTTP permissions require an HTTPS origin".into());
    }
    let host = &value["https://".len()..];
    if host.is_empty()
        || host.contains(['/', '?', '#', '@', ' '])
        || host.chars().any(char::is_control)
    {
        return Err("HTTP origin must contain only an HTTPS host and optional port".into());
    }
    let (hostname, port) = host
        .split_once(':')
        .map_or((host, None), |(hostname, port)| (hostname, Some(port)));
    if hostname.is_empty()
        || hostname.eq_ignore_ascii_case("localhost")
        || hostname.parse::<std::net::IpAddr>().is_ok()
        || hostname.starts_with('.')
        || hostname.ends_with('.')
    {
        return Err("HTTP origin must use a DNS hostname, not localhost or an IP address".into());
    }
    if !hostname
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
    {
        return Err("HTTP origin hostname contains an invalid character".into());
    }
    if let Some(port) = port {
        let port = port
            .parse::<u16>()
            .map_err(|_| "HTTP origin port is invalid".to_owned())?;
        if port == 0 {
            return Err("HTTP origin port must be positive".into());
        }
        Ok(format!("https://{}:{port}", hostname.to_ascii_lowercase()))
    } else {
        Ok(format!("https://{}", hostname.to_ascii_lowercase()))
    }
}

/// Extracts an HTTPS origin from a runtime URL before applying the same host
/// normalization as manifest parsing. Redirect targets must call the broker
/// again with their own URL, so an allowed first hop never grants a second one.
fn normalize_runtime_https_origin(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() < "https://".len() || !value[.."https://".len()].eq_ignore_ascii_case("https://")
    {
        return Err("runtime HTTP requests require HTTPS".into());
    }
    let authority = &value["https://".len()..];
    let authority = authority.split(['/', '?', '#']).next().unwrap_or_default();
    normalize_https_origin(&format!("https://{authority}"))
}

#[cfg(test)]
mod tests {
    use super::{
        Capability, EffectiveGrant, PermissionChange, PermissionScope, diff_permissions,
        parse_permissions,
    };

    #[test]
    fn structured_permissions_keep_scope_and_reason() {
        let permissions = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["HTTPS://Api.Example.com"], reason = "Translate text" }
            files = { read = "user-selected", reason = "Open a file" }
            clipboard = { access = ["write"], reason = "Copy result" }
        }))
        .expect("structured permissions parse");

        let http = permissions.get(Capability::Http).expect("http permission");
        assert_eq!(
            http.scope,
            PermissionScope::Http {
                origins: ["https://api.example.com".to_owned()].into_iter().collect()
            }
        );
        assert_eq!(http.reason.as_deref(), Some("Translate text"));
        assert_eq!(
            permissions.names().collect::<Vec<_>>(),
            ["clipboard", "files", "http"]
        );
    }

    #[test]
    fn unknown_fields_and_non_https_origins_are_denied() {
        let unknown = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["https://api.example.com"], wildcard = true }
        }));
        assert!(unknown.is_err());

        let insecure = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["http://api.example.com"] }
        }));
        assert!(insecure.is_err());
    }

    #[test]
    fn effective_grant_is_the_intersection_of_three_layers() {
        let declared = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["https://api.example.com", "https://cdn.example.com"] }
            clipboard = { access = ["read", "write"] }
        }))
        .expect("declared");
        let user = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["https://api.example.com"] }
            clipboard = { access = ["read"] }
        }))
        .expect("user grant");
        let host = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["https://api.example.com"] }
            clipboard = { access = ["read", "write"] }
        }))
        .expect("host policy");
        let grant = EffectiveGrant::from_layers(&declared, &user, &host);
        assert!(grant.allows(Capability::Http));
        assert!(grant.allows(Capability::Clipboard));
        assert_eq!(
            grant.permissions().get(Capability::Http).unwrap().scope,
            PermissionScope::Http {
                origins: ["https://api.example.com".to_owned()].into_iter().collect()
            }
        );
    }

    #[test]
    fn user_grant_can_remove_one_capability_without_mutating_the_declaration() {
        let declared = parse_permissions(&toml::Value::Table(toml::toml! {
            clipboard = { access = ["read"] }
            http = { origins = ["https://api.example.test"] }
        }))
        .expect("declared permissions");

        let narrowed = declared.without(Capability::Http);

        assert!(narrowed.contains(Capability::Clipboard));
        assert!(!narrowed.contains(Capability::Http));
        assert!(declared.contains(Capability::Http));
    }

    #[test]
    fn permission_diff_distinguishes_expansion_and_narrowing() {
        let old = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["https://api.example.com"] }
            clipboard = { access = ["read", "write"] }
        }))
        .expect("old");
        let new = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["https://api.example.com", "https://cdn.example.com"] }
            clipboard = { access = ["read"] }
            storage = { quota_bytes = 1_048_576 }
        }))
        .expect("new");
        let diff = diff_permissions(&old, &new);
        assert_eq!(diff[0].capability, Capability::Clipboard);
        assert_eq!(diff[0].change, PermissionChange::Narrowed);
        assert_eq!(diff[1].capability, Capability::Http);
        assert_eq!(diff[1].change, PermissionChange::Expanded);
        assert_eq!(diff[2].change, PermissionChange::Added);
    }

    #[test]
    fn capability_broker_allows_granted_operations() {
        let (identity, grant, handle) = capability_broker_fixture();
        let broker = super::CapabilityBroker;

        assert!(
            broker
                .authorize(
                    &identity,
                    &grant,
                    super::CapabilityRequest::Http {
                        origin: "https://API.example.test/anything".into(),
                    },
                )
                .is_ok()
        );
        assert!(
            broker
                .authorize(&identity, &grant, super::CapabilityRequest::FilesSelect)
                .is_ok()
        );
        assert!(
            broker
                .authorize(
                    &identity,
                    &grant,
                    super::CapabilityRequest::FilesRead { handle },
                )
                .is_ok()
        );
        assert!(
            broker
                .authorize(
                    &identity,
                    &grant,
                    super::CapabilityRequest::StorageWrite { total_bytes: 1024 },
                )
                .is_ok()
        );
        assert!(matches!(
            broker.authorize(
                &identity,
                &grant,
                super::CapabilityRequest::StoragePut {
                    key: "answer".into(),
                    value: b"42".to_vec(),
                }
            ),
            Ok(super::AuthorizedCapability::StoragePut {
                quota_bytes: 1024,
                ..
            })
        ));
        assert!(matches!(
            broker.authorize(
                &identity,
                &grant,
                super::CapabilityRequest::StorageGet {
                    key: "answer".into()
                }
            ),
            Ok(super::AuthorizedCapability::StorageGet { .. })
        ));
    }

    #[test]
    fn capability_broker_rejects_scope_and_identity_mismatches() {
        let (identity, grant, _) = capability_broker_fixture();
        let broker = super::CapabilityBroker;

        assert_eq!(
            broker.authorize(&identity, &grant, super::CapabilityRequest::ClipboardWrite),
            Err(super::CapabilityDenied::ScopeExceeded)
        );
        assert_eq!(
            broker.authorize(
                &identity,
                &grant,
                super::CapabilityRequest::Http {
                    origin: "https://other.example.test".into(),
                },
            ),
            Err(super::CapabilityDenied::ScopeExceeded)
        );
        assert_eq!(
            broker.authorize(
                &identity,
                &grant,
                super::CapabilityRequest::FilesRead {
                    handle: super::HostFileHandle::issue("other", "session-1", "handle-1"),
                },
            ),
            Err(super::CapabilityDenied::HandleMismatch)
        );
        assert_eq!(
            broker.authorize(
                &identity,
                &grant,
                super::CapabilityRequest::StorageWrite { total_bytes: 1025 },
            ),
            Err(super::CapabilityDenied::ScopeExceeded)
        );
    }

    fn capability_broker_fixture() -> (
        super::PluginIdentity,
        super::EffectiveGrant,
        super::HostFileHandle,
    ) {
        let permissions = parse_permissions(&toml::Value::Table(toml::toml! {
            clipboard = { access = ["read"] }
            http = { origins = ["https://api.example.test"] }
            files = { read = "user-selected" }
            storage = { quota_bytes = 1024 }
        }))
        .expect("permissions parse");
        let grant = EffectiveGrant::from_layers(&permissions, &permissions, &permissions);
        let identity = super::PluginIdentity::new("demo", "session-1");
        let handle = super::HostFileHandle::issue("demo", "session-1", "handle-1");
        (identity, grant, handle)
    }

    #[test]
    fn runtime_http_origin_is_rechecked_for_each_redirect_hop() {
        let permissions = parse_permissions(&toml::Value::Table(toml::toml! {
            http = { origins = ["https://api.example.test", "https://cdn.example.test"] }
        }))
        .expect("HTTP permissions parse");
        let grant = EffectiveGrant::from_layers(&permissions, &permissions, &permissions);
        let identity = super::PluginIdentity::new("demo", "redirect-session");
        let broker = super::CapabilityBroker;

        assert!(
            broker
                .authorize(
                    &identity,
                    &grant,
                    super::CapabilityRequest::Http {
                        origin: "https://api.example.test/v1/start".into(),
                    },
                )
                .is_ok()
        );
        assert!(
            broker
                .authorize(
                    &identity,
                    &grant,
                    super::CapabilityRequest::Http {
                        origin: "https://cdn.example.test/v1/final".into(),
                    },
                )
                .is_ok()
        );
        assert_eq!(
            broker.authorize(
                &identity,
                &grant,
                super::CapabilityRequest::Http {
                    origin: "https://evil.example.test/redirect".into(),
                },
            ),
            Err(super::CapabilityDenied::ScopeExceeded)
        );
        assert_eq!(
            broker.authorize(
                &identity,
                &grant,
                super::CapabilityRequest::Http {
                    origin: "http://api.example.test".into(),
                },
            ),
            Err(super::CapabilityDenied::InvalidOrigin)
        );
    }
}
