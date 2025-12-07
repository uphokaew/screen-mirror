# Cross-Platform Installation Guide

## Windows

### Prerequisites
```powershell
# Install Rust
winget install Rustlang.Rustup

# Install FFmpeg
choco install ffmpeg-full

# Install ADB (optional - can use scrcpy's ADB)
choco install adb
```

### Build
```powershell
cd scrcpy-custom
cargo build --release
```

### Run
```powershell
# Use scrcpy's ADB if available
$env:Path += ";C:\Program Files\scrcpy"
adb forward tcp:5555 tcp:5555
cargo run --release -- --mode tcp
```

---

## Linux (Ubuntu/Debian)

### Prerequisites
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install dependencies
sudo apt update
sudo apt install -y \
    build-essential \
    pkg-config \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libswscale-dev \
    libva-dev \
    libvdpau-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libasound2-dev

# Install ADB
sudo apt install adb

# For NVIDIA users (NVDEC support)
sudo apt install nvidia-cuda-toolkit

# For Intel users (VAAPI support)
sudo apt install intel-media-va-driver i965-va-driver
```

### Build
```bash
cd scrcpy-custom
cargo build --release
```

### Run
```bash
# If using scrcpy's ADB
export PATH="/opt/scrcpy/adb:$PATH"  # adjust path as needed

adb forward tcp:5555 tcp:5555
cargo run --release -- --mode tcp --hw-decoder vaapi
```

### Linux-specific: VAAPI Setup

For AMD/Intel GPUs:
```bash
# Check VAAPI support
vainfo

# Should show:
# libva info: VA-API version 1.x.x
# libva info: Driver version: ...

# Run with VAAPI
cargo run --release -- --hw-decoder vaapi
```

---

## macOS

### Prerequisites
```bash
# Install Homebrew (if not installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install FFmpeg
brew install ffmpeg

# Install ADB
brew install android-platform-tools

# Or use scrcpy's ADB
brew install scrcpy  # includes ADB
```

### Build
```bash
cd scrcpy-custom
cargo build --release
```

### macOS-specific: Metal Backend

wgpu will automatically use Metal backend on macOS for optimal performance.

### Run
```bash
# Using system ADB or scrcpy's ADB
adb forward tcp:5555 tcp:5555
cargo run --release -- --mode tcp --hw-decoder auto
```

### macOS-specific: VideoToolbox (Future)

macOS has VideoToolbox for hardware decoding. Current implementation uses FFmpeg's software decoder, but hardware support can be added via:
```bash
# FFmpeg with VideoToolbox support
brew install ffmpeg --with-videotoolbox
```

---

## Platform-Specific Notes

### Hardware Decoder Support by Platform

| Platform | Decoder | Status | Command |
|----------|---------|--------|---------|
| Windows | NVDEC | ✅ Full | `--hw-decoder nvdec` |
| Windows | QSV | ✅ Full | `--hw-decoder qsv` |
| Linux | NVDEC | ✅ Full | `--hw-decoder nvdec` |
| Linux | VAAPI | ✅ Full | `--hw-decoder vaapi` |
| Linux | QSV | ✅ Full | `--hw-decoder qsv` |
| macOS | VideoToolbox | ⚠️ Via FFmpeg | `--hw-decoder auto` |

### Using scrcpy's ADB

All platforms can use scrcpy's bundled ADB:

**Windows**:
```powershell
# Add scrcpy to PATH
$env:Path += ";C:\Program Files\scrcpy"
```

**Linux**:
```bash
# Symlink scrcpy's ADB
sudo ln -s /opt/scrcpy/adb /usr/local/bin/adb
```

**macOS**:
```bash
# scrcpy installed via Homebrew already adds ADB to PATH
which adb  # Should show /opt/homebrew/bin/adb
```

### Platform-Specific Performance

**Windows**:
- Best with NVIDIA GPU (NVDEC)
- Intel QSV also excellent
- Lowest latency: 30-50ms

**Linux**:
- VAAPI works well on AMD/Intel
- NVDEC on NVIDIA
- Slightly higher latency: 40-60ms

**macOS**:
- Currently software decoding
- Higher latency: 60-80ms
- Future: VideoToolbox integration

---

## Troubleshooting by Platform

### Windows-specific Issues

**"VCRUNTIME140.dll not found"**:
```powershell
# Install Visual C++ Redistributable
winget install Microsoft.VCRedist.2015+.x64
```

### Linux-specific Issues

**"cannot find -lva"**:
```bash
sudo apt install libva-dev
```

**Permission denied (ADB)**:
```bash
# Add udev rules for Android devices
sudo wget -O /etc/udev/rules.d/51-android.rules https://raw.githubusercontent.com/M0Rf30/android-udev-rules/main/51-android.rules
sudo udevadm control --reload-rules
```

### macOS-specific Issues

**"xcrun: error"**:
```bash
# Install Xcode Command Line Tools
xcode-select --install
```

**Gatekeeper blocking binary**:
```bash
# Allow binary to run
xattr -d com.apple.quarantine target/release/scrcpy-custom
```

---

## Building for Release (All Platforms)

```bash
# Optimized release build
cargo build --release --target-dir ./target

# Strip debug symbols (Linux/macOS)
strip target/release/scrcpy-custom

# Create distributable (example)
tar -czf scrcpy-custom-$(uname -s)-$(uname -m).tar.gz \
    -C target/release scrcpy-custom
```

---

## Integration with scrcpy

### Shared ADB Connection

Both scrcpy and scrcpy-custom can share the same ADB connection:

```bash
# Start scrcpy (if needed)
scrcpy &

# Use same ADB for our app
adb forward tcp:5555 tcp:5555
cargo run --release -- --mode tcp

# Both can run simultaneously on different ports
```

### Using scrcpy's Server APK (Future)

Future versions may support using scrcpy's server APK directly, which would eliminate the need for custom Android server development.

---

## Next Steps

1. **Install on your platform** following instructions above
2. **Test basic connection** using USB
3. **Try WiFi connection** if supported
4. **Optimize settings** for your hardware
