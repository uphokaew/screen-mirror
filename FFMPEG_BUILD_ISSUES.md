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

## Solution 2: Pre-built FFmpeg Binaries (Faster)

If you want to skip vcpkg compilation:

### Step 1: Download FFmpeg Development Files

1. Go to https://github.com/BtbN/FFmpeg-Builds/releases
2. Download: `ffmpeg-n6.0-latest-win64-gpl-shared-6.0.zip`
3. Extract to `C:\ffmpeg`

### Step 2: Set environment variables

```powershell
$env:FFMPEG_DIR = "C:\ffmpeg"
$env:PATH += ";C:\ffmpeg\bin"

# Permanently (run as admin):
[System.Environment]::SetEnvironmentVariable("FFMPEG_DIR", "C:\ffmpeg", "Machine")
[System.Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";C:\ffmpeg\bin", "Machine")
```

### Step 3: Create pkg-config files manually

Create `C:\ffmpeg\lib\pkgconfig\libavcodec.pc`:
```
prefix=C:/ffmpeg
exec_prefix=${prefix}
libdir=${prefix}/lib
includedir=${prefix}/include

Name: libavcodec
Description: FFmpeg codec library
Version: 6.0
Libs: -L${libdir} -lavcodec
Cflags: -I${includedir}
```

Repeat for: `libavformat.pc`, `libavutil.pc`, `libswscale.pc`, `libavfilter.pc`, `libavdevice.pc`

### Step 4: Install pkg-config for Windows

```powershell
choco install pkgconfiglite
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
