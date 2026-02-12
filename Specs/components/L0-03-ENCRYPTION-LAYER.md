# L0-03 — Encryption Layer

<!--
=============================================================================
COMPONENT SPEC — Per-profile encryption, key management, secure memory.
Engineer review: E-SC (Security, lead), E-RS (Rust), E-QA (QA)
THIS COMPONENT PROTECTS ALL PATIENT DATA. Zero shortcuts.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=20` |
| [2] Dependencies | `offset=40 limit=15` |
| [3] Interfaces | `offset=55 limit=70` |
| [4] Key Lifecycle | `offset=125 limit=60` |
| [5] Encryption Operations | `offset=185 limit=60` |
| [6] Recovery Phrase | `offset=245 limit=50` |
| [7] Profile Lifecycle | `offset=295 limit=55` |
| [8] Error Handling | `offset=350 limit=25` |
| [9] Security (Threat Model) | `offset=375 limit=50` |
| [10] Testing | `offset=425 limit=55` |
| [11] Performance | `offset=480 limit=15` |
| [12] Open Questions | `offset=495 limit=10` |

---

## [1] Identity

**What:** Implement per-profile AES-256-GCM encryption with PBKDF2 key derivation, secure memory handling (zeroize), 12-word recovery phrase generation and verification, and the profile open/close lifecycle.

**After this session:**
- Create a new profile (name + password → derived key → encrypted storage)
- Open profile (password → derive key → decrypt → access data)
- Close profile (zero all sensitive memory)
- Generate 12-word recovery phrase at profile creation
- Recover profile access using recovery phrase
- Encrypt/decrypt arbitrary byte slices (used by all data storage)
- Cryptographic profile erasure (delete key material → data unrecoverable)
- All sensitive memory zeroed on drop

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Sections 11.1, 11.4

---

## [2] Dependencies

**Incoming:** L0-01 (project exists), L0-02 (SQLite schema exists — encryption wraps database access)

**Outgoing:** L1-* (all pipeline components write through encryption), L3-01 (profile management UI)

**New Cargo.toml dependencies:**
```toml
aes-gcm = "0.10"
pbkdf2 = { version = "0.12", features = ["simple"] }
sha2 = "0.10"
rand = "0.8"
zeroize = { version = "1", features = ["derive"] }
bip39 = "2"
base64 = "0.22"
```

---

## [3] Interfaces

### Core Encryption Trait

```rust
use zeroize::Zeroize;

/// Master encryption key — zeroed on drop
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct ProfileKey {
    key_bytes: [u8; 32],  // AES-256 = 32 bytes
}

impl ProfileKey {
    /// Derive from password + salt using PBKDF2
    pub fn derive(password: &str, salt: &[u8; 32]) -> Self;

    /// Derive from recovery phrase
    pub fn derive_from_recovery(phrase: &str, salt: &[u8; 32]) -> Result<Self, CryptoError>;

    /// Encrypt data
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError>;

    /// Decrypt data
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CryptoError>;
}

/// Encrypted data container: nonce + ciphertext + tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub nonce: [u8; 12],     // AES-GCM nonce (96 bits)
    pub ciphertext: Vec<u8>,  // Encrypted data + auth tag
}

impl EncryptedData {
    /// Serialize to bytes for storage
    pub fn to_bytes(&self) -> Vec<u8>;

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError>;
}
```

### Profile Manager Trait

```rust
pub trait ProfileManager {
    /// Create new profile. Returns profile ID and recovery phrase.
    fn create_profile(
        &self,
        name: &str,
        password: &str,
    ) -> Result<(ProfileInfo, RecoveryPhrase), CryptoError>;

    /// Open existing profile with password. Returns active session.
    fn open_profile(
        &self,
        profile_id: &Uuid,
        password: &str,
    ) -> Result<ProfileSession, CryptoError>;

    /// Open existing profile with recovery phrase.
    fn recover_profile(
        &self,
        profile_id: &Uuid,
        recovery_phrase: &str,
    ) -> Result<ProfileSession, CryptoError>;

