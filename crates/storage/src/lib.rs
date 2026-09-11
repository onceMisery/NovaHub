#![forbid(unsafe_code)]

use std::path::Path;

use novahub_core_domain::CommandDescriptor;

const MAX_QUICKLINK_ID_BYTES: usize = 128;
const MAX_TITLE_BYTES: usize = 256;
const MAX_TEMPLATE_BYTES: usize = 4096;
const MAX_SNIPPET_BYTES: usize = 64 * 1024;
const MAX_PLUGIN_GRANT_BYTES: usize = 64 * 1024;
const MAX_PLUGIN_KV_KEY_BYTES: usize = 128;
const MAX_PLUGIN_KV_VALUE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Quicklink {
    pub id: String,
    pub title: String,
    pub url_template: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snippet {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// One validated, host-owned record produced by the migration importer.
/// Keeping this DTO in storage avoids making the migration parser a second
/// owner of `SQLite` schema or transaction behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationRecord {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub content: String,
}

pub struct Storage {
    connection: rusqlite::Connection,
}

impl Storage {
    /// Opens a file-backed `SQLite` database and applies the host schema.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when opening or migrating the database fails.
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let connection = rusqlite::Connection::open(path)?;
        let storage = Self { connection };
        storage.migrate()?;
        Ok(storage)
    }

    /// Opens an isolated in-memory database for tests and probes.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when the schema cannot be initialized.
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let connection = rusqlite::Connection::open_in_memory()?;
        let storage = Self { connection };
        storage.migrate()?;
        Ok(storage)
    }

    /// Applies all currently known migrations in one transaction.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors and rolls back the transaction on failure.
    pub fn migrate(&self) -> rusqlite::Result<()> {
        self.connection.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS schema_version (
                 version INTEGER NOT NULL
             );
             INSERT INTO schema_version(version)
                 SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM schema_version);
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS commands (
                 id TEXT PRIMARY KEY NOT NULL,
                 title TEXT NOT NULL,
                 subtitle TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS plugins (
                 id TEXT PRIMARY KEY NOT NULL,
                 version TEXT NOT NULL,
                 active INTEGER NOT NULL,
                 installed_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS plugin_user_grants (
                 plugin_id TEXT PRIMARY KEY NOT NULL,
                 permissions_json TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS plugin_kv (
                 plugin_id TEXT NOT NULL,
                 key TEXT NOT NULL,
                 value BLOB NOT NULL,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY(plugin_id, key)
             );
             CREATE TABLE IF NOT EXISTS clipboard_items (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 content_hash TEXT NOT NULL,
                 kind TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 expires_at INTEGER NOT NULL,
                 pinned INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS clipboard_items_hash_idx
                 ON clipboard_items(content_hash);
             CREATE TABLE IF NOT EXISTS quicklinks (
                 id TEXT PRIMARY KEY NOT NULL,
                 title TEXT NOT NULL,
                 url_template TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS snippets (
                 id TEXT PRIMARY KEY NOT NULL,
                 title TEXT NOT NULL,
                 body TEXT NOT NULL
             );
             UPDATE schema_version SET version = 3 WHERE version < 3;
             COMMIT;",
        )?;
        let has_pinned_column: bool = self.connection.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('clipboard_items') WHERE name = 'pinned'",
            [],
            |row| row.get(0),
        )?;
        if !has_pinned_column {
            self.connection.execute(
                "ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        Ok(())
    }

    /// Returns the schema version recorded by the host.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when the version row cannot be read.
    pub fn schema_version(&self) -> rusqlite::Result<u32> {
        self.connection
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
    }

    /// Stores a host-owned setting value.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when the write fails.
    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// Persists clipboard retention settings and trims metadata atomically.
    ///
    /// The encrypted payloads remain owned by the in-memory Vault; this
    /// transaction only keeps `SQLite` metadata aligned with the same bounds.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors and rolls back every metadata change on failure.
    pub fn set_clipboard_policy(&self, max_items: usize, ttl_seconds: i64) -> rusqlite::Result<()> {
        let max_items = i64::try_from(max_items).unwrap_or(i64::MAX);
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO settings(key, value) VALUES ('clipboard_max_items', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [max_items.to_string()],
        )?;
        transaction.execute(
            "INSERT INTO settings(key, value) VALUES ('clipboard_ttl_seconds', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [ttl_seconds.to_string()],
        )?;
        transaction.execute(
            "UPDATE clipboard_items
             SET expires_at = MIN(expires_at, created_at + ?1)
             WHERE pinned = 0",
            [ttl_seconds],
        )?;
        transaction.execute(
            "DELETE FROM clipboard_items
             WHERE pinned = 0 AND id NOT IN (
                 SELECT id FROM clipboard_items
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?1
             )",
            [max_items],
        )?;
        transaction.commit()
    }

    /// Reads one host-owned setting.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when the query fails.
    pub fn get_setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })
    }

    /// Deletes one host-owned setting.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when the delete fails.
    pub fn delete_setting(&self, key: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Replaces the host-owned static command snapshot atomically.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors and rolls back when any command cannot be written.
    pub fn replace_commands(&self, commands: &[CommandDescriptor]) -> rusqlite::Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute("DELETE FROM commands", [])?;
        for command in commands {
            transaction.execute(
                "INSERT INTO commands(id, title, subtitle) VALUES (?1, ?2, ?3)",
                rusqlite::params![command.id.as_str(), command.title, command.subtitle],
            )?;
        }
        transaction.commit()
    }

    /// Returns the number of commands in the host snapshot.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when the count cannot be read.
    pub fn command_count(&self) -> rusqlite::Result<u32> {
        self.connection
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
    }

    /// Records the host-owned installed plugin pointer after filesystem
    /// activation succeeds.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the metadata cannot be written.
    pub fn record_plugin(
        &self,
        id: &str,
        version: &str,
        installed_at: i64,
    ) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO plugins(id, version, active, installed_at) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(id) DO UPDATE SET version = excluded.version, active = 1, installed_at = excluded.installed_at",
            rusqlite::params![id, version, installed_at],
        )?;
        Ok(())
    }

    /// Removes plugin metadata after the package has been removed.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the metadata cannot be removed.
    pub fn remove_plugin(&self, id: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("DELETE FROM plugins WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Returns the number of host-owned installed plugin records.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the count cannot be read.
    pub fn plugin_count(&self) -> rusqlite::Result<u32> {
        self.connection
            .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
    }

    /// Replaces the normalized user-grant document for one plugin.
    ///
    /// The storage layer treats the JSON as bounded host-owned data. The
    /// plugin manager remains the owner of permission parsing and validation.
    /// Persisting an empty normalized grant is an explicit revocation and must
    /// not be represented by deleting the row.
    ///
    /// # Errors
    ///
    /// Returns an input error for invalid identities, empty or oversized
    /// documents and negative timestamps, or a database error on write.
    pub fn set_plugin_user_grant(
        &self,
        plugin_id: &str,
        permissions_json: &str,
        updated_at: i64,
    ) -> Result<(), StorageInputError> {
        validate_plugin_id(plugin_id)?;
        if permissions_json.is_empty()
            || permissions_json.len() > MAX_PLUGIN_GRANT_BYTES
            || updated_at < 0
        {
            return Err(StorageInputError::InvalidInput("invalid plugin user grant"));
        }
        self.connection
            .execute(
                "INSERT INTO plugin_user_grants(plugin_id, permissions_json, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(plugin_id) DO UPDATE SET
                     permissions_json = excluded.permissions_json,
                     updated_at = excluded.updated_at",
                rusqlite::params![plugin_id, permissions_json, updated_at],
            )
            .map_err(StorageInputError::Database)?;
        Ok(())
    }

    /// Reads the persisted user-grant document for one plugin.
    ///
    /// `None` means the plugin has never received an initial installation
    /// grant. An explicit revocation is stored as a present, empty grant.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid plugin identity or a database
    /// error when the query fails.
    pub fn plugin_user_grant(&self, plugin_id: &str) -> Result<Option<String>, StorageInputError> {
        validate_plugin_id(plugin_id)?;
        self.connection
            .query_row(
                "SELECT permissions_json FROM plugin_user_grants WHERE plugin_id = ?1",
                [plugin_id],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(StorageInputError::Database(other)),
            })
    }

    /// Removes the persisted user-grant row after a plugin is fully removed.
    ///
    /// An empty grant is deliberately kept for revocation and pending-delete
    /// states. This operation is reserved for the terminal uninstall state so
    /// a later installation with the same ID cannot inherit stale approval.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid plugin identity or a database
    /// error when the row cannot be removed.
    pub fn remove_plugin_user_grant(&self, plugin_id: &str) -> Result<(), StorageInputError> {
        validate_plugin_id(plugin_id)?;
        self.connection
            .execute(
                "DELETE FROM plugin_user_grants WHERE plugin_id = ?1",
                [plugin_id],
            )
            .map_err(StorageInputError::Database)?;
        Ok(())
    }

    /// Writes one bounded value in a plugin-owned namespace.
    ///
    /// The quota check and replacement happen in one transaction. Replacing a
    /// key counts only the new value, so callers cannot exceed their effective
    /// storage grant through concurrent read/modify/write logic in Plugin Host.
    ///
    /// # Errors
    ///
    /// Returns an input or quota error for invalid data, or a database error
    /// when the transaction cannot be completed.
    pub fn set_plugin_kv(
        &self,
        plugin_id: &str,
        key: &str,
        value: &[u8],
        quota_bytes: u64,
        updated_at: i64,
    ) -> Result<(), StorageInputError> {
        validate_plugin_kv_input(plugin_id, key, value, quota_bytes, updated_at)?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(StorageInputError::Database)?;
        let current_bytes: u64 = transaction
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(value)), 0)
                 FROM plugin_kv
                 WHERE plugin_id = ?1 AND key <> ?2",
                rusqlite::params![plugin_id, key],
                |row| row.get(0),
            )
            .map_err(StorageInputError::Database)?;
        let value_bytes = u64::try_from(value.len()).unwrap_or(u64::MAX);
        let next_bytes = current_bytes.saturating_add(value_bytes);
        if next_bytes > quota_bytes {
            return Err(StorageInputError::QuotaExceeded);
        }
        transaction
            .execute(
                "INSERT INTO plugin_kv(plugin_id, key, value, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(plugin_id, key) DO UPDATE SET
                     value = excluded.value,
                     updated_at = excluded.updated_at",
                rusqlite::params![plugin_id, key, value, updated_at],
            )
            .map_err(StorageInputError::Database)?;
        transaction.commit().map_err(StorageInputError::Database)
    }

    /// Reads one bounded value from a plugin-owned namespace.
    ///
    /// # Errors
    ///
    /// Returns an input error for invalid identities or keys, or a database
    /// error when the value cannot be read.
    pub fn plugin_kv(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, StorageInputError> {
        validate_plugin_kv_key(plugin_id, key)?;
        self.connection
            .query_row(
                "SELECT value FROM plugin_kv WHERE plugin_id = ?1 AND key = ?2",
                rusqlite::params![plugin_id, key],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(StorageInputError::Database(other)),
            })
    }

    /// Removes all typed KV values after a plugin reaches terminal uninstall.
    ///
    /// # Errors
    ///
    /// Returns an input or database error.
    pub fn remove_plugin_kv_namespace(&self, plugin_id: &str) -> Result<(), StorageInputError> {
        validate_plugin_id(plugin_id)?;
        self.connection
            .execute("DELETE FROM plugin_kv WHERE plugin_id = ?1", [plugin_id])
            .map_err(StorageInputError::Database)?;
        Ok(())
    }

    /// Removes authorization and typed KV state in one terminal-uninstall
    /// transaction.
    ///
    /// # Errors
    ///
    /// Returns an input or database error and rolls back both deletions.
    pub fn remove_plugin_state(&self, plugin_id: &str) -> Result<(), StorageInputError> {
        validate_plugin_id(plugin_id)?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(StorageInputError::Database)?;
        transaction
            .execute("DELETE FROM plugin_kv WHERE plugin_id = ?1", [plugin_id])
            .map_err(StorageInputError::Database)?;
        transaction
            .execute(
                "DELETE FROM plugin_user_grants WHERE plugin_id = ?1",
                [plugin_id],
            )
            .map_err(StorageInputError::Database)?;
        transaction.commit().map_err(StorageInputError::Database)
    }

    /// Records clipboard metadata while never accepting sensitive plaintext.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when metadata cannot be written.
    pub fn record_clipboard(
        &self,
        content_hash: &str,
        kind: &str,
        created_at: i64,
        ttl_seconds: i64,
        sensitive: bool,
    ) -> rusqlite::Result<bool> {
        if sensitive || content_hash.is_empty() || ttl_seconds <= 0 {
            return Ok(false);
        }
        let updated = self.connection.execute(
            "UPDATE clipboard_items
             SET kind = ?2, created_at = ?3, expires_at = ?3 + ?4
             WHERE content_hash = ?1",
            rusqlite::params![content_hash, kind, created_at, ttl_seconds],
        )?;
        if updated > 0 {
            return Ok(true);
        }
        self.connection.execute(
            "INSERT INTO clipboard_items(content_hash, kind, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?3 + ?4)",
            rusqlite::params![content_hash, kind, created_at, ttl_seconds],
        )?;
        Ok(true)
    }

    /// Persists the user's pin state without storing clipboard plaintext.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the metadata row cannot be updated.
    pub fn set_clipboard_pinned(&self, content_hash: &str, pinned: bool) -> rusqlite::Result<bool> {
        let changed = self.connection.execute(
            "UPDATE clipboard_items SET pinned = ?2 WHERE content_hash = ?1",
            rusqlite::params![content_hash, pinned],
        )?;
        Ok(changed > 0)
    }

    /// Deletes expired clipboard metadata and returns the number removed.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when cleanup fails.
    pub fn purge_expired_clipboard(&self, now: i64) -> rusqlite::Result<u32> {
        let removed = self.connection.execute(
            "DELETE FROM clipboard_items WHERE expires_at <= ?1 AND pinned = 0",
            [now],
        )?;
        Ok(u32::try_from(removed).unwrap_or(u32::MAX))
    }

    /// Returns the number of retained clipboard metadata rows.
    ///
    /// # Errors
    ///
    /// Returns `SQLite` errors when the count cannot be read.
    pub fn clipboard_count(&self) -> rusqlite::Result<u32> {
        self.connection
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |row| row.get(0))
    }

    /// Removes all non-sensitive clipboard metadata after the in-memory vault
    /// has been cleared by the host.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the metadata cannot be removed.
    pub fn clear_clipboard(&self) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM clipboard_items", [])?;
        Ok(())
    }

    /// Stores a host-owned Quicklink after validating its bounded URL template.
    /// Only `{query}` is substituted at execution time; scripts and other
    /// template expressions are never evaluated.
    ///
    /// # Errors
    ///
    /// Returns a validation error or `SQLite` error.
    pub fn save_quicklink(
        &self,
        id: &str,
        title: &str,
        url_template: &str,
    ) -> Result<(), StorageInputError> {
        validate_identifier(id)?;
        validate_title(title)?;
        validate_url_template(url_template)?;
        self.connection
            .execute(
                "INSERT INTO quicklinks(id, title, url_template) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET title = excluded.title, url_template = excluded.url_template",
                rusqlite::params![id, title, url_template],
            )
            .map(|_| ())
            .map_err(StorageInputError::Database)
    }

    /// Resolves one Quicklink using a percent-encoded query value.
    ///
    /// # Errors
    ///
    /// Returns `NotFound`, a validation error, or `SQLite` error.
    pub fn resolve_quicklink(&self, id: &str, query: &str) -> Result<String, StorageInputError> {
        let template = self
            .connection
            .query_row(
                "SELECT url_template FROM quicklinks WHERE id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageInputError::NotFound,
                other => StorageInputError::Database(other),
            })?;
        Ok(template.replace("{query}", &percent_encode(query)))
    }

    /// Stores a plain-text Snippet without executing or interpreting it.
    ///
    /// # Errors
    ///
    /// Returns a validation error or `SQLite` error.
    pub fn save_snippet(&self, id: &str, title: &str, body: &str) -> Result<(), StorageInputError> {
        validate_identifier(id)?;
        validate_title(title)?;
        if body.is_empty() || body.len() > MAX_SNIPPET_BYTES {
            return Err(StorageInputError::InvalidInput(
                "snippet body exceeds limit",
            ));
        }
        self.connection
            .execute(
                "INSERT INTO snippets(id, title, body) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET title = excluded.title, body = excluded.body",
                rusqlite::params![id, title, body],
            )
            .map(|_| ())
            .map_err(StorageInputError::Database)
    }

    /// Reads one plain-text Snippet body.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` or `SQLite` error.
    pub fn read_snippet(&self, id: &str) -> Result<String, StorageInputError> {
        self.connection
            .query_row("SELECT body FROM snippets WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageInputError::NotFound,
                other => StorageInputError::Database(other),
            })
    }

    /// Imports validated Quicklinks and Snippets in one transaction.
    ///
    /// Every record is checked before the transaction starts. Any database
    /// error rolls the whole batch back, so a failed import cannot leave a
    /// partially migrated configuration behind.
    ///
    /// # Errors
    ///
    /// Returns a validation error for unsupported or malformed records, or a
    /// database error when the transaction cannot be committed.
    pub fn import_migration_records(
        &self,
        records: &[MigrationRecord],
    ) -> Result<u32, StorageInputError> {
        let mut ids = std::collections::BTreeSet::new();
        for record in records {
            if !ids.insert((record.kind.as_str(), record.id.as_str())) {
                return Err(StorageInputError::InvalidInput(
                    "duplicate migration record id",
                ));
            }
            match record.kind.as_str() {
                "quicklink" => {
                    validate_identifier(&record.id)?;
                    validate_title(&record.title)?;
                    validate_url_template(&record.content)?;
                }
                "snippet" => {
                    validate_identifier(&record.id)?;
                    validate_title(&record.title)?;
                    if record.content.is_empty() || record.content.len() > MAX_SNIPPET_BYTES {
                        return Err(StorageInputError::InvalidInput(
                            "snippet body exceeds limit",
                        ));
                    }
                }
                _ => {
                    return Err(StorageInputError::InvalidInput(
                        "unsupported migration kind",
                    ));
                }
            }
        }

        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(StorageInputError::Database)?;
        for record in records {
            match record.kind.as_str() {
                "quicklink" => {
                    transaction
                        .execute(
                            "INSERT INTO quicklinks(id, title, url_template) VALUES (?1, ?2, ?3)
                             ON CONFLICT(id) DO UPDATE SET title = excluded.title, url_template = excluded.url_template",
                            rusqlite::params![record.id, record.title, record.content],
                        )
                        .map_err(StorageInputError::Database)?;
                }
                "snippet" => {
                    transaction
                        .execute(
                            "INSERT INTO snippets(id, title, body) VALUES (?1, ?2, ?3)
                             ON CONFLICT(id) DO UPDATE SET title = excluded.title, body = excluded.body",
                            rusqlite::params![record.id, record.title, record.content],
                        )
                        .map_err(StorageInputError::Database)?;
                }
                _ => unreachable!("migration records were validated above"),
            }
        }
        transaction.commit().map_err(StorageInputError::Database)?;
        Ok(u32::try_from(records.len()).unwrap_or(u32::MAX))
    }
}

