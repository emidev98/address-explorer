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

## macOS

The macOS build is ad-hoc signed but not notarized, so Gatekeeper stops the
first launch. Without an Apple Developer ID, macOS reports that the app cannot
be verified; releases published before v0.0.3 were unsigned and were reported as
damaged instead.

1. Open the `.dmg` and drag `AddressExplorer.app` to Applications.
2. Open **System Settings → Privacy & Security**, find the message about
   `AddressExplorer`, and choose **Open Anyway**.
3. If the app still refuses to start, clear the quarantine flag:

   ```sh
   xattr -dr com.apple.quarantine "/Applications/AddressExplorer.app"
   ```

`codesign --verify --deep --strict "/Applications/AddressExplorer.app"` confirms
that the bundle signature is intact. Only run builds you downloaded from the
official release page.

## Windows

The NSIS installer is unsigned, so SmartScreen may show a warning. Choose
**More info → Run anyway** if you trust the download. The portable `.zip`
contains the executable and can be extracted anywhere; WebView2 is required and
is included with current Windows 10 and Windows 11.

## Android

The release APK is **debug-signed**. It is installable for testing but is not a
Play Store artifact and cannot be updated with a production signing key.

Enable installation from unknown sources for your browser or file manager, then
open the APK. Do not use the debug APK for production data.

## Web

The deployed PWA is Local-first: the database lives in the browser through a
SharedWorker. Remote mode is optional and requires a matching Address Explorer
server. See [`DEPLOYMENT.md`](DEPLOYMENT.md).

The PWA requires a current browser with SharedWorker and service-worker support,
served over HTTPS or `localhost`.
