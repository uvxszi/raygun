# Platforms

Raygun is built for Android first and Windows second; the release workflow
(`.github/workflows/release.yml`) also builds it for Linux, macOS and FreeBSD. This page lists what
each release contains, what each platform needs, and what doesn't work where.

## Release files

A tag `v<version>` builds every job below. The release waits for Windows x64 and Android; the rest
add their files when they built (a failed one shows in the run and leaves its files out).

| Platform | Files | Built on | Starts in CI |
|---|---|---|---|
| Windows x64 | `Raygun_<v>_x64-setup.exe`, `Raygun_<v>_x64_en-US.msi`, `Raygun_<v>_x64_portable.exe` | windows-latest | – |
| Windows x86 (32-bit) | `Raygun_<v>_x86-setup.exe`, `Raygun_<v>_x86_en-US.msi`, `Raygun_<v>_x86_portable.exe` | windows-latest (cross) | yes |
| Windows ARM64 | `Raygun_<v>_arm64_setup.exe`, `Raygun_<v>_arm64_portable.exe` | windows-latest (cross) | – |
| Android | `Raygun_<v>_android_universal.apk`, `_arm64.apk`, `_x86_64.apk` (signed) | ubuntu-24.04 | – |
| Linux x64 | `Raygun_<v>_amd64.AppImage`, `Raygun_<v>_amd64.deb`, `Raygun_<v>_x86_64.rpm` | ubuntu-22.04 | AppImage and installed deb |
| Linux ARM64 | `Raygun_<v>_aarch64.AppImage`, `Raygun_<v>_arm64.deb`, `Raygun_<v>_aarch64.rpm` | ubuntu-22.04-arm | AppImage and installed deb |
| Linux riscv64 | `Raygun_<v>_riscv64.deb`, `Raygun_<v>_riscv64.rpm` | Ubuntu 24.04 container (cross) | – |
| Flatpak | `Raygun_<v>_x86_64.flatpak`, `Raygun_<v>_aarch64.flatpak` | GNOME 50 runtime | – |
| Arch Linux, CachyOS | `raygun-bin-<v>-1-x86_64.pkg.tar.zst`, `raygun-bin-<v>-1-aarch64.pkg.tar.zst` | archlinux container | x86_64, installed with pacman |
| macOS | `Raygun_<v>_aarch64.dmg` (Apple silicon), `Raygun_<v>_x64.dmg` (Intel) | macos-15 | yes |
| FreeBSD x86_64 | `Raygun_<v>_freebsd_x86_64.tar.gz` | FreeBSD 14.3 VM | – |

"Starts in CI" means the build is started (on Linux in a virtual display) with a data folder of its
own, must log that its start went well (20 healthy seconds) within 90 seconds, still be running then
and have written no crash record. A screenshot of each is kept with the run (`screenshots-*`).

The names are what the in-app updater looks for (`updates.rs` `pick_asset`); keep them. Copies of
1.9.4 and earlier take the first `-setup.exe` and the first `.msi` in GitHub's order (by name), so
no other Windows file may sort before the x64 ones with those endings. That's why ARM64's setup is
`_arm64_setup.exe` and ARM64 has no MSI.

To test the workflow without releasing: Actions → Release → Run workflow on a branch with no tag
(`gh workflow run release.yml --ref <branch>`). The files stay as the run's artifacts. `only`
picks jobs (`windows,windows-more,linux,linux-riscv64,macos,flatpak,arch,freebsd,android`), `quick`
builds with thin LTO.

## Updates

| Copy | How it updates |
|---|---|
| Windows setup exe / MSI | downloads the new setup or MSI for its processor and runs it |
| Android | downloads the universal APK, Android's installer updates over it |
| Linux AppImage | downloads the AppImage for its processor, puts it in place of its own file, starts again |
| Portable exe, deb, rpm, Arch, Flatpak, macOS, FreeBSD | opens the release page |

## Linux

Built on Ubuntu 22.04, so the binary needs glibc 2.35 or newer and WebKitGTK 4.1 (Ubuntu 22.04+,
Debian 12+, Fedora 37+, Arch, openSUSE Tumbleweed). The riscv64 build is cross-compiled against
Ubuntu 24.04's riscv64 libraries (glibc 2.39: Ubuntu 24.04+, Debian 13+, Fedora 40+): 22.04's
riscv64 archive lags its amd64 updates, and the two must agree on shared libraries' versions.

WebKitGTK plays sound and video with GStreamer: Raygun needs its base and good plugins, `libav`
for AAC and H.264 (most songs and videos), and bad for some streaming formats. HLS and DASH play
through Media Source Extensions in the page (hls.js, dash.js), which need the same plugins.

