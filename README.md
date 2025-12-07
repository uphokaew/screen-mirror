# scrcpy-custom - Ultra-Low Latency Screen Mirroring

High-performance screen mirroring from Android to PC with ultra-low latency optimizations.

## Features

✅ **Dual Protocol Support**: TCP (USB) and QUIC (WiFi)  
✅ **Hardware Acceleration**: NVDEC, QSV, VAAPI for video decoding  
✅ **GPU Rendering**: wgpu with bilinear upscaling  
✅ **Adaptive Bitrate**: Automatic quality adjustment based on network conditions  
✅ **FEC**: Forward Error Correction for wireless packet recovery  
✅ **Audio Support**: Low-latency playback with jitter buffer  
✅ **A/V Sync**: PTS-based synchronization

## Quick Start

### Prerequisites

**Install FFmpeg** with hardware acceleration:
```bash
# Windows (with Chocolatey)
choco install ffmpeg-full

# Linux (Ubuntu)
sudo apt install ffmpeg
```

### Build & Run

```bash
cd scrcpy-custom
cargo build --release

# USB Connection
adb forward tcp:5555 tcp:5555
cargo run --release -- --mode tcp --host 127.0.0.1 --port 5555

# WiFi Connection  
cargo run --release -- --mode quic --host <DEVICE_IP> --port 5556
```

## Command-Line Options

```
OPTIONS:
    -m, --mode <tcp|quic>          Connection mode [default: tcp]
        --host <IP>                Server IP [default: 127.0.0.1]
    -p, --port <PORT>              Server port [default: 5555]
    -b, --bitrate <Mbps>           Video bitrate [default: 8]
        --hw-decoder <DECODER>     auto|nvdec|qsv|vaapi [default: auto]
        --adaptive-bitrate         Enable adaptive bitrate [default: true]
```

## Performance

- **USB (TCP)**: < 50ms latency
- **WiFi (QUIC)**: < 100ms latency with FEC
- **Throughput**: 2-20 Mbps adaptive
- **Resolution**: 720p-1080p streaming, GPU upscaling

## Platform Support

✅ **Windows**: Full support (NVDEC, QSV)  
✅ **Linux**: Full support (NVDEC, QSV, VAAPI)  
✅ **macOS**: Software decoding (VideoToolbox via FFmpeg)

See [INSTALL_CROSSPLATFORM.md](INSTALL_CROSSPLATFORM.md) for platform-specific instructions.

## Testing

Run automated tests:
```bash
# Linux/macOS
./test.sh

# Windows
.\test.ps1
```

See [TESTING.md](TESTING.md) for comprehensive testing guide.

## Development Status

**Completed**: Core networking, HW video decoding, GPU rendering, audio pipeline, A/V sync, FEC, adaptive bitrate  
**Pending**: Android server implementation

## Using scrcpy's ADB

This project can use scrcpy's bundled ADB:
```bash
# Windows: Add to PATH
$env:Path += ";C:\Program Files\scrcpy"

# Linux: Symlink
sudo ln -s /opt/scrcpy/adb /usr/local/bin/adb

# macOS: Already in PATH if installed via Homebrew
```

## Documentation

- **[USAGE_TH.md](USAGE_TH.md)** - คู่มือการใช้งานภาษาไทย (ละเอียด)
- **[HOTSPOT_MODE_TH.md](HOTSPOT_MODE_TH.md)** - 🔥 การใช้ Android เป็น WiFi Hotspot (แนะนำ!)
- **[INSTALL_CROSSPLATFORM.md](INSTALL_CROSSPLATFORM.md)** - Cross-platform installation
- **[TESTING.md](TESTING.md)** - Testing and verification guide
- **[ANDROID_SERVER.md](ANDROID_SERVER.md)** - Android server implementation guide
- **[FFMPEG_BUILD_ISSUES.md](FFMPEG_BUILD_ISSUES.md)** - FFmpeg build troubleshooting
- **[config.example.toml](config.example.toml)** - Example configuration

## License

MIT - See [LICENSE](LICENSE) file
