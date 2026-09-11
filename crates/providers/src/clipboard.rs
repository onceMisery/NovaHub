#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};
use std::fmt::Write;

pub const MAX_HISTORY_ITEMS: usize = 500;
pub const MIN_HISTORY_ITEMS: usize = 1;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const HISTORY_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
pub const MIN_HISTORY_TTL_SECONDS: i64 = 60 * 60;
pub const MAX_HISTORY_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// User-adjustable clipboard retention limits. The bounds keep configuration
/// inside the same memory and privacy budget as the MVP defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardPolicy {
    max_items: usize,
    ttl_seconds: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardPolicyError {
    InvalidItemLimit,
    InvalidTtl,
}

impl std::fmt::Display for ClipboardPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidItemLimit => write!(
                formatter,
                "clipboard item limit must be between {MIN_HISTORY_ITEMS} and {MAX_HISTORY_ITEMS}"
            ),
            Self::InvalidTtl => write!(
                formatter,
                "clipboard retention must be between {MIN_HISTORY_TTL_SECONDS} and {MAX_HISTORY_TTL_SECONDS} seconds"
            ),
        }
    }
}

impl std::error::Error for ClipboardPolicyError {}

impl ClipboardPolicy {
    /// Creates a bounded retention policy.
    ///
    /// # Errors
    ///
    /// Returns an error when either value would exceed the host budget.
    pub fn new(max_items: usize, ttl_seconds: i64) -> Result<Self, ClipboardPolicyError> {
        if !(MIN_HISTORY_ITEMS..=MAX_HISTORY_ITEMS).contains(&max_items) {
            return Err(ClipboardPolicyError::InvalidItemLimit);
        }
        if !(MIN_HISTORY_TTL_SECONDS..=MAX_HISTORY_TTL_SECONDS).contains(&ttl_seconds) {
            return Err(ClipboardPolicyError::InvalidTtl);
        }
        Ok(Self {
            max_items,
            ttl_seconds,
        })
    }

    #[must_use]
    pub const fn max_items(self) -> usize {
        self.max_items
    }

    #[must_use]
    pub const fn ttl_seconds(self) -> i64 {
        self.ttl_seconds
    }
}

impl Default for ClipboardPolicy {
    fn default() -> Self {
        Self {
            max_items: MAX_HISTORY_ITEMS,
            ttl_seconds: HISTORY_TTL_SECONDS,
        }
    }
}

/// Host-side cadence gate for clipboard polling.
///
/// The gate does not read the clipboard or retain payloads. It only decides
/// whether the host event loop may invoke the platform adapter at a given
/// instant, keeping the polling policy independent from OS APIs.
#[derive(Clone, Debug)]
pub struct ClipboardPoller {
    interval: Duration,
    next_poll: Option<Instant>,
    paused: bool,
}

impl Default for ClipboardPoller {
    fn default() -> Self {
        Self::new(DEFAULT_POLL_INTERVAL)
    }
}

impl ClipboardPoller {
    #[must_use]
    pub const fn new(interval: Duration) -> Self {
        Self {
            interval,
            next_poll: None,
            paused: false,
        }
    }

