# Database encryption

Address Explorer can encrypt the active local or remote database at rest. The
feature is optional: a fresh installation asks once whether to set a password,
and the user may continue without encryption. The setting lives in
**Settings → General → Data**.

## What it protects

When encryption is enabled, every page of the SQLite database is encrypted,
including indexes, triggers, and reference data. Native and pure-server builds
use SQLCipher. The browser Local backend uses SQLite3MC with its
SQLCipher-compatible codec, so `relaxed-idb` stores encrypted pages.

The feature protects data at rest: a copied database file, a stolen browser
profile, or an offline disk cannot be read without the password. It does not
protect data while the vault is unlocked, and it is not authentication. The
remote server still has no user accounts or authorization; anyone who can
reach it while it is unlocked can use the API.

## Cryptography

- Password key derivation: **Argon2id** (RFC 9106, version 1.3), 64 MiB of
  memory, 3 passes, 1 lane, a 16-byte random salt, and a 32-byte output.
- Database cipher: **SQLCipher 4** compatibility, AES-256-CBC with
  HMAC-SHA512, 4096-byte pages.
- The Argon2id output is used as SQLCipher's raw key. SQLCipher's built-in
  PBKDF2 is bypassed, so the Argon2 cost is paid once per unlock rather than
  once per database request.
- The 16-byte salt is the public SQLCipher header salt. The password and the
  derived key are never written to the database.
- The derived key is held in zeroizing memory while unlocked. Locking, idle
  expiry, a password change, or process exit drops it.
- 256-bit symmetric keys remain approximately 128-bit secure under Grover's
  algorithm, which is the accepted post-quantum target for symmetric
  encryption. No public-key cryptography is involved in local at-rest
  encryption.

The password is not stored as a hash. A hash cannot decrypt the database, and
the SQLCipher key check already rejects an incorrect password.

## Lifecycle

- **First run:** a modal offers to set a password or continue without
  encryption. The answer is recorded in the `setting` table.
- **Setup:** an existing plaintext database is copied into a new encrypted
  file after schema, integrity, and foreign-key validation. The plaintext file
  is replaced only after the encrypted copy validates.
- **Unlock:** the browser or native application reads the public salt, derives
  the key with Argon2id, and verifies it by opening the database. A wrong
  password fails closed.
- **Idle lock:** the default is one hour. The options are 1 minute, 5 minutes,
  10 minutes, 1 hour, 6 hours, 24 hours, and Never. The timer is reset by user
  activity and database requests. Nothing about the key is persisted.
- **Change password:** the database is re-encrypted with a new Argon2id key
  and salt. A safety copy is kept only for the duration of the operation.
- **Import:** accepts a plaintext SQLite backup, validates and migrates it, and
  writes it into the encrypted database with the current key.
- **Export:** always produces a plaintext SQLite file. Store exported files
  securely.
- **Clear database:** removes user data but keeps the current encryption key.
- **Lost password:** the only path is to erase the encrypted database and
  start again. A previously exported plaintext backup can be imported
  afterwards. No recovery key, password hint, or bypass exists.

## Remote servers

Remote clients send the password to the server only for setup, unlock, and a
password change. Use HTTPS, except on loopback. The server derives the key,
discards the password, and keeps only the key in memory while the database is
unlocked. The idle timeout is server-wide: any active client heartbeat keeps
the vault unlocked, and a server restart always locks it. When the vault is
locked, every data endpoint fails closed; only health, encryption status,
unlock, and the destructive reset endpoint are available.

## Limitations

- A lost password is unrecoverable by design.
- Encryption does not protect a running application from malware, screen
  capture, browser extensions, or a compromised server process.
- The database size and access times remain visible.
- An attacker can delete or roll back the encrypted file; that is a
  denial-of-service or rollback risk, not a way to read it.
- Exported plaintext backups are not encrypted.
- Older application builds cannot open an encrypted database. Export a
  plaintext backup before enabling encryption if an older build may be needed.
- Encrypted database files are implementation-specific (native/server SQLCipher
  versus browser SQLite3MC). Use the plaintext export/import flow to move data
  between installations.
