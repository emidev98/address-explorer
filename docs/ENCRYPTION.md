# Database encryption

Address Explorer encrypts the active local or remote database at rest. Every
build requires a password on first launch; there is no unencrypted mode. The
password is created in a blocking first-run screen, and the inactivity timeout
lives in **Settings → Security**.

## What it protects

Every page of the SQLite database is encrypted, including indexes, triggers,
and reference data. Native and pure-server builds use SQLCipher. The browser
Local backend uses SQLite3MC with its SQLCipher-compatible codec, so
`relaxed-idb` stores encrypted pages.

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
- The derived key is held in zeroizing memory while unlocked. Locking, the
  inactivity timeout, a password change, or process exit drops it.
- 256-bit symmetric keys remain approximately 128-bit secure under Grover's
  algorithm, which is the accepted post-quantum target for symmetric
  encryption. No public-key cryptography is involved in local at-rest
  encryption.

The password is not stored as a hash. A hash cannot decrypt the database, and
the SQLCipher key check already rejects an incorrect password.

## Lifecycle

- **First run:** a blocking screen requires a password before any data is
  served. An existing plaintext database is encrypted in place and keeps its
  data, security policy, and inactivity timeout.
- **Setup:** a fresh database is created encrypted. A plaintext database from
  an older build is copied into a new encrypted file after schema, integrity,
  and foreign-key validation. The plaintext file is replaced only after the
  encrypted copy validates.
- **Unlock:** the browser or native application reads the public salt, derives
  the key with Argon2id, and verifies it by opening the database. A wrong
  password fails closed.
- **Idle lock:** the default is one hour. The options are 1 minute, 5 minutes,
  10 minutes, 1 hour, 6 hours, and 24 hours. The same value bounds the remote
  server session, so there is one inactivity timer. Only explicit user input
  resets it; background polling cannot keep the vault open. A network server
  applies at most one hour and rejects larger values. Nothing about the key is
  persisted.
- **Password policy:** the minimum length is 12 characters by default and is
  configurable in **Settings → Security** within the safe
  range. Existing passwords are still accepted for unlock.
- **Change password:** the database is re-encrypted with a new Argon2id key
  and salt. A safety copy is kept only for the duration of the operation.
- **Import:** validates the canonical schema, rebuilds application data into a
  fresh database, and rejects triggers, views, extra objects, and altered
  schema SQL. Encrypted backups are authenticated by SQLCipher.
- **Export:** every export is encrypted with a separate backup passphrase.
- **Clear database:** removes user data but keeps the current encryption key.
- **Lost password:** on a server, run `address-explorer reset-vault` on the
  host and type `ERASE`. In desktop and browser builds, use **Forgot
  password?** on the unlock screen. Erasing returns to the setup screen and
  requires a new password. No recovery key, password hint, or bypass exists.

## Remote servers

Remote clients send the password only to setup, unlock, and change-password
endpoints over TLS. Unlock always verifies the password, even when the vault is
already open, and returns one active session token. Every data request must
send `Authorization: Bearer <token>`. A new unlock replaces the token; locking
or the inactivity timeout clears the vault key and the token. The inactivity
timeout is server-wide, and a server restart always locks the vault. While
locked, data endpoints fail closed; locked reset is refused and must be done on
the host.

## Limitations

- A lost password is unrecoverable by design.
- Core dumps are disabled on Unix builds, but swap, hibernation images, crash
  reporters, and hypervisor memory snapshots can still capture live secrets.
  Use encrypted swap and restrict host access for production servers.
- Removed plaintext database files are overwritten before unlinking and
  SQLite `secure_delete` is enabled, but SSDs, copy-on-write filesystems,
  snapshots, and backups may retain old blocks. This is best-effort erasure.
- Encryption does not protect a running application from malware, screen
  capture, browser extensions, or a compromised server process.
- The database size and access times remain visible.
- An attacker can delete or roll back the encrypted file; that is a
  denial-of-service or rollback risk, not a way to read it.
- Older application builds cannot open an encrypted database. Export a backup
  from the current build and import it with the same build.
- Encrypted database files are implementation-specific (native/server SQLCipher
  versus browser SQLite3MC). Use the encrypted export/import flow to move data
  between installations.
