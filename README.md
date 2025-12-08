# scrcpy-custom

A high-performance, Rust-based scrcpy client with custom UI, low-latency audio, and wireless hotspot support.

**Select Language / เลือกภาษา**:
- [🇺🇸 English Documentation](#english-documentation)
- [🇹🇭 เอกสารภาษาไทย (Thai Documentation)](#เอกสารภาษาไทย-thai-documentation)

---

<a name="english-documentation"></a>
## 🇺🇸 English Documentation

### 1. Installation

**Requirements**:
- **Windows 10/11**
- **Rust 1.70+**: [Install Rust](https://rustup.rs/)
- **FFmpeg**: Must be in PATH. (Install via `choco install ffmpeg-full` or download from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/))
- **ADB**: Must be in PATH. (Install via `choco install adb` or SDK Platform Tools)

**Build**:
```powershell
# In the project directory
cargo build --release
```

**Assets**:
Ensure `adb.exe` and `scrcpy-server` (jar) are in the same folder as the executable or in `bin/` / `assets/`.

### 2. Basic Usage

**Run (Default Interactive Mode)**:
```powershell
cargo run --release
# Or if built:
./target/release/scrcpy-custom.exe
```
This will open a menu to verify dependencies and choose connection mode.

**USB Mode (Lowest Latency)**:
```powershell
# 1. Connect Android via USB
# 2. Forward port
adb forward tcp:5555 tcp:5555
# 3. Run
cargo run --release -- --mode tcp --host 127.0.0.1 --port 5555
```

**Wireless Mode (WiFi)**:
```powershell
# 1. Get Android IP (Settings -> About -> Status)
# 2. Run (Replace IP)
cargo run --release -- --mode quic --host 192.168.1.100
```

### 3. Hotspot Mode (Direct Connection) 🔥
Recommended for locations without WiFi routers or for lower latency than a busy router.

**Steps**:
1.  **Android**: Enable **WiFi Hotspot**.
    - Tips: Use **5GHz** band for best speed. Name it "AndroidMirror".
2.  **PC**: Connect WiFi to "AndroidMirror".
3.  **Run**:
    - Android IP is usually `192.168.43.1`.
    ```powershell
    cargo run --release -- --mode quic --host 192.168.43.1 --port 5556
    ```

### 4. Audio & OBS Support
- **Audio**: Sound plays on PC automatically.
    - Uses **Opus** (Low Latency) by default. Falls back to **AAC** if needed.
    - Android sound is muted to prevent echo.
- **OBS Studio**:
    - Source: **Window Capture**
    - Window: `[scrcpy-custom]: scrcpy-custom`
    - Audio: Capture **Desktop Audio** to hear game sound.

### 5. Troubleshooting
- **No Audio**: Ensure Android 11+. Check PC volume.
- **Lag**: Use USB. If wireless, use 5GHz Hotspot. Reduce bitrate (`--bitrate 4`).
- **Connection Refused**: Check `adb devices`. Ensure `adb forward` command was run for USB mode.

---

<a name="เอกสารภาษาไทย-thai-documentation"></a>
## 🇹🇭 เอกสารภาษาไทย (Thai Documentation)

### 1. การติดตั้ง

**สิ่งที่ต้องมี**:
- **Windows 10/11**
- **Rust**: [ดาวน์โหลด](https://rustup.rs/)
- **FFmpeg**: ต้องติดตั้งและเรียกใช้ได้ผ่าน CMD (`ffmpeg -version`). แนะนำให้ติดตั้งผ่าน Chocolatey: `choco install ffmpeg-full`
- **ADB**: ต้องติดตั้งเพื่อเชื่อมต่อมือถือ (`choco install adb`)

**การสร้างโปรแกรม (Build)**:
```powershell
cd C:\Users\beok1\Desktop\scrcpy-custom
cargo build --release
```

### 2. การใช้งานเบื้องต้น

**รันโปรแกรม (โหมดเมนู)**:
```powershell
cargo run --release
```
โปรแกรมจะตรวจสอบความพร้อมและให้ท่านเลือกโหมดการเชื่อมต่อ

**โหมดสาย USB (ความหน่วงต่ำที่สุด)**:
1. เสียบสาย USB, เปิด USB Debugging บนมือถือ
2. พิมพ์คำสั่งเตรียมการ:
   ```powershell
   adb forward tcp:5555 tcp:5555
   ```
3. รันโปรแกรม:
   ```powershell
   cargo run --release -- --mode tcp --host 127.0.0.1 --port 5555
   ```

**โหมดไร้สาย (WiFi)**:
1. ดู IP ของมือถือ (Settings -> About phone -> Status -> IP address)
2. รันโปรแกรม (ใส่ IP ของท่าน):
   ```powershell
   cargo run --release -- --mode quic --host 192.168.1.100
   ```

### 3. โหมด Hotspot (แนะนำสำหรับไร้สาย) 🔥
เหมาะสำหรับที่ที่ไม่มี Router หรือต้องการความเร็วสูงกว่าผ่าน Router บ้าน

**ขั้นตอน**:
1.  **บนมือถือ**: เปิด **WiFi Hotspot**
    - *แนะนำ*: ตั้งค่าเป็นคลื่น **5GHz** เพื่อความเร็วสูงสุด
2.  **บนคอมพิวเตอร์**: เชื่อมต่อ WiFi ไปยังชื่อ Hotspot ของมือถือ
3.  **รันโปรแกรม**:
    - IP ปกติของ Android Hotspot คือ `192.168.43.1`
    ```powershell
    cargo run --release -- --mode quic --host 192.168.43.1 --port 5556
    ```

### 4. ระบบเสียงและการใช้งานกับ OBS
- **เสียง (Audio)**: เสียงจากมือถือจะดังที่คอมพิวเตอร์อัตโนมัติ
    - ใช้ระบบ **Opus** เพื่อความหน่วงต่ำ (Low Latency)
    - เสียงที่มือถือจะถูกปิดเพื่อป้องกันเสียงสะ้อน
- **OBS Studio**:
    - เพิ่ม Source แบบ **Window Capture**
    - เลือกหน้าต่าง: `[scrcpy-custom]: scrcpy-custom`
    - เสียง: ใช้ **Desktop Audio** ของ OBS เพื่อดึงเสียงเกม

### 5. การแก้ไขปัญหา (Troubleshooting)
- **ไม่มีเสียง**: ต้องใช้ Android 11 ขึ้นไป
- **ภาพกระตุก**:
    - หากใช้ WiFi: ให้ลองลด Bitrate (`--bitrate 4`) หรือเปลี่ยนไปใช้ USB
    - หากใช้ USB: ตรวจสอบว่าเป็นช่อง USB 3.0
- **เชื่อมต่อไม่ได้**:
    - ตรวจสอบ `adb devices` ว่าขึ้นหรือไม่
    - ตรวจสอบ `adb forward --list` ว่ามีการตั้งค่า Port หรือยัง

### 6. ข้อมูลจำเพาะโปรเจค (Specifications)
- **ภาษา**: Rust (High Performance & Safety)
- **Video Decoder**: FFmpeg (Hardware Acceleration: NVDEC/QSV)
- **Audio Decoder**: Audiopus (Opus) / Symphonia (AAC/MP3)
- **Network Protocol**:
    - **TCP**: สำหรับ USB (Reliable)
    - **QUIC**: สำหรับ Wireless (Low Latency / Packet Loss Tolerant)
- **Features**: Hotspot Optimization, Custom Jitter Buffer, Dynamic Bitrate.

---
**Version**: 0.1.0 | **Updated**: 2025-12-08