#[derive(Debug)]
pub enum StorageInputError {
    InvalidInput(&'static str),
    NotFound,
    QuotaExceeded,
    Database(rusqlite::Error),
}

impl std::fmt::Display for StorageInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => formatter.write_str(message),
            Self::NotFound => formatter.write_str("stored item was not found"),
            Self::QuotaExceeded => formatter.write_str("plugin storage quota exceeded"),
            Self::Database(error) => write!(formatter, "storage database error: {error}"),
        }
    }
}

impl std::error::Error for StorageInputError {}

fn validate_identifier(value: &str) -> Result<(), StorageInputError> {
    if value.trim().is_empty()
        || value.len() > MAX_QUICKLINK_ID_BYTES
        || value.chars().any(char::is_control)
        || value.contains(['/', '\\'])
    {
        Err(StorageInputError::InvalidInput("invalid stored item id"))
    } else {
        Ok(())
    }
}

fn validate_plugin_id(value: &str) -> Result<(), StorageInputError> {
    if value.trim().is_empty()
        || value.len() > MAX_QUICKLINK_ID_BYTES
        || value.chars().any(char::is_control)
        || value.contains(['/', '\\'])
    {
        Err(StorageInputError::InvalidInput("invalid plugin id"))
    } else {
        Ok(())
    }
}

