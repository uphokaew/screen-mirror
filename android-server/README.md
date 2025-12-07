# Android Server Build Guide

## Prerequisites

1. **Android Studio** (Latest version recommended)
2. **JDK 8 or higher**
3. **Android SDK** with API 33

## Build Instructions

### Option 1: Build with Android Studio

1. Open Android Studio
2. Click "Open an existing project"
3. Navigate to `scrcpy-custom/android-server`
4. Wait for Gradle sync
5. Click Build → Build Bundle(s)/APK(s) → Build APK(s)
6. Find APK in `app/build/outputs/apk/debug`

### Option 2: Build via Command Line

```bash
cd android-server

# On Windows
gradlew.bat assembleDebug

# On Linux/macOS
./gradlew assembleDebug
```

APK will be in: `app/build/outputs/apk/debug/app-debug.apk`

## Installation

```bash
# Install APK
adb install app/build/outputs/apk/debug/app-debug.apk

# Or if already installed
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## Usage

1. **Launch app** on Android device
2. **Click "Start Screen Capture"**
3. **Grant permission** when prompted
4. **Note the IP address** displayed
5. **On PC**, run:
   ```powershell
   cargo run --release -- --mode tcp --host <ANDROID_IP> --port 5555
   ```

## Testing with Local PC

### USB Connection

```bash
# Forward ADB port
adb forward tcp:5555 tcp:5555

# On PC
cargo run --release -- --mode tcp --host 127.0.0.1 --port 5555
```

### WiFi Connection

```bash
# Get Android IP
adb shell ip addr show wlan0 | findstr "inet "

# Connect PC to same WiFi
cargo run --release -- --mode quic --host <ANDROID_IP> --port 5556
```

## Project Structure

```
android-server/
├── app/
│   ├── build.gradle                  # App dependencies
│   └── src/main/
│       ├── AndroidManifest.xml       # Permissions and components
│       ├── java/com/scrcpy/custom/server/
│       │   ├── MainActivity.java          # Main UI activity
│       │   ├── NetworkUtils.java          # Network utilities
│       │   ├── service/
│       │   │   └── ScreenCaptureService.java  # Capture service
│       │   ├── encoder/
│       │   │   └── VideoEncoder.java      # H.264 encoder
│       │   └── network/
│       │       └── NetworkStreamer.java   # TCP streaming
│       └── res/
│           ├── layout/
│           │   └── activity_main.xml  # UI layout
│           └── values/
│               └── strings.xml        # String resources
└── build.gradle                       # Project config
```

## Troubleshooting

### Issue: Build fails

**Solution**:
```bash
# Clean build
gradlew clean
gradlew assembleDebug
```

### Issue: Permission denied

**Solution**: Grant screen capture permission in app

### Issue: Connection refused

**Solution**:
1. Check Android firewall settings
2. Ensure both devices on same network (WiFi mode)
3. Check ADB forward (USB mode)

## Features

✅ **Hardware H.264 Encoding** - Uses MediaCodec with hardware encoder selection  
✅ **Zero-Copy Pipeline** - Surface → MediaCodec → Network  
✅ **Dynamic Bitrate** - Supports bitrate adjustment from PC client  
✅ **Protocol Compatible** - Matches Rust client protocol exactly  
✅ **Low Latency** - Optimized for real-time streaming  

## Configuration

Default settings (can be modified in `ScreenCaptureService.java`):
- **Resolution**: Full screen (detected automatically)
- **Bitrate**: 8 Mbps
- **Frame Rate**: 60 FPS
- **Codec**: H.264 (hardware accelerated)
- **Port**: 5555 (TCP)

## Next Steps

1. Build and install Android server
2. Test with Rust client
3. Measure end-to-end latency
4. Fine-tune settings for your device

---

**Status**: ✅ Complete and ready to build!