    /// Returns whether a poll is due and schedules the next permitted poll.
    pub fn due(&mut self, now: Instant) -> bool {
        if self.paused {
            return false;
        }
        let due = self.next_poll.is_none_or(|next| now >= next);
        if due {
            self.next_poll = Some(now + self.interval);
        }
        due
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            self.next_poll = None;
        }
    }

    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardKind {
    Text,
    Image,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardItem {
    pub id: u64,
    pub kind: ClipboardKind,
    pub created_at: i64,
    pub size: usize,
    pub pinned: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardError {
    TooLarge { limit: usize },
    Empty,
    Expired,
    Missing,
    CredentialStore(String),
    Encryption,
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { limit } => {
                write!(formatter, "clipboard payload exceeds {limit} bytes")
            }
            Self::Empty => formatter.write_str("clipboard payload must not be empty"),
            Self::Expired => formatter.write_str("clipboard item has expired"),
            Self::Missing => formatter.write_str("clipboard item was not found"),
            Self::CredentialStore(message) => {
                write!(formatter, "credential store error: {message}")
            }
            Self::Encryption => formatter.write_str("clipboard payload encryption failed"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// Host-owned key provider. A production adapter maps this trait to Windows
/// Credential Manager or macOS Keychain; plugins never receive the key.
pub trait CredentialStore {
    /// Returns the 256-bit key used for clipboard payload encryption.
    ///
    /// # Errors
    ///
    /// Returns a credential-store error when the platform key cannot be read.
    fn encryption_key(&self) -> Result<[u8; 32], ClipboardError>;
}

/// Deterministic test/development key provider. It is intentionally explicit
/// so it cannot be mistaken for a platform credential store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryCredentialStore {
    key: [u8; 32],
}

impl MemoryCredentialStore {
    #[must_use]
    pub const fn new(key: [u8; 32]) -> Self {
        Self { key }
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn encryption_key(&self) -> Result<[u8; 32], ClipboardError> {
        Ok(self.key)
    }
}

#[derive(Clone, Debug)]
struct StoredClipboardItem {
    metadata: ClipboardItem,
    expires_at: i64,
    content_digest: [u8; 32],
    nonce: [u8; 24],
    ciphertext: Vec<u8>,
}

/// Bounded, host-owned clipboard history. Newest items are kept at the front;
/// reads promote an item to the front so eviction remains true LRU.
#[derive(Clone, Debug)]
pub struct ClipboardVault<C> {
    credentials: C,
    policy: ClipboardPolicy,
    items: VecDeque<StoredClipboardItem>,
    next_id: u64,
}

impl<C> ClipboardVault<C>
where
    C: CredentialStore,
{
    #[must_use]
    pub fn new(credentials: C) -> Self {
        Self {
            credentials,
            policy: ClipboardPolicy::default(),
            items: VecDeque::new(),
            next_id: 1,
        }
    }

    #[must_use]
    pub const fn policy(&self) -> ClipboardPolicy {
        self.policy
    }

    /// Replaces retention limits and immediately trims existing entries.
    pub fn set_policy(&mut self, policy: ClipboardPolicy) {
        self.policy = policy;
        for item in &mut self.items {
            if !item.metadata.pinned {
                let policy_expiry = item
                    .metadata
                    .created_at
                    .saturating_add(policy.ttl_seconds());
                item.expires_at = item.expires_at.min(policy_expiry);
            }
        }
        self.trim_to_policy();
    }

    /// Captures text after applying the host privacy and size policy.
    /// Sensitive clipboard sources must be filtered before this call.
    ///
    /// # Errors
    ///
    /// Returns a size, empty-payload, credential, or encryption error.
    pub fn capture_text(
        &mut self,
        text: &str,
        created_at: i64,
        sensitive: bool,
    ) -> Result<Option<ClipboardItem>, ClipboardError> {
        if sensitive {
            return Ok(None);
        }
        self.capture(ClipboardKind::Text, text.as_bytes(), created_at)
            .map(Some)
    }

    /// Captures an image payload without decoding it in the host process.
    ///
    /// # Errors
    ///
    /// Returns a size, empty-payload, credential, or encryption error.
    pub fn capture_image(
        &mut self,
        bytes: &[u8],
        created_at: i64,
    ) -> Result<ClipboardItem, ClipboardError> {
        self.capture(ClipboardKind::Image, bytes, created_at)
    }

    /// Returns metadata in newest-first order and removes expired entries.
    pub fn list(&mut self, now: i64) -> Vec<ClipboardItem> {
        self.purge_expired(now);
        self.items
            .iter()
            .map(|item| item.metadata.clone())
            .collect()
    }

    /// Decrypts one item and promotes it to the most-recent position.
    ///
    /// # Errors
    ///
    /// Returns `Missing` or `Expired` when the item is unavailable, or a
    /// credential/encryption error when the protected payload cannot be read.
    pub fn read(&mut self, id: u64, now: i64) -> Result<Vec<u8>, ClipboardError> {
        self.purge_expired(now);
        let index = self
            .items
            .iter()
            .position(|item| item.metadata.id == id)
            .ok_or(ClipboardError::Missing)?;
        let key = self.credentials.encryption_key()?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(&key).map_err(|_| ClipboardError::Encryption)?;
        let plaintext = {
            let item = self.items.get(index).ok_or(ClipboardError::Missing)?;
            cipher
                .decrypt(XNonce::from_slice(&item.nonce), item.ciphertext.as_ref())
                .map_err(|_| ClipboardError::Encryption)?
        };
        let item = self.items.remove(index).ok_or(ClipboardError::Missing)?;
        self.items.push_front(item);
        Ok(plaintext)
    }

    /// Pins or unpins one retained item and returns its content digest so the
    /// host can persist the preference without exposing plaintext.
    ///
    /// # Errors
    ///
    /// Returns `Missing` when the item is no longer retained.
    pub fn set_pinned(&mut self, id: u64, pinned: bool) -> Result<String, ClipboardError> {
        let item = self
            .items
            .iter_mut()
            .find(|item| item.metadata.id == id)
            .ok_or(ClipboardError::Missing)?;
        item.metadata.pinned = pinned;
        item.expires_at = if pinned {
            i64::MAX
        } else {
            item.metadata
                .created_at
                .saturating_add(self.policy.ttl_seconds())
        };
        Ok(hex_digest(&item.content_digest))
    }

    /// Deletes expired entries and returns the number removed.
    pub fn purge_expired(&mut self, now: i64) -> usize {
        let before = self.items.len();
        self.items
            .retain(|item| item.metadata.pinned || item.expires_at > now);
        before - self.items.len()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Removes every in-memory encrypted payload.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    fn capture(
        &mut self,
        kind: ClipboardKind,
        bytes: &[u8],
        created_at: i64,
    ) -> Result<ClipboardItem, ClipboardError> {
        if bytes.is_empty() {
            return Err(ClipboardError::Empty);
        }
        let limit = match kind {
            ClipboardKind::Text => MAX_TEXT_BYTES,
            ClipboardKind::Image => MAX_IMAGE_BYTES,
        };
        if bytes.len() > limit {
            return Err(ClipboardError::TooLarge { limit });
        }
        let content_digest = content_digest(bytes);
        if let Some(item) = self.items.front_mut()
            && item.metadata.kind == kind
            && item.content_digest == content_digest
        {
            item.metadata.created_at = created_at;
            item.expires_at = if item.metadata.pinned {
                i64::MAX
            } else {
                created_at.saturating_add(self.policy.ttl_seconds())
            };
            return Ok(item.metadata.clone());
        }
        let key = self.credentials.encryption_key()?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(&key).map_err(|_| ClipboardError::Encryption)?;
        let mut nonce = [0_u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(XNonce::from_slice(&nonce), bytes)
            .map_err(|_| ClipboardError::Encryption)?;
        let metadata = ClipboardItem {
            id: self.next_id,
            kind,
            created_at,
            size: bytes.len(),
            pinned: false,
        };
        self.next_id = self.next_id.saturating_add(1);
        self.items.push_front(StoredClipboardItem {
            metadata: metadata.clone(),
            expires_at: created_at.saturating_add(self.policy.ttl_seconds()),
            content_digest,
            nonce,
            ciphertext,
        });
        self.trim_to_policy();
        Ok(metadata)
    }

    fn trim_to_policy(&mut self) {
        while self.items.len() > self.policy.max_items() {
            let removable = self
                .items
                .iter()
                .rposition(|item| !item.metadata.pinned)
                .or_else(|| self.items.len().checked_sub(1));
            let Some(index) = removable else {
                break;
            };
            self.items.remove(index);
        }
    }
}

fn content_digest(content: &[u8]) -> [u8; 32] {
    Sha256::digest(content).into()
}

/// Hashes clipboard bytes without retaining their plaintext.
#[must_use]
pub fn content_hash(content: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(content).into();
    hex_digest(&digest)
}

fn hex_digest(digest: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{
        ClipboardError, ClipboardKind, ClipboardPolicy, ClipboardPoller, ClipboardVault,
        CredentialStore, HISTORY_TTL_SECONDS, MAX_HISTORY_ITEMS, MAX_IMAGE_BYTES, MAX_TEXT_BYTES,
        MemoryCredentialStore, content_hash,
    };
    use std::time::{Duration, Instant};

    #[derive(Debug)]
    struct RotatingCredentialStore {
        key: Cell<[u8; 32]>,
    }

    impl CredentialStore for RotatingCredentialStore {
        fn encryption_key(&self) -> Result<[u8; 32], ClipboardError> {
            Ok(self.key.get())
        }
    }

    #[test]
    fn clipboard_hash_is_deterministic_and_does_not_equal_plaintext() {
        let hash = content_hash(b"secret");
        assert_eq!(hash.len(), 64);
        assert!(!hash.contains("secret"));
        assert_eq!(hash, content_hash(b"secret"));
    }

    #[test]
    fn vault_encrypts_text_and_keeps_sensitive_content_out() {
        let mut vault = ClipboardVault::new(MemoryCredentialStore::new([7; 32]));
        assert_eq!(
            vault.capture_text("secret", 10, true).expect("capture"),
            None
        );
        let item = vault
            .capture_text("hello", 10, false)
            .expect("capture")
            .expect("non-sensitive item");
        assert_eq!(item.kind, ClipboardKind::Text);
        assert_eq!(vault.read(item.id, 11).expect("decrypt"), b"hello");
    }

    #[test]
    fn adjacent_duplicate_updates_timestamp_without_growing_history() {
        let mut vault = ClipboardVault::new(MemoryCredentialStore::new([7; 32]));
        let first = vault
            .capture_text("hello", 10, false)
            .expect("capture")
            .expect("item");
        let duplicate = vault
            .capture_text("hello", 20, false)
            .expect("duplicate capture")
            .expect("item");

        assert_eq!(duplicate.id, first.id);
        assert_eq!(vault.len(), 1);
        assert_eq!(vault.list(20)[0].created_at, 20);
    }

    #[test]
    fn pin_state_is_host_owned_and_does_not_change_payload_access() {
        let mut vault = ClipboardVault::new(MemoryCredentialStore::new([9; 32]));
        let item = vault
            .capture_text("pinned", 10, false)
            .expect("capture")
            .expect("item");
        assert!(!item.pinned);
        let digest = vault.set_pinned(item.id, true).expect("pin item");
        assert_eq!(digest, content_hash(b"pinned"));
        assert!(vault.list(10)[0].pinned);
        assert_eq!(vault.read(item.id, 10).expect("read pinned"), b"pinned");
        assert!(vault.set_pinned(999, false).is_err());
    }

    #[test]
    fn pinned_items_are_not_removed_by_ttl_or_unpinned_lru_eviction() {
        let mut vault = ClipboardVault::new(MemoryCredentialStore::new([8; 32]));
        let pinned = vault
            .capture_text("keep", 10, false)
            .expect("capture")
            .expect("item");
        vault.set_pinned(pinned.id, true).expect("pin item");
        vault
            .capture_text("new", 20, false)
            .expect("capture newest");
        vault.set_policy(ClipboardPolicy::new(1, 3_600).expect("policy"));
        let items = vault.list(10 + HISTORY_TTL_SECONDS + 1);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, pinned.id);
        assert!(items[0].pinned);
    }

    #[test]
    fn poller_enforces_interval_and_pause_without_reading_payloads() {
        let start = Instant::now();
        let mut poller = ClipboardPoller::new(Duration::from_secs(1));
        assert!(poller.due(start));
        assert!(!poller.due(start + Duration::from_millis(999)));
        assert!(poller.due(start + Duration::from_secs(1)));

        poller.set_paused(true);
        assert!(poller.is_paused());
        assert!(!poller.due(start + Duration::from_secs(2)));
        poller.set_paused(false);
        assert!(poller.due(start + Duration::from_secs(2)));
    }

    #[test]
    fn failed_decryption_does_not_evict_the_history_item() {
        let mut vault = ClipboardVault::new(RotatingCredentialStore {
            key: Cell::new([7; 32]),
        });
        let item = vault
            .capture_text("hello", 10, false)
            .expect("capture")
            .expect("item");
        vault.credentials.key.set([8; 32]);
        assert_eq!(vault.read(item.id, 11), Err(ClipboardError::Encryption));
        vault.credentials.key.set([7; 32]);
        assert_eq!(vault.read(item.id, 11).expect("retry decrypt"), b"hello");
    }

    #[test]
    fn vault_applies_image_limit_ttl_and_lru_bound() {
        let mut vault = ClipboardVault::new(MemoryCredentialStore::new([3; 32]));
        assert!(
            vault
                .capture_image(&vec![0; MAX_IMAGE_BYTES + 1], 10)
                .is_err()
        );
        assert!(
            vault
                .capture_text(&"x".repeat(MAX_TEXT_BYTES + 1), 10, false)
                .is_err()
        );
        for index in 0..=MAX_HISTORY_ITEMS {
            let timestamp_offset = i64::try_from(index).unwrap_or(i64::MAX);
            vault
                .capture_text(&index.to_string(), 10 + timestamp_offset, false)
                .expect("bounded capture");
        }
        assert_eq!(vault.len(), MAX_HISTORY_ITEMS);
        assert_eq!(
            vault.purge_expired(
                10 + i64::try_from(MAX_HISTORY_ITEMS).unwrap_or(i64::MAX)
                    + super::HISTORY_TTL_SECONDS,
            ),
            MAX_HISTORY_ITEMS
        );
        assert!(vault.is_empty());
    }

    #[test]
    fn policy_bounds_retention_and_trims_existing_items() {
        let mut vault = ClipboardVault::new(MemoryCredentialStore::new([4; 32]));
        for index in 0..3 {
            vault
                .capture_text(&format!("item-{index}"), 10 + index, false)
                .expect("capture");
        }
        let policy = ClipboardPolicy::new(2, 3_600).expect("bounded policy");
        vault.set_policy(policy);

        assert_eq!(vault.policy(), policy);
        assert_eq!(vault.len(), 2);
        assert_eq!(vault.list(10_000).len(), 0);
    }

    #[test]
    fn default_policy_preserves_mvp_retention_budget() {
        let vault = ClipboardVault::new(MemoryCredentialStore::new([5; 32]));
        assert_eq!(vault.policy().max_items(), MAX_HISTORY_ITEMS);
        assert_eq!(vault.policy().ttl_seconds(), HISTORY_TTL_SECONDS);
    }
}