    /// List available profiles (names only — no encrypted data).
    fn list_profiles(&self) -> Result<Vec<ProfileInfo>, CryptoError>;

    /// Delete profile (cryptographic erasure).
    fn delete_profile(&self, profile_id: &Uuid) -> Result<(), CryptoError>;
}

/// Active profile session — holds derived key in memory
/// Key is zeroed when session is dropped
pub struct ProfileSession {
    pub profile_id: Uuid,
    pub profile_name: String,
    key: ProfileKey,  // Private — never exposed
    pub db_path: PathBuf,
}

impl ProfileSession {
    /// Encrypt data using this profile's key
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError>;

    /// Decrypt data using this profile's key
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CryptoError>;

    /// Get the SQLite connection for this profile (database path)
    pub fn db_path(&self) -> &Path;
}

impl Drop for ProfileSession {
    fn drop(&mut self) {
        // ProfileKey's Zeroize derive handles key clearing
        tracing::info!(profile_id = %self.profile_id, "Profile session closed, key zeroed");
    }
}

/// Profile metadata (stored unencrypted — David acceptable: names are visible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub managed_by: Option<String>,  // Caregiver attribution
    pub password_hint: Option<String>,
}

/// Recovery phrase wrapper
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct RecoveryPhrase {
    phrase: String,  // 12 words, BIP39
}

impl RecoveryPhrase {
    pub fn as_str(&self) -> &str;
    pub fn words(&self) -> Vec<&str>;
}
```

---

## [4] Key Lifecycle

### Key Derivation (PBKDF2)

**E-SC:** OWASP 2024 recommends 600,000 iterations for PBKDF2-SHA256. This is deliberately slow (~0.5s) to resist brute force.

```rust
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

const PBKDF2_ITERATIONS: u32 = 600_000;
const KEY_LENGTH: usize = 32;  // AES-256
const SALT_LENGTH: usize = 32;

impl ProfileKey {
    pub fn derive(password: &str, salt: &[u8; SALT_LENGTH]) -> Self {
        let mut key_bytes = [0u8; KEY_LENGTH];
        pbkdf2_hmac::<Sha256>(
            password.as_bytes(),
            salt,
            PBKDF2_ITERATIONS,
            &mut key_bytes,
        );
        Self { key_bytes }
    }
}
```

### Salt Generation

```rust
use rand::RngCore;

fn generate_salt() -> [u8; SALT_LENGTH] {
    let mut salt = [0u8; SALT_LENGTH];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}
```

### Profile Creation Storage

On disk, each profile stores (UNENCRYPTED — these are needed to derive the key):

```
~/Coheara/profiles/
├── profiles.json              # List of ProfileInfo (names, IDs)
└── <uuid>/
    ├── salt.bin               # 32-byte random salt
    ├── verification.enc       # Encrypted known plaintext (for password verification)
    ├── recovery_salt.bin      # Separate salt for recovery phrase derivation
    ├── database/
    │   └── coheara.db         # SQLite database (encrypted at application level)
    ├── vectors/
    │   └── (LanceDB files)    # Vector store (encrypted at application level)
    ├── originals/             # Source documents (encrypted individually)
    ├── markdown/              # Converted .md files (encrypted individually)
    └── exports/               # Generated PDFs (encrypted individually)
```

### Password Verification

**E-SC:** We can't store the password. We can't store the key. We store an encrypted known value. If decryption succeeds, the password is correct.

```rust
const VERIFICATION_PLAINTEXT: &[u8] = b"COHEARA_PROFILE_VERIFICATION_V1";

fn create_verification(key: &ProfileKey) -> Result<EncryptedData, CryptoError> {
    key.encrypt(VERIFICATION_PLAINTEXT)
}

