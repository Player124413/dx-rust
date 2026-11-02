# DX RUST Android

This is a native Bevy/Rust port with a Java launcher and GameActivity. It does **not** include Deus Ex data.

## Build

Requirements: JDK 17, Android SDK 36, NDK 27.2.12479018, Rust 1.95+, `cargo-ndk` 4.1.2, Gradle 8.13.

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --version 4.1.2 --locked
export ANDROID_SDK_ROOT=/path/to/Android/Sdk
./android/build_rust.sh
gradle -p android assembleRelease
```

The APK is `android/app/build/outputs/apk/release/app-release.apk`. The CI workflow builds the same artifact.
The release build is debug-signed for sideloading; replace the signing config before any store release.

## First run

1. Install the APK and open **DX RUST**.
2. Tap **Import game folder** and select a legal Deus Ex GOTY folder (Steam/GOG copy). The launcher copies
   `System`, `Maps`, `Textures`, `Sounds`, and `Music` into app-private external storage.
3. Pick **Auto**, optionally enable the FPS overlay, then **PLAY**.
4. Use the **EDIT** touch button (or the launcher checkbox) to drag/resize/rebind controls. **SAVE / PLAY** persists
   `touch_layout.ron` in app storage.

## Performance policy

- Auto starts with a 720p-class 3D pixel budget and adapts render scale with hysteresis; HUD/touch stay native-res.
- Battery/Balanced cap expensive enhancements and texture size. High allows the desktop enhancement mix.
- ETC2 is used only when the GPU reports support; unsupported devices fall back to capped RGBA.
- The first load of a texture may transcode and cache ETC2. Later loads reuse the cache. This is a load-time cost,
  not a per-frame cost. 60 FPS is a target, not a guarantee on every device.
