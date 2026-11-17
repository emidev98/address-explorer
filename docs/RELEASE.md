# Releases

Releases are built by [`.github/workflows/release.yml`](../.github/workflows/release.yml)
when a **stable** GitHub release is published. Pre-releases are ignored.

For the build and deployment topology, see
[`ARCHITECTURE.md`](ARCHITECTURE.md). For server/PWA operations, see
[`DEPLOYMENT.md`](DEPLOYMENT.md).

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
- Builds a debug-signed Android APK.
- Builds Linux x86_64/arm64 server tarballs and a multi-arch GHCR image.
- Builds the GHCR image from the prebuilt server binary using
  `.github/docker/server-runtime.Dockerfile`; the repository root `Dockerfile`
  is a separate source-build alternative.
- Generates `SHA256SUMS.txt`.
- Appends a grouped commit changelog to the release notes.
- Deploys the web PWA to Vercel.
- Links the release notes to the production deployment.

All build jobs must succeed before any asset is attached or Vercel is
deployed.

## Desktop artifacts

The workflow publishes the Linux AppImage and `.deb` bundles only. It does not
build macOS or Windows desktop bundles, so a release never queues a native
runner; users on those platforms build the app from source, as described in
[`INSTALLATION.md`](INSTALLATION.md) and [`BUILDING.md`](BUILDING.md).

Desktop code is still compiled on every pull request and every push to `main`:
`dx check --platform desktop`, the desktop Cargo tests, and desktop Clippy run
on Linux. Platform packaging — codesigning, notarization, NSIS — is out of scope
for both CI and releases.

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
Keep the project's Build Command empty so a dashboard or CLI source deploy
cannot run `cargo build` on Vercel. For a manual deploy from a prepared
checkout, use `scripts/deploy-vercel.sh --prod`.

The production alias is `https://address-explorer-teal.vercel.app`. The deploy
job compares the alias Vercel reports with the repository variable
`VERCEL_PRODUCTION_URL` (the alias above when the variable is unset) and fails
when they differ, so a deployment to an unexpected host cannot pass silently.
Once the assets are attached, the `Link the deployment in the release notes` job
appends a `## Web app` section delimited by
`<!-- address-explorer-deployment:start -->` and
`<!-- address-explorer-deployment:end -->` to the release body, and records the
same URL on the run's deployment environment. Re-running the workflow replaces
the section instead of duplicating it.

## Retrying a release

Use **Actions → Release → Run workflow** with the existing tag. The same
version, branch, and CI gates apply. Asset uploads use `--clobber`, so a retry
replaces previous artifacts.

If CI is still running or failed for the tag commit, wait for or fix `main`
first.

## Signing

Android builds a debug-signed APK. A future production release needs a keystore
and the equivalent `ANDROID_KEYSTORE_*` secrets; the build should then produce a
signed APK and AAB instead of the debug APK.

Linux bundles are unsigned. macOS and Windows bundles are not built here, so
their codesigning and notarization requirements do not apply to this workflow.

Signing credentials **MUST NOT** be committed to the repository.

## Notes and limitations

- Only the Linux desktop bundles are published; macOS and Windows users build
  from source. See [`INSTALLATION.md`](INSTALLATION.md).
- The Android APK is a debug build, not a Play Store artifact.
- The server image is published to GHCR; no live backend is deployed. See
  [`DEPLOYMENT.md`](DEPLOYMENT.md).
- If GitHub **immutable releases** are enabled, assets must be attached while
  the release is a draft. The current workflow uploads after publication and
  will fail once immutability is enabled.
- The Vercel Hobby plan is limited to personal, non-commercial use.
