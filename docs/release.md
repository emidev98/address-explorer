# Releases

Releases are built by [`.github/workflows/release.yml`](../.github/workflows/release.yml)
when a **stable** GitHub release is published. Pre-releases are ignored.

## Release checklist

1. Update the version in `Cargo.toml`.
2. Refresh `Cargo.lock` with any Cargo command, for example:

   ```sh
   cargo check -p address-explorer --no-default-features --features server
   ```

3. Commit both files and merge to `main`.
4. Wait for the `CI` workflow to finish successfully on `main`.
5. Create a GitHub release with tag `vX.Y.Z` targeting that green `main` commit.
6. Publish the release.

The workflow validates:

- the tag matches `vX.Y.Z`;
- the tag version matches `Cargo.toml`;
- the tagged commit is reachable from `main`;
- a successful `CI` run exists for that commit on `main`.

## What the workflow does

- Builds the web PWA and attaches `address-explorer-web_<version>.zip`.
- Builds Linux x86_64 and arm64 AppImage/`.deb` bundles.
- Builds macOS arm64 `.dmg` and `.app.zip`.
- Builds the Windows x86_64 NSIS installer and portable `.zip`.
- Builds a debug-signed Android APK.
- Builds Linux x86_64/arm64 server tarballs and a multi-arch GHCR image.
- Generates `SHA256SUMS.txt`.
- Appends a grouped commit changelog to the release notes.
- Deploys the web PWA to Vercel.

All build jobs must succeed before any asset is attached or Vercel is deployed.

## Release notes

The generated section is delimited by:

```md
<!-- address-explorer-changelog:start -->
...
<!-- address-explorer-changelog:end -->
```

Your manually written notes are preserved. Re-running the workflow replaces only
the delimited section. Commits are grouped into Features, Fixes, Improvements,
Maintenance, and Other Changes using their conventional prefix.

## Vercel

The deployment uses the GitHub environment named **`Deploy Vercel`**. It must
contain:

- secret `VERCEL_TOKEN`;
- variable `VERCEL_ORG_ID`;
- variable `VERCEL_PROJECT_ID`.

The workflow builds a Vercel Build Output directory from the PWA and runs
`vercel deploy --prebuilt --prod`, so Vercel never needs the Rust toolchain.
`vercel.json` disables the Vercel Git integration to avoid duplicate deploys.

## Retrying a release

Use **Actions → Release → Run workflow** with the existing tag. The same
version, branch, and CI gates apply. Asset uploads use `--clobber`, so a retry
replaces previous artifacts.

If CI is still running or failed for the tag commit, wait for or fix `main`
first.

## Signing hooks

Desktop artifacts are currently unsigned. The workflows contain comments marking
where signing steps belong; the following secrets are reserved for enabling them:

- macOS: `APPLE_CERTIFICATE_P12` (base64), `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`, and
  `APPLE_APP_SPECIFIC_PASSWORD`.
- Windows: `WINDOWS_CERTIFICATE_PFX` (base64) and
  `WINDOWS_CERTIFICATE_PASSWORD`.

Android currently builds a debug-signed APK. A future production release needs a
keystore and the equivalent `ANDROID_KEYSTORE_*` secrets; the build should then
produce a signed APK and AAB instead of the debug APK.

Signing credentials **MUST NOT** be committed to the repository.

## Notes and limitations

- Desktop and Android artifacts are unsigned or debug-signed. See
  [`installation.md`](installation.md).
- The Android APK is a debug build, not a Play Store artifact.
- The server image is published to GHCR; no live backend is deployed. See
  [`deployment.md`](deployment.md).
- If GitHub **immutable releases** are enabled, assets must be attached while
  the release is a draft. The current workflow uploads after publication and
  will fail once immutability is enabled.
- The Vercel Hobby plan is limited to personal, non-commercial use.
