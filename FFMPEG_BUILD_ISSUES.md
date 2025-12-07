# FFmpeg Installation & Build Issues - Troubleshooting Guide

## Problem: `failed to run custom build command for ffmpeg-sys-next`

### Cause
The `ffmpeg-sys-next` crate requires FFmpeg development libraries to compile. On Windows, these are often not available by default.

---

## Solution 1: Use vcpkg (Recommended for Windows)

vcpkg is the easiest way to install FFmpeg libraries on Windows.

### Step 1: Install vcpkg

```powershell
# Clone vcpkg
git clone https://github.com/Microsoft/vcpkg.git C:\vcpkg
cd C:\vcpkg

# Bootstrap
.\bootstrap-vcpkg.bat

# Add to PATH (optional)
$env:PATH += ";C:\vcpkg"
```

### Step 2: Install FFmpeg

```powershell
# Install FFmpeg with all features
.\vcpkg install ffmpeg:x64-windows

# This will take 10-30 minutes
```

### Step 3: Set environment variables

```powershell
# Add to your PowerShell profile or set temporarily
$env:FFMPEG_DIR = "C:\vcpkg\installed\x64-windows"
$env:PKG_CONFIG_PATH = "C:\vcpkg\installed\x64-windows\lib\pkgconfig"

# Install pkg-config for Windows
.\vcpkg install pkgconfig:x64-windows
```

### Step 4: Build project

```powershell
cd C:\Users\beok1\Desktop\scrcpy-custom
cargo clean
cargo build --release
```

---

## Solution 2: Pre-built FFmpeg Binaries (Faster & Recommended)

> [!IMPORTANT]
> **You MUST use FFmpeg 6.0**. Newer versions (7.x) are NOT compatible with the current Rust libraries.

### Step 1: Download FFmpeg 6.0

1. Go to [FFmpeg-Builds Releases](https://github.com/GyanD/codexffmpeg/releases) or use direct link:
   - [ffmpeg-n6.0-latest-win64-gpl-shared-6.0.zip](https://github.com/GyanD/codexffmpeg/releases/download/6.1/ffmpeg-6.1-full_build-shared.7z)
2. Extract the contents.
3. Move the extracted files to `C:\ffmpeg` so that `C:\ffmpeg\bin` exists.

### Step 2: Set environment variables

Run in PowerShell (Admin recommended for permanent setup):

```powershell
$env:FFMPEG_DIR = "C:\ffmpeg"
$env:PATH += ";C:\ffmpeg\bin"

# Permanently:
[System.Environment]::SetEnvironmentVariable("FFMPEG_DIR", "C:\ffmpeg", "Machine")
```

### Step 3: Generate pkg-config files

We have provided a script to automate this.

```powershell
# Run the setup script
powershell -ExecutionPolicy Bypass -File scripts/setup_ffmpeg_pc.ps1
```

### Step 4: Install LLVM (Required for bindgen)

```powershell
choco install llvm -y
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

### Step 5: Build

```powershell
cargo clean
cargo build --release
```
```

### Step 5: Build

```powershell
$env:PKG_CONFIG_PATH = "C:\ffmpeg\lib\pkgconfig"
cargo clean
cargo build --release
```

---

## Solution 3: Use Static FFmpeg Feature (Simplest)

Modify `Cargo.toml` to use static FFmpeg bundled with the crate:

```toml
[dependencies]
# Change this:
# ffmpeg-next = "6.1"

# To this:
ffmpeg-next = { version = "6.1", features = ["static"] }
```

**Pros**: No external dependencies needed  
**Cons**: Longer compile time, larger binary size, no hardware acceleration

Then build:
```powershell
cargo clean
cargo build --release
```

---

## Solution 4: Use Bindgen Fallback (Last Resort)

If all else fails, use pre-generated bindings:

```toml
[dependencies]
ffmpeg-next = { version = "6.1", default-features = false, features = ["codec", "format", "software-scaling"] }
```

---

## Linux Solution

```bash
# Ubuntu/Debian
sudo apt install -y \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libswscale-dev \
    libavfilter-dev \
    pkg-config

# Then build
cargo build --release
```

## macOS Solution

```bash
# Install via Homebrew
brew install ffmpeg pkg-config

# Set PKG_CONFIG_PATH if needed
export PKG_CONFIG_PATH="/opt/homebrew/lib/pkgconfig"

# Then build
cargo build --release
```

---

## Quick Fix for Current Issue

Try this immediately:

```powershell
# Option A: Use static feature (fastest to try)
# Edit Cargo.toml and change ffmpeg-next line to:
# ffmpeg-next = { version = "6.1", features = ["static"] }

# Then:
cargo clean
cargo build --release

# Option B: Install via vcpkg (recommended for production)
# Follow Solution 1 above
```

---

## Verification

After successful build, verify FFmpeg:

```powershell
# Check binary exists
ls target\release\scrcpy-custom.exe

# Run help
.\target\release\scrcpy-custom.exe --help

# Check FFmpeg libraries (if using dynamic linking)
dumpbin /dependents target\release\scrcpy-custom.exe | findstr av
```

---

## Additional Notes

### Hardware Acceleration

- **Static build**: No hardware acceleration (NVDEC/QSV unavailable)
- **Dynamic build with libs**: Full hardware acceleration support
- **Recommended**: Use vcpkg or pre-built binaries for best performance

### Build Time

- **Static**: 10-20 minutes first build
- **Dynamic**: 2-5 minutes with libs installed

---

## Still Having Issues?

1. Check Rust version: `rustc --version` (need 1.70+)
2. Check Visual Studio Build Tools installed
3. Try: `cargo clean` then `cargo build --release --verbose` to see detailed errors
4. Enable logging: `$env:RUST_LOG="debug"; cargo build`

Contact with full error output from verbose build.