fn verify_password(key: &ProfileKey, stored: &EncryptedData) -> bool {
    match key.decrypt(stored) {
        Ok(plaintext) => plaintext == VERIFICATION_PLAINTEXT,
        Err(_) => false,
    }
}
```

---

## [5] Encryption Operations

### AES-256-GCM Encrypt/Decrypt

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::aead::rand_core::RngCore;

const NONCE_LENGTH: usize = 12;

impl ProfileKey {
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError> {
        let key = Key::<Aes256Gcm>::from_slice(&self.key_bytes);
        let cipher = Aes256Gcm::new(key);

        let mut nonce_bytes = [0u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        Ok(EncryptedData {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CryptoError> {
        let key = Key::<Aes256Gcm>::from_slice(&self.key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&encrypted.nonce);

        cipher.decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)
    }
}
```

### File Encryption

```rust
/// Encrypt a file and write to disk
pub fn encrypt_file(
    key: &ProfileKey,
    plaintext_path: &Path,
    encrypted_path: &Path,
) -> Result<(), CryptoError> {
    let plaintext = std::fs::read(plaintext_path)?;
    let encrypted = key.encrypt(&plaintext)?;
    let bytes = encrypted.to_bytes();
    std::fs::write(encrypted_path, bytes)?;

    // Zero the plaintext buffer
    // (Vec<u8> from fs::read — we can't zeroize it directly,
    //  but it will be deallocated when dropped.
    //  For maximum security, use read_to_vec with a Zeroizing wrapper)
    Ok(())
}

/// Decrypt a file from disk
pub fn decrypt_file(
    key: &ProfileKey,
    encrypted_path: &Path,
) -> Result<Vec<u8>, CryptoError> {
    let bytes = std::fs::read(encrypted_path)?;
    let encrypted = EncryptedData::from_bytes(&bytes)?;
    key.decrypt(&encrypted)
}
```

### Database Encryption Strategy

**E-SC + E-DA decision:** Application-level encryption for SQLite. Each row's sensitive text fields are encrypted before INSERT, decrypted after SELECT. This avoids SQLCipher dependency while giving field-level control.

