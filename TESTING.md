# Testing & Verification Guide

## Overview

This document covers testing strategies for scrcpy-custom at different levels.

## Test Levels

### 1. Unit Tests (✅ Implemented)

Tests individual components in isolation.

**Run all unit tests**:
```bash
cargo test --lib
```

**Test specific modules**:
```bash
# Network module
cargo test --lib network

# Video decoder
cargo test --lib video

# Audio player
cargo test --lib audio

# Sync engine
cargo test --lib sync
```

**With verbose output**:
```bash
cargo test --lib -- --nocapture
```

### 2. Integration Tests

Tests component interaction without Android device.

**Automated test suite**:

**Linux/macOS**:
```bash
chmod +x test.sh
./test.sh
```

**Windows**:
```powershell
.\test.ps1
```

**Test coverage includes**:
- ✅ Rust toolchain verification
- ✅ FFmpeg installation
- ✅ ADB availability
- ✅ Project structure
- ✅ Compilation
- ✅ Unit tests
- ✅ Build system
- ✅ Hardware acceleration detection

### 3. Mock Server Tests

Tests end-to-end flow with simulated Android server.

```bash
chmod +x integration_test.sh
./integration_test.sh
```

This will:
1. Build release binary
2. Start Python mock server (sends fake H.264 packets)
3. Connect client
4. Verify packet reception
5. Cleanup

### 4. Real Device Tests

Tests with actual Android device.

#### Test 1: USB Connection (TCP)

```bash
# 1. Connect Android device
adb devices

# 2. Forward port
adb forward tcp:5555 tcp:5555

# 3. Run client  
cargo run --release -- --mode tcp --host 127.0.0.1 --port 5555

# Expected output:
# [INFO] Connected successfully!
# [INFO] Using hardware decoder: h264_cuvid
# [INFO] Received video packet: seq=1, pts=16666, size=1234 bytes
```

**Success criteria**:
- ✅ Connection established < 2 seconds
- ✅ Hardware decoder detected
- ✅ Video packets received
- ✅ FPS > 55
- ✅ Latency < 50ms

#### Test 2: WiFi Connection (QUIC)

```bash
# 1. Get device IP
adb shell ip addr show wlan0 | grep "inet "

# 2. Run client
cargo run --release -- --mode quic --host 192.168.1.100 --port 5556

# Expected output:
# [INFO] Using QUIC connection (wireless/WiFi)
# [INFO] Connected successfully!
# [INFO] FEC enabled with 10% redundancy
```

**Success criteria**:
- ✅ Connection established < 3 seconds
- ✅ QUIC protocol active
- ✅ FEC recovery working
- ✅ Adaptive bitrate functional
- ✅ Latency < 100ms

#### Test 3: Adaptive Bitrate

```bash
# Start with high bitrate
cargo run --release -- --mode quic --bitrate 12 --adaptive-bitrate

# Monitor logs for bitrate adjustments
# Expected: Bitrate decreases when packet loss increases
```

Simulate network degradation:
```bash
# Linux: Use tc (traffic control)
sudo tc qdisc add dev wlan0 root netem delay 100ms loss 5%

# Observe: Bitrate should drop automatically
# Clean up:
sudo tc qdisc del dev wlan0 root
```

#### Test 4: Hardware Acceleration Verification

```bash
# NVIDIA (NVDEC)
cargo run --release -- --hw-decoder nvdec

# Intel (QSV)
cargo run --release -- --hw-decoder qsv

# AMD/Intel Linux (VAAPI)
cargo run --release -- --hw-decoder vaapi

# Check logs for confirmation:
# [INFO] Using hardware decoder: h264_cuvid  # NVDEC
# [INFO] Using hardware decoder: h264_qsv    # QSV
# [INFO] Using hardware decoder: h264_vaapi  # VAAPI
```

#### Test 5: Audio/Video Sync

```bash
# Run and monitor sync stats
cargo run --release -- --mode tcp

# Check logs every second:
# Drift: 2ms | Dropped: 0
# Drift should stay < 50ms
# Dropped frames should be minimal
```

### 5. Performance Tests

#### Latency Measurement

**Method 1: Visual (Camera)**
1. Display clock on Android
2. Record both Android and PC screen with high-speed camera
3. Compare timestamps frame-by-frame

**Method 2: Audio Click Test**
1. Play click sound on Android
2. Measure time until heard on PC
3. Repeat 10 times, average results

**Expected latency**:
- USB (TCP): 30-50ms
- WiFi (QUIC): 60-100ms
- Software decoder: +20ms

#### FPS Test

Monitor FPS in logs:
```
FPS: 60.0 | Latency: 45.2ms | ...
```

Target: Stable 60 FPS with < 1% variance

#### CPU/GPU Usage

**Linux**:
```bash
# Monitor during playback
htop
nvidia-smi  # For NVIDIA GPUs
```

**Windows**:
- Task Manager → Performance
- GPU usage should be 10-30%
- CPU should be < 20% with hardware accel

**macOS**:
```bash
# Activity Monitor
top -l 1 | grep scrcpy-custom
```

### 6. Stress Tests

#### Network Stress Test

```bash
# High packet loss scenario
sudo tc qdisc add dev wlan0 root netem loss 10%

# Run client
cargo run --release -- --mode quic

# Verify:
# - FEC recovers lost packets
# - Adaptive bitrate reduces quality
# - Stream remains stable

# Cleanup
sudo tc qdisc del dev wlan0 root
```

####

 Long Duration Test

```bash
# Run for 1 hour
timeout 3600 cargo run --release -- --mode tcp

# Monitor:
# - Memory usage (should be stable)
# - No crashes
# - No significant performance degradation
```

## Automated Testing with CI/CD

### GitHub Actions Example

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Install FFmpeg (Ubuntu)
        if: matrix.os == 'ubuntu-latest'
        run: sudo apt-get install -y ffmpeg libva-dev
      
      - name: Install FFmpeg (macOS)
        if: matrix.os == 'macos-latest'
        run: brew install ffmpeg
      
      - name: Install FFmpeg (Windows)
        if: matrix.os == 'windows-latest'
        run: choco install ffmpeg
      
      - name: Run tests
        run: cargo test --verbose
      
      - name: Build
        run: cargo build --release
```

## Test Results Checklist

Before considering the project "complete":

- [ ] All unit tests pass on all platforms
- [ ] Integration tests pass
- [ ] USB connection works reliably
- [ ] WiFi connection works with FEC
- [ ] Hardware acceleration verified on target GPUs
- [ ] Latency meets targets (< 50ms USB, < 100ms WiFi)
- [ ] No memory leaks in 1-hour test
- [ ] Adaptive bitrate adjusts correctly
- [ ] Audio/video sync stays within 50ms
- [ ] Works with scrcpy's ADB

## Known Limitations

- ⚠️ Requires Android server (not yet implemented)
- ⚠️ macOS hardware decoding via FFmpeg only (not native VideoToolbox)
- ⚠️ QUIC certificate validation is basic (self-signed for dev)

## Reporting Issues

When reporting issues, include:
1. Platform (OS, version)
2. GPU model and driver version
3. FFmpeg version (`ffmpeg -version`)
4. Log output with `RUST_LOG=debug`
5. Connection mode (TCP/QUIC)
6. Network conditions (if WiFi)

Example:
```bash
RUST_LOG=debug cargo run --release -- --mode tcp > debug.log 2>&1
```

---

**Test Status**: ✅ All automated tests implemented and passing  
**Manual Tests**: ⏳ Pending Android server implementation
