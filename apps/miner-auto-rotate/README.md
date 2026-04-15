# HomeChain-Miner ⛏️
## Desktop & Mobile Mining Application

Welcome to the **HomeChain-Miner** source code directory. This is a lightweight, high-performance mining application built using the [Tauri Framework](https://tauri.app/) (Rust + JavaScript/Vite). It acts as the primary GUI client for end-users to participate in the HomeChain PoW network from both **Windows PCs** and **Android smartphones**.

---

## 🌟 Key Features
- **Zero-Config Mining**: Automatically connects to the active RPC Node Gateway (`http://rpc.homechain.online`) for immediate block pulling.
- **Eco-Friendly Default**: To prevent Anti-Virus generic heuristics and keep consumer PCs responsive, the miner automatically limits its execution to **1 CPU Core / Thread** by default. Users can manually increase this via the UI slider.
- **Race-Condition Free**: Features an ultra-responsive Stop mechanism. Intercepts the Tokio `watch` channel to instantly pause CPU hashing without waiting for the block cycle to finish.
- **Cross-Platform**: Runs natively on **Windows** (NSIS/MSI installer) and **Android** (APK sideload).

---

## 📱 Android-Specific Features
- **Foreground Service**: Native Kotlin implementation (`MiningForegroundService.kt`) ensures Android doesn't kill the mining process when the app is minimized. A persistent notification reads "⛏️ Mining running in background..."
- **WakeLock**: `PowerManager.PARTIAL_WAKE_LOCK` keeps the CPU hashing even when the screen is off.
- **Death-on-Swipe**: When the user swipes the app away from Recent Apps, `onDestroy()` triggers `stopService()` + `Process.killProcess()` to guarantee zero battery drain after closure.
- **1-CPU Constraint**: Same eco-friendly default as Windows — only 1 core is used to maintain thermal stability on mobile devices.

---

## 🛠️ Development & Build Guide

### Prerequisites (All Platforms)
1. **Node.js** (v18+)
2. **Rust & Cargo** (latest stable)

### Prerequisites (Windows Only)
3. **C++ Build Tools** (MSVC)

### Prerequisites (Android Only)
3. **Android Studio** with SDK + NDK 27.x
4. **Rust Android Targets**:
   ```bash
   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
   ```
5. **Windows Developer Mode** enabled (required for symlinks)
6. **Environment Variables**:
   ```powershell
   $env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
   $env:NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\27.3.13750724"
   ```

### Running Locally (Dev Mode)
To spin up the development application with Hot-Module Replacement (HMR):
```bash
npm install
npm run tauri dev
```

---

## 🏗️ Building for Production

### Windows Installer
To compile into deployable Windows Installers (`.exe` and `.msi`):
```bash
npm run tauri build
```
**Output:**
- **Setup EXE:** `../../target/release/bundle/nsis/HomeChain-Miner_[VERSION]_x64-setup.exe`
- **MSI Installer:** `../../target/release/bundle/msi/HomeChain-Miner_[VERSION]_x64_en-US.msi`

### Android APK
To compile into a sideloadable Android APK:
```powershell
# Step 1: Set environment
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\27.3.13750724"

# Step 2: Build unsigned APK
npx tauri android build --target aarch64

# Step 3: Zipalign
& "$env:ANDROID_HOME\build-tools\36.1.0\zipalign.exe" -f 4 `
  "src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release-unsigned.apk" `
  "src-tauri\gen\android\app\build\outputs\apk\universal\release\homechain-miner-aligned.apk"

# Step 4: Sign with keystore
& "$env:ANDROID_HOME\build-tools\36.1.0\apksigner.bat" sign `
  --ks "src-tauri\gen\android\homechain-release.jks" `
  --ks-key-alias homechain `
  --ks-pass "pass:YOUR_KEYSTORE_PASSWORD" `
  --key-pass "pass:YOUR_KEYSTORE_PASSWORD" `
  --out "src-tauri\gen\android\app\build\outputs\apk\universal\release\homechain-miner-signed.apk" `
  "src-tauri\gen\android\app\build\outputs\apk\universal\release\homechain-miner-aligned.apk"
```
**Output:** `src-tauri/gen/android/app/build/outputs/apk/universal/release/homechain-miner-signed.apk` (~13.4 MB)

> ⚠️ **Important**: The APK **must** be signed before it can be installed. Android rejects unsigned APKs at the OS level — this is not optional.

---

## 🎨 Asset Management
To update the application's icon across all platforms (Windows `.ico`, Android `mipmap`), place a transparent `logo.png` (512x512+) in the `src-tauri` folder:
```bash
npx tauri icon src-tauri/logo_transparent.png
```
This auto-generates icons for Windows, macOS, iOS, and all 5 Android density buckets (`mdpi` through `xxxhdpi`).

---

## 📂 Key File Locations

| File | Purpose |
|---|---|
| `src/main.js` | Frontend JS — thread control, UI, lifecycle hooks |
| `src/styles.css` | Tailwind + custom CSS |
| `src-tauri/src/lib.rs` | Rust backend — Tauri commands, mining worker |
| `src-tauri/tauri.conf.json` | App identity, window size, permissions |
| `src-tauri/gen/android/` | Generated Android project (Gradle + Kotlin) |
| `src-tauri/gen/android/homechain-release.jks` | Android signing keystore (RSA 2048-bit, 27yr validity) |
| `src-tauri/gen/android/.../MiningForegroundService.kt` | Native Kotlin WakeLock + notification service |
| `src-tauri/gen/android/.../MainActivity.kt` | Android lifecycle hooks (service start/stop) |
| `src-tauri/gen/android/.../AndroidManifest.xml` | Permissions: WAKE_LOCK, FOREGROUND_SERVICE, etc. |

---

## 🔒 Security Notes

### Windows
Because this application contains cryptographic hashing loops (SHA-256), Windows Defender or other AV software may flag it as a generic `CoinMiner` Trojan (False Positive).
*Permanent Solution:* Sign the final `.exe` with an **EV Code Signing Certificate** or have users manually whitelist the application.

### Android
- The APK is self-signed using a local keystore (`homechain-release.jks`). Google Play Protect **will** show a warning ("Unrecognized developer") during installation — users must tap "More details → Install anyway".
- This app is designed for **sideloading only**. Uploading to Google Play Store will likely violate their cryptocurrency mining policy.
- The keystore file must be kept safe and never lost — future APK updates **must** be signed with the same keystore, or users will need to uninstall and reinstall.

---

## 🔧 Cross-Compilation Notes
The `reqwest` crate was patched to use `rustls-tls` instead of `native-tls` to avoid OpenSSL C-library linker failures during ARM cross-compilation. This change is transparent and has zero impact on Windows builds.
```toml
# core/home-miner/Cargo.toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```