fn validate_plugin_kv_key(plugin_id: &str, key: &str) -> Result<(), StorageInputError> {
    validate_plugin_id(plugin_id)?;
    if key.trim().is_empty()
        || key.len() > MAX_PLUGIN_KV_KEY_BYTES
        || key.chars().any(char::is_control)
    {
        return Err(StorageInputError::InvalidInput(
            "invalid plugin storage key",
        ));
    }
    Ok(())
}

fn validate_plugin_kv_input(
    plugin_id: &str,
    key: &str,
    value: &[u8],
    quota_bytes: u64,
    updated_at: i64,
) -> Result<(), StorageInputError> {
    validate_plugin_kv_key(plugin_id, key)?;
    if value.len() > MAX_PLUGIN_KV_VALUE_BYTES || quota_bytes == 0 || updated_at < 0 {
        return Err(StorageInputError::InvalidInput(
            "invalid plugin storage value",
        ));
    }
    Ok(())
}

fn validate_title(value: &str) -> Result<(), StorageInputError> {
    if value.trim().is_empty()
        || value.len() > MAX_TITLE_BYTES
        || value.chars().any(char::is_control)
    {
        Err(StorageInputError::InvalidInput("invalid stored item title"))
    } else {
        Ok(())
    }
}

fn validate_url_template(value: &str) -> Result<(), StorageInputError> {
    if value.len() > MAX_TEMPLATE_BYTES
        || value.chars().any(char::is_control)
        || !(value.starts_with("https://") || value.starts_with("http://"))
        || value
            .split('{')
            .skip(1)
            .any(|part| !part.starts_with("query}"))
    {
        return Err(StorageInputError::InvalidInput(
            "invalid Quicklink URL template",
        ));
    }
    Ok(())
}