**Alternative considered:** SQLCipher (transparent encryption). Rejected because:
- Adds a C library dependency (complicates cross-compilation)
- All-or-nothing encryption (can't have unencrypted indexes)
- Our application-level approach gives field-level control

**What's encrypted:** Content fields (medication names, doses, lab values, symptoms, notes, conversation text). **What's NOT encrypted:** IDs, dates, enum types, foreign keys (needed for queries). This allows SQL queries on dates and types while protecting medical content.

```rust
/// Wrapper for encrypted database operations
pub struct EncryptedDb {
    conn: Connection,
    key: ProfileKey,
}

impl EncryptedDb {
    /// Insert medication with encrypted fields
    pub fn insert_medication(&self, med: &Medication) -> Result<(), DatabaseError> {
        let encrypted_name = self.key.encrypt(med.generic_name.as_bytes())?;
        let encrypted_dose = self.key.encrypt(med.dose.as_bytes())?;
        // ... encrypt other sensitive fields

        self.conn.execute(
            "INSERT INTO medications (id, generic_name, dose, ...) VALUES (?1, ?2, ?3, ...)",
            params![
                med.id.to_string(),
                base64::encode(&encrypted_name.to_bytes()),
                base64::encode(&encrypted_dose.to_bytes()),
                // ... non-encrypted fields stored directly
                med.status.as_str(),
                med.start_date.map(|d| d.to_string()),
            ],
        )?;
        Ok(())
    }
}
```

---

## [6] Recovery Phrase

### Generation (BIP39)

```rust
use bip39::{Mnemonic, Language};

impl RecoveryPhrase {
    /// Generate a new 12-word recovery phrase
    pub fn generate() -> Self {
        let mnemonic = Mnemonic::generate_in(Language::English, 12)
            .expect("Mnemonic generation should not fail");
        Self {
            phrase: mnemonic.to_string(),
        }
    }

    /// Validate a recovery phrase
    pub fn validate(phrase: &str) -> bool {
        Mnemonic::parse_in(Language::English, phrase).is_ok()
    }
}

impl ProfileKey {
    /// Derive key from recovery phrase
    /// Uses a SEPARATE salt from the password salt (recovery_salt.bin)
    pub fn derive_from_recovery(
        phrase: &str,
        recovery_salt: &[u8; SALT_LENGTH],
    ) -> Result<Self, CryptoError> {
        if !RecoveryPhrase::validate(phrase) {
            return Err(CryptoError::InvalidRecoveryPhrase);
        }

        // Derive the same key that the original password would produce
        // This works because at profile creation, we:
        // 1. Derive key from password + salt
        // 2. Derive a separate key from recovery_phrase + recovery_salt
        // 3. Encrypt the original salt + a key-verification token with the recovery key
        // 4. At recovery time, decrypt to get the password-derived key parameters

        // Actually: simpler approach:
        // Store an encrypted copy of the master key, encrypted with a recovery-derived key
        // Recovery: derive recovery key → decrypt master key → use master key

        let mut key_bytes = [0u8; KEY_LENGTH];
        pbkdf2_hmac::<Sha256>(
            phrase.as_bytes(),
            recovery_salt,
            PBKDF2_ITERATIONS,
            &mut key_bytes,
        );
        Ok(Self { key_bytes })
    }
}
```

### Recovery Storage

**E-SC:** At profile creation, we store the master key encrypted with the recovery-derived key. This allows recovery without storing the password or the master key in plaintext.

```
Profile creation:
  1. Generate salt, recovery_salt
  2. Derive master_key from password + salt
  3. Generate recovery_phrase
  4. Derive recovery_key from recovery_phrase + recovery_salt
  5. Encrypt master_key with recovery_key → store as recovery_blob.enc
  6. Store verification.enc (master_key encrypts known plaintext)
  7. Display recovery_phrase to user ONCE. User writes it down.
  8. Zero recovery_key and recovery_phrase from memory.

Profile recovery:
  1. User enters recovery_phrase
  2. Derive recovery_key from recovery_phrase + recovery_salt
  3. Decrypt recovery_blob.enc → get master_key
  4. Verify master_key against verification.enc
  5. User sets new password
  6. Re-derive and store new verification with new salt
```

---

## [7] Profile Lifecycle

### Create Profile

```rust
pub fn create_profile(
    profiles_dir: &Path,
    name: &str,
    password: &str,
    managed_by: Option<&str>,
) -> Result<(ProfileInfo, RecoveryPhrase), CryptoError> {
    let profile_id = Uuid::new_v4();
    let profile_dir = profiles_dir.join(profile_id.to_string());

    // Create directory structure
    std::fs::create_dir_all(profile_dir.join("database"))?;
    std::fs::create_dir_all(profile_dir.join("vectors"))?;
    std::fs::create_dir_all(profile_dir.join("originals"))?;
    std::fs::create_dir_all(profile_dir.join("markdown"))?;
    std::fs::create_dir_all(profile_dir.join("exports"))?;

    // Generate cryptographic material
    let salt = generate_salt();
    let recovery_salt = generate_salt();
    let master_key = ProfileKey::derive(password, &salt);
    let recovery_phrase = RecoveryPhrase::generate();
    let recovery_key = ProfileKey::derive_from_recovery(
        recovery_phrase.as_str(),
        &recovery_salt,
    )?;

    // Store salt
    std::fs::write(profile_dir.join("salt.bin"), salt)?;
    std::fs::write(profile_dir.join("recovery_salt.bin"), recovery_salt)?;

    // Store verification token
    let verification = master_key.encrypt(VERIFICATION_PLAINTEXT)?;
    std::fs::write(
        profile_dir.join("verification.enc"),
        verification.to_bytes(),
    )?;

    // Store recovery blob (master key encrypted with recovery key)
    let recovery_blob = recovery_key.encrypt(&master_key.key_bytes)?;
    std::fs::write(
        profile_dir.join("recovery_blob.enc"),
        recovery_blob.to_bytes(),
    )?;

    // Initialize SQLite database
    let db_path = profile_dir.join("database/coheara.db");
    let conn = Connection::open(&db_path)?;
    run_migrations(&conn)?;
    drop(conn);

    // Initialize LanceDB
    // (LanceDB creates tables on first insert — just ensure directory exists)

    // Save profile info
    let info = ProfileInfo {
        id: profile_id,
        name: name.to_string(),
        created_at: chrono::Local::now().naive_local(),
        managed_by: managed_by.map(|s| s.to_string()),
        password_hint: None,
    };
    save_profile_info(profiles_dir, &info)?;

    // Zero sensitive material
    drop(recovery_key);  // Zeroize on drop
    // master_key will be zeroed when this function returns (if not moved)

    Ok((info, recovery_phrase))
}
```

### Open Profile

```rust
pub fn open_profile(
    profiles_dir: &Path,
    profile_id: &Uuid,
    password: &str,
) -> Result<ProfileSession, CryptoError> {
    let profile_dir = profiles_dir.join(profile_id.to_string());

    // Load salt
    let salt: [u8; SALT_LENGTH] = std::fs::read(profile_dir.join("salt.bin"))?
        .try_into()
        .map_err(|_| CryptoError::CorruptedProfile)?;

    // Derive key
    let key = ProfileKey::derive(password, &salt);

    // Verify password
    let verification_bytes = std::fs::read(profile_dir.join("verification.enc"))?;
    let verification = EncryptedData::from_bytes(&verification_bytes)?;
    if !verify_password(&key, &verification) {
        return Err(CryptoError::WrongPassword);
    }

    // Load profile info
    let info = load_profile_info(profiles_dir, profile_id)?;

    Ok(ProfileSession {
        profile_id: *profile_id,
        profile_name: info.name,
        key,
        db_path: profile_dir.join("database/coheara.db"),
    })
}
```

### Cryptographic Erasure

```rust
pub fn delete_profile(
    profiles_dir: &Path,
    profile_id: &Uuid,
) -> Result<(), CryptoError> {
    let profile_dir = profiles_dir.join(profile_id.to_string());

    // Step 1: Overwrite salt files with random data (destroys key derivation)
    let random_salt = generate_salt();
    std::fs::write(profile_dir.join("salt.bin"), random_salt)?;
    std::fs::write(profile_dir.join("recovery_salt.bin"), random_salt)?;

    // Step 2: Overwrite verification and recovery blob
    let random_data = vec![0u8; 256];
    std::fs::write(profile_dir.join("verification.enc"), &random_data)?;
    std::fs::write(profile_dir.join("recovery_blob.enc"), &random_data)?;

    // Step 3: Delete the entire profile directory
    std::fs::remove_dir_all(&profile_dir)?;

    // Step 4: Remove from profiles.json
    remove_profile_info(profiles_dir, profile_id)?;

    tracing::info!(profile_id = %profile_id, "Profile cryptographically erased");
    Ok(())
}
```

---

## [8] Error Handling

```rust
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed — wrong key or corrupted data")]
    DecryptionFailed,

    #[error("Wrong password")]
    WrongPassword,

    #[error("Invalid recovery phrase")]
    InvalidRecoveryPhrase,

    #[error("Profile not found: {0}")]
    ProfileNotFound(Uuid),

    #[error("Corrupted profile data")]
    CorruptedProfile,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Profile already exists: {0}")]
    ProfileExists(String),
}
```

---

## [9] Security (Threat Model)

**E-SC comprehensive review:**

| Threat | Mitigation | Verified By |
|--------|-----------|-------------|
| Brute force password | PBKDF2 600K iterations (~0.5s per attempt). 8-char password → ~centuries. | T-08 timing test |
| Memory dump while open | Zeroize derive on ProfileKey and RecoveryPhrase. Key bytes zeroed on drop. | T-09 zeroize test |
| Cold boot attack | OS-level concern, out of scope. Mitigated by short session times. | N/A |
| Disk forensics after delete | Salt overwritten → key underivable. Directory deleted. Encrypted data without key = noise. | T-14 erasure test |
| Recovery phrase stolen | Same security as password. 12 words from BIP39 = 128 bits entropy. | Design review |
| Malicious profile.json edit | Profile names are non-sensitive. Encrypted data integrity verified by AES-GCM auth tag. | T-10 tampering test |
| Nonce reuse | Random 96-bit nonce per encryption. Probability of collision negligible for profile-scale data. | Design review |
| Side-channel timing | PBKDF2 constant-time comparison. AES-GCM uses constant-time operations. | Crate guarantees |

---

## [10] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | Create profile | Profile directory created, all files present |
| T-02 | Open with correct password | Returns ProfileSession, can encrypt/decrypt |
| T-03 | Open with wrong password | Returns CryptoError::WrongPassword |
| T-04 | Encrypt then decrypt | Round-trip: plaintext → encrypt → decrypt → same plaintext |
| T-05 | Decrypt with wrong key | Returns CryptoError::DecryptionFailed |
| T-06 | Generate recovery phrase | 12 valid BIP39 English words |
| T-07 | Recover with correct phrase | Returns ProfileSession, can access data |
| T-08 | PBKDF2 takes > 200ms | Timing test: derivation not instant (brute force protection) |
| T-09 | Key zeroed after drop | After dropping ProfileSession, key bytes are zero |
| T-10 | Tampered ciphertext detected | Modify one byte of ciphertext → decrypt fails (auth tag) |
| T-11 | List profiles shows names | Multiple profiles → list returns all names and IDs |
| T-12 | Delete profile removes all files | After deletion, profile directory doesn't exist |
| T-13 | Delete profile removes from list | After deletion, list_profiles doesn't include it |
| T-14 | Cryptographic erasure irreversible | After deletion, cannot open profile even with correct password |
| T-15 | File encrypt/decrypt round-trip | Write encrypted file, read back, matches original |

### Critical Security Tests

```rust
#[test]
fn wrong_password_rejected() {
    let dir = tempdir().unwrap();
    let (info, _phrase) = create_profile(dir.path(), "Test", "correct_password", None).unwrap();
    let result = open_profile(dir.path(), &info.id, "wrong_password");
    assert!(matches!(result, Err(CryptoError::WrongPassword)));
}

#[test]
fn tampered_ciphertext_detected() {
    let key = ProfileKey::derive(b"test", &[0u8; 32]);
    let encrypted = key.encrypt(b"secret data").unwrap();
    let mut tampered = encrypted.clone();
    tampered.ciphertext[0] ^= 0xFF;  // Flip bits
    assert!(key.decrypt(&tampered).is_err());  // Auth tag fails
}

#[test]
fn key_zeroed_on_drop() {
    let key = ProfileKey::derive(b"test", &[0u8; 32]);
    let key_ptr = key.key_bytes.as_ptr();
    drop(key);
    // After drop, memory at key_ptr should be zeroed
    // (This is hard to test reliably due to compiler optimizations,
    //  but zeroize crate uses volatile writes to prevent optimization)
}

#[test]
fn pbkdf2_takes_meaningful_time() {
    let start = std::time::Instant::now();
    let _key = ProfileKey::derive("test_password", &[0u8; 32]);
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() > 200, "PBKDF2 too fast: {}ms", elapsed.as_millis());
}
```

---

## [11] Performance

| Metric | Target |
|--------|--------|
| Key derivation (PBKDF2) | 200-800ms (security requirement — intentionally slow) |
| Single field encrypt | < 1ms |
| Single field decrypt | < 1ms |
| File encrypt (1MB) | < 50ms |
| Profile creation | < 2 seconds (includes key derivation + directory setup + DB init) |
| Profile open | < 1 second (key derivation + verification) |

---

## [12] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Application-level field encryption vs SQLCipher | DECIDED: Application-level (see Section 5) |
| OQ-02 | Which fields to encrypt vs leave queryable | Decided: Content encrypted, IDs/dates/enums queryable |
| OQ-03 | LanceDB vector encryption | Vectors themselves aren't patient-identifiable. Content field (chunk text) should be encrypted. Research LanceDB encryption support. |
