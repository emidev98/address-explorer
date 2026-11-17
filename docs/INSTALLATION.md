# Installing release builds

Download artifacts from the repository's GitHub Releases page. The release
workflow publishes checksums in `SHA256SUMS.txt`. For the runtime modes and
build/deployment diagram, see [`ARCHITECTURE.md`](ARCHITECTURE.md).

Verify a download with:

```sh
sha256sum -c SHA256SUMS.txt
```

## Linux

- **AppImage**: make it executable and run it:

  ```sh
  chmod +x address-explorer_<version>_linux_x86_64.AppImage
  ./address-explorer_<version>_linux_x86_64.AppImage
  ```

  If your distribution does not have AppImage FUSE support, run it with
  `--appimage-extract-and-run`.

- **Debian/Ubuntu**: install the `.deb`:

  ```sh
  sudo apt install ./address-explorer_<version>_linux_x86_64.deb
  ```

## macOS and Windows

The release workflow does not publish macOS or Windows desktop bundles by
default; only the web, Linux, Android, and server artifacts are attached to a
release. Build the desktop app on the target platform instead:

```sh
cargo prepare
dx bundle --desktop --release --locked --no-default-features
```

See [`BUILDING.md`](BUILDING.md) for the per-platform prerequisites.

If a build is ever published, treat it as unsigned. Without an Apple Developer
ID a macOS bundle is ad-hoc signed, so Gatekeeper reports an unidentified
developer and the first launch needs **Open Anyway**; the NSIS installer is
unsigned, so SmartScreen warns before running it. Only run builds downloaded
from the official release page.

## Android

The release APK is **debug-signed**, arm64-only
(`address-explorer_<version>_android_arm64.apk`), and supports Android 7.0
(API 24) and newer, including Android 14, 15 and 16. It is installable for
testing but is not a Play Store artifact and cannot be updated with a production
signing key.

Enable installation from unknown sources for your browser or file manager, then
open the APK. Do not use the debug APK for production data.

## Web

The deployed PWA is Local-first: the database lives in the browser through a
SharedWorker. Remote mode is optional and requires a matching Address Explorer
server. See [`DEPLOYMENT.md`](DEPLOYMENT.md).

The PWA requires a current browser with SharedWorker and service-worker support,
served over HTTPS or `localhost`.