fn percent_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut output, "%{byte:02X}").expect("String formatting cannot fail");
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{MigrationRecord, Storage};

    #[test]
    fn settings_roundtrip_uses_host_owned_sqlite_schema() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage.set_setting("theme", "dark").expect("write setting");
        assert_eq!(
            storage
                .get_setting("theme")
                .expect("read setting")
                .as_deref(),
            Some("dark")
        );
        storage.delete_setting("theme").expect("delete setting");
        assert_eq!(storage.get_setting("theme").expect("read deleted"), None);
    }

    #[test]
    fn clipboard_policy_updates_settings_and_trims_metadata_atomically() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        for index in 0..3 {
            assert!(
                storage
                    .record_clipboard(&format!("hash-{index}"), "text", index, 604_800, false)
                    .expect("record metadata")
            );
        }
        storage
            .set_clipboard_policy(2, 3_600)
            .expect("apply clipboard policy");

        assert_eq!(storage.clipboard_count().expect("count metadata"), 2);
        assert_eq!(
            storage
                .get_setting("clipboard_max_items")
                .expect("read item limit")
                .as_deref(),
            Some("2")
        );
        assert_eq!(
            storage
                .get_setting("clipboard_ttl_seconds")
                .expect("read ttl")
                .as_deref(),
            Some("3600")
        );
    }

    #[test]
    fn migration_is_idempotent_and_records_schema_version() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage.migrate().expect("first migration");
        storage.migrate().expect("second migration");
        assert_eq!(storage.schema_version().expect("schema version"), 3);
        let grant_table_exists: bool = storage
            .connection
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'plugin_user_grants'",
                [],
                |row| row.get(0),
            )
            .expect("inspect plugin grant schema");
        assert!(grant_table_exists);
        let kv_table_exists: bool = storage
            .connection
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'plugin_kv'",
                [],
                |row| row.get(0),
            )
            .expect("inspect plugin KV schema");
        assert!(kv_table_exists);
    }

    #[test]
    fn command_snapshot_replaces_previous_rows_atomically() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        let command = novahub_core_domain::CommandDescriptor {
            id: novahub_core_domain::CommandId::new("demo"),
            title: "Demo".into(),
            subtitle: "Demo command".into(),
        };
        storage
            .replace_commands(std::slice::from_ref(&command))
            .expect("write command snapshot");
        assert_eq!(storage.command_count().expect("count commands"), 1);
        storage.replace_commands(&[]).expect("clear snapshot");
        assert_eq!(storage.command_count().expect("count cleared"), 0);
    }

    #[test]
    fn clipboard_metadata_rejects_sensitive_rows_and_purges_expired_rows() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        assert!(
            !storage
                .record_clipboard("hash", "text", 10, 10, true)
                .expect("sensitive decision")
        );
        assert!(
            storage
                .record_clipboard("hash", "text", 10, 10, false)
                .expect("metadata write")
        );
        assert!(
            storage
                .record_clipboard("hash", "text", 20, 10, false)
                .expect("duplicate metadata update")
        );
        assert_eq!(storage.clipboard_count().expect("count"), 1);
        assert_eq!(storage.purge_expired_clipboard(20).expect("not expired"), 0);
        assert_eq!(storage.purge_expired_clipboard(31).expect("purge"), 1);
        assert_eq!(storage.clipboard_count().expect("count after purge"), 0);
    }

    #[test]
    fn clipboard_pin_state_updates_without_storing_plaintext() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .record_clipboard("hash", "text", 10, 100, false)
            .expect("record metadata");
        assert!(
            storage
                .set_clipboard_pinned("hash", true)
                .expect("pin metadata")
        );
        assert!(
            !storage
                .set_clipboard_pinned("missing", true)
                .expect("missing metadata remains valid")
        );
    }

    #[test]
    fn pinned_clipboard_metadata_survives_expiry_cleanup() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .record_clipboard("hash", "text", 10, 10, false)
            .expect("record metadata");
        storage
            .set_clipboard_pinned("hash", true)
            .expect("pin metadata");

        assert_eq!(
            storage
                .purge_expired_clipboard(20)
                .expect("purge expired metadata"),
            0
        );
        assert_eq!(storage.clipboard_count().expect("count metadata"), 1);
    }

    #[test]
    fn migration_adds_pinned_column_to_legacy_clipboard_schema() {
        let connection = rusqlite::Connection::open_in_memory().expect("legacy database");
        connection
            .execute_batch(
                "CREATE TABLE schema_version (version INTEGER NOT NULL);
                 INSERT INTO schema_version(version) VALUES (1);
                 CREATE TABLE clipboard_items (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     content_hash TEXT NOT NULL,
                     kind TEXT NOT NULL,
                     created_at INTEGER NOT NULL,
                     expires_at INTEGER NOT NULL
                 );",
            )
            .expect("create legacy schema");
        let storage = Storage { connection };

        storage.migrate().expect("migrate legacy schema");
        let has_pinned_column: bool = storage
            .connection
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('clipboard_items') WHERE name = 'pinned'",
                [],
                |row| row.get(0),
            )
            .expect("inspect migrated schema");
        assert!(has_pinned_column);
    }

    #[test]
    fn plugin_metadata_tracks_filesystem_activation_and_removal() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .record_plugin("demo", "1.0.0", 10)
            .expect("record plugin");
        assert_eq!(storage.plugin_count().expect("plugin count"), 1);
        storage.remove_plugin("demo").expect("remove plugin");
        assert_eq!(
            storage.plugin_count().expect("plugin count after removal"),
            0
        );
    }

    #[test]
    fn plugin_user_grant_roundtrips_and_can_be_replaced_with_revocation() {
        let path = std::env::temp_dir().join(format!(
            "novahub-plugin-user-grant-{}.sqlite3",
            std::process::id()
        ));
        {
            let storage = Storage::open(&path).expect("open grant database");
            storage
                .set_plugin_user_grant("demo", r#"{"entries":{"storage":{}}}"#, 10)
                .expect("store initial grant");
            assert_eq!(
                storage
                    .plugin_user_grant("demo")
                    .expect("read initial grant")
                    .as_deref(),
                Some(r#"{"entries":{"storage":{}}}"#)
            );
        }
        {
            let storage = Storage::open(&path).expect("reopen grant database");
            assert_eq!(
                storage
                    .plugin_user_grant("demo")
                    .expect("restore grant")
                    .as_deref(),
                Some(r#"{"entries":{"storage":{}}}"#)
            );
            storage
                .set_plugin_user_grant("demo", r#"{"entries":{}}"#, 20)
                .expect("persist revocation");
            assert_eq!(
                storage
                    .plugin_user_grant("demo")
                    .expect("read revoked grant")
                    .as_deref(),
                Some(r#"{"entries":{}}"#)
            );
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn plugin_user_grant_rejects_invalid_identity_and_unbounded_payload() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        assert!(storage.set_plugin_user_grant("", "{}", 0).is_err());
        assert!(storage.set_plugin_user_grant("demo", "", 0).is_err());
        assert!(
            storage
                .set_plugin_user_grant("demo", &"x".repeat(64 * 1024 + 1), 0)
                .is_err()
        );
        assert!(storage.set_plugin_user_grant("demo", "{}", -1).is_err());
    }

    #[test]
    fn plugin_user_grant_can_be_removed_only_after_terminal_uninstall() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .set_plugin_user_grant("demo", r#"{"entries":{}}"#, 10)
            .expect("persist explicit empty grant");
        assert!(
            storage
                .plugin_user_grant("demo")
                .expect("read grant")
                .is_some()
        );

        storage
            .remove_plugin_user_grant("demo")
            .expect("remove terminal grant row");
        assert!(
            storage
                .plugin_user_grant("demo")
                .expect("read removed grant")
                .is_none()
        );
        assert!(storage.remove_plugin_user_grant("").is_err());
    }

    #[test]
    fn plugin_kv_is_namespaced_replaced_and_quota_checked_atomically() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .set_plugin_kv("demo", "answer", b"42", 4, 1)
            .expect("write first value");
        storage
            .set_plugin_kv("other", "answer", b"43", 4, 1)
            .expect("write other namespace");
        assert_eq!(
            storage.plugin_kv("demo", "answer").expect("read value"),
            Some(b"42".to_vec())
        );
        assert_eq!(
            storage
                .plugin_kv("other", "answer")
                .expect("read other value"),
            Some(b"43".to_vec())
        );

        storage
            .set_plugin_kv("demo", "answer", b"four", 4, 2)
            .expect("replace within quota");
        assert!(matches!(
            storage.set_plugin_kv("demo", "second", b"x", 4, 3),
            Err(super::StorageInputError::QuotaExceeded)
        ));
        assert!(
            storage
                .plugin_kv("demo", "second")
                .expect("read rejected value")
                .is_none()
        );
    }

    #[test]
    fn terminal_uninstall_removes_the_plugin_kv_namespace() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .set_plugin_kv("demo", "key", b"value", 16, 1)
            .expect("write plugin value");
        storage
            .remove_plugin_kv_namespace("demo")
            .expect("remove plugin namespace");
        assert!(
            storage
                .plugin_kv("demo", "key")
                .expect("read removed value")
                .is_none()
        );
    }

    #[test]
    fn quicklinks_percent_encode_query_and_reject_script_templates() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .save_quicklink("docs", "Docs", "https://example.test/?q={query}")
            .expect("save quicklink");
        assert_eq!(
            storage
                .resolve_quicklink("docs", "Rust & Slint")
                .expect("resolve quicklink"),
            "https://example.test/?q=Rust%20%26%20Slint"
        );
        assert!(
            storage
                .save_quicklink("bad", "Bad", "javascript:alert({query})")
                .is_err()
        );
    }

    #[test]
    fn snippets_are_bounded_plain_text_rows() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        storage
            .save_snippet("hello", "Hello", "No code is executed")
            .expect("save snippet");
        assert_eq!(
            storage.read_snippet("hello").expect("read snippet"),
            "No code is executed"
        );
        assert!(storage.save_snippet("empty", "Empty", "").is_err());
    }

    #[test]
    fn migration_records_commit_as_one_batch_and_reject_duplicates_before_writing() {
        let storage = Storage::open_in_memory().expect("in-memory database");
        let records = vec![
            MigrationRecord {
                kind: "quicklink".into(),
                id: "docs".into(),
                title: "Docs".into(),
                content: "https://example.test/?q={query}".into(),
            },
            MigrationRecord {
                kind: "snippet".into(),
                id: "hello".into(),
                title: "Hello".into(),
                content: "Plain text".into(),
            },
        ];
        assert_eq!(
            storage
                .import_migration_records(&records)
                .expect("import batch"),
            2
        );
        assert_eq!(
            storage
                .resolve_quicklink("docs", "hello world")
                .expect("resolve imported quicklink"),
            "https://example.test/?q=hello%20world"
        );
        assert_eq!(
            storage
                .read_snippet("hello")
                .expect("read imported snippet"),
            "Plain text"
        );

        let duplicate = vec![
            MigrationRecord {
                kind: "snippet".into(),
                id: "same".into(),
                title: "One".into(),
                content: "one".into(),
            },
            MigrationRecord {
                kind: "snippet".into(),
                id: "same".into(),
                title: "Two".into(),
                content: "two".into(),
            },
        ];
        assert!(storage.import_migration_records(&duplicate).is_err());
        assert!(storage.read_snippet("same").is_err());
    }
}