| Package | Packages it brings or asks for |
|---|---|
| AppImage | carries GStreamer and its plugins (from Ubuntu 22.04) |
| deb | depends on `gstreamer1.0-plugins-base`, `-good`, `gstreamer1.0-libav`; recommends `-bad` |
| rpm | recommends `gstreamer1-plugins-base`, `-good`, `-bad-free`, `gstreamer1-plugin-libav` (Fedora's names; RPM Fusion's `gstreamer1-libav` has every codec) |
| Arch | depends on `webkit2gtk-4.1`, `gst-plugins-base`, `-good`, `gst-libav`; optional `gst-plugins-bad` |
| Flatpak | the GNOME runtime has GStreamer; codecs Flathub ships as runtime extensions |

Where data goes: an AppImage keeps it in `raygun-data` beside the AppImage (portable). A copy a
package installed (`/usr`, `/opt`, a Flatpak's `/app`, a Snap) keeps it in the system's app-data
folder, `~/.local/share/app.raygun.player` (`~/.var/app/app.raygun.player/data/app.raygun.player`
in the Flatpak), without asking. `RAYGUN_HOME` overrides both.

With NVIDIA's own driver, WebKitGTK's DMA-BUF renderer leaves the window blank; Raygun turns it off
there (`WEBKIT_DISABLE_DMABUF_RENDERER=1`) unless the variable is set already.

Media keys and the desktop's media controls (MPRIS) come from WebKitGTK's Media Session support for
what plays in the page, where the system's WebKitGTK is built with it. In a Flatpak, WebKit names
its player after the app (`org.mpris.MediaPlayer2.app.raygun.player.*`), which Flatpak allows.

yt-dlp and FFmpeg are fetched for Linux x64 and ARM64 like on Windows (yt-dlp's own builds; on
riscv64 yt-dlp's zipapp, which needs Python 3). Voices for reading aloud (sherpa-onnx 1.13.8) are
in the catalogue for Linux x64 and ARM64. The Mihon container's Java runtime is pinned for both.

### What Linux doesn't have

- The built-in browser's ad and tracker blocker: it filters WebView2's requests (Windows only).
- Hidden web views for converted Mihon sources (`runWebView`): Windows (WebView2) and Android only.
- Browser checks by hand open the page with a plain request; on Windows the window repeats the
  source's own request.
- Choosing the audio output in Devices: WebKitGTK has no `setSinkId`.
- Reading screenshots for importing songs uses the downloadable ocrs model instead of Windows' OCR.

## Flatpak

`packaging/flatpak/app.raygun.player.yml` repackages the release's deb on `org.gnome.Platform` 50
(WebKitGTK 4.1, GStreamer) and checks that every library Raygun links is in the runtime. It has
network, sound, GPU, the Downloads, Music, Pictures and Videos folders, and notifications. Install a
bundle with `flatpak install --user Raygun_<v>_x86_64.flatpak` (it fetches the runtime from
Flathub). Publishing on Flathub would need a manifest that builds from source.

## Arch Linux and CachyOS

`packaging/arch/PKGBUILD` (`raygun-bin`) repackages the deb for x86_64 and aarch64. CachyOS,
EndeavourOS and Manjaro install the same package (`pacman -U raygun-bin-…pkg.tar.zst`); the
AppImage works there too. The aarch64 package is made on x86_64 (it's the same files) and is for
Arch Linux ARM.

## macOS

Unsigned and not notarized (that needs an Apple Developer ID). The first start of a downloaded app
needs right-click → Open, or `xattr -dr com.apple.quarantine /Applications/Raygun.app`. Raygun keeps
its data in `raygun-data` beside `Raygun.app` (in `/Applications` that's `/Applications/raygun-data`);
a copy started from the download's quarantine asks where to keep it. `Info.plist` asks for the local
network (linked devices, Connect, casting). Voices and the Mihon container's Java runtime are
pinned for both processors; FFmpeg comes from the system (Homebrew).

## FreeBSD

Experimental. Tauri has no FreeBSD bundler or CLI build, so the job builds with cargo in a FreeBSD
14.3 VM (thin LTO) and packs the binary, a desktop entry and an icon with a README. It needs
`webkit2-gtk_41` and GStreamer's plugins from packages. Not built for it: one running copy at a
time (the single-instance plugin has no BSD implementation), FFmpeg and yt-dlp downloads (yt-dlp's
zipapp works with Python), voices.

## Not built

- **iOS**: Raygun has no iOS project (`gen/apple`) or iOS side of its native player plugin, which is
  Android's Kotlin (playback in the background, the lock screen, video). An iOS app is only
  installable when signed with an Apple account, and the App Store doesn't allow code downloaded
  to run (extensions). An unsigned build for sideloading would first need the iOS project and the
  plugin's Swift side.
- **Linux riscv64 AppImage and Flatpak**: linuxdeploy has no riscv64 build and Flathub builds no
  riscv64 runtime. sherpa-onnx's riscv64 build is for T-Head's processors only, so there are no
  voices on riscv64.
- **Windows ARM64 MSI**: an ARM64 `.msi` would sort before the x64 one, and copies of 1.9.4 and
  earlier would take it for theirs.
