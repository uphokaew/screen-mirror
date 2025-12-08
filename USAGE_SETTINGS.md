# การตั้งค่าและการใช้งาน (Settings & Usage)

โปรเจกต์นี้ได้รับการปรับแต่งให้เน้น **ประสิทธิภาพสูงสุด (Max Performance)** และ **ลดภาระเครื่อง (Low Overhead)** โดยตัดส่วนที่ไม่จำเป็นออก (เช่น การควบคุม Input) และเน้นการแสดงผลภาพและเสียงเท่านั้น

## ตารางการเปรียบเทียบ (Overview)

| ฟีเจอร์ | สถานะ | หมายเหตุ |
| :--- | :--- | :--- |
| **Video** | ✅ เปิดใช้งาน | รองรับ Hardware Decode |
| **Audio** | ✅ เปิดใช้งาน | รองรับ Opus/AAC |
| **Control** | ❌ ปิดถาวร | ไม่สามารถคลิกหรือพิมพ์ได้ (View Only) |
| **Adaptive Bitrate** | ❌ ปิดการทำงาน | ใช้ Fixed Bitrate เพื่อความเสถียร |

## วิธีการใช้งาน (Usage)

รันโปรแกรมผ่าน Command Line ด้วยคำสั่ง `cargo run` พร้อมระบุพารามิเตอร์ที่ต้องการ

### คำสั่งพื้นฐาน (Basic Command)

```powershell
# รันด้วยการตั้งค่าเริ่มต้น (TCP, 8Mbps, 1080p, Audio Enabled)
cargo run --release
```

### การตั้งค่าทั้งหมด (All Settings)

| พารามิเตอร์ | ค่าเริ่มต้น | คำอธิบาย | ตัวอย่าง |
| :--- | :--- | :--- | :--- |
| `--mode` | `tcp` | โหมดการเชื่อมต่อ (`tcp` หรือ `quic`) | `--mode quic` |
| `--host` | `127.0.0.1` | IP ของเครื่อง Android (ถ้าต่อสาย USB ไม่ต้องแก้) | `--host 192.168.1.50` |
| `--port` | `5555` | พอร์ตการเชื่อมต่อ | `--port 5555` |
| `--bitrate` | `8` | บิตเรตวิดีโอ (Mbps) ยิ่งมากภาพยิ่งชัดแต่กินเน็ต | `--bitrate 16` |
| `--max-size` | `0` (Native) | จำกัดความละเอียดวิดีโอ (เช่น 720, 1080) | `--max-size 1024` |
| `--no-audio` | `false` | ปิดเสียง (ถ้าต้องการภาพอย่างเดียว) | `--no-audio` |
| `--hw-decoder` | `auto` | เลือกตัวถอดรหัส (`auto`, `nvdec`, `qsv`, `vaapi`) | `--hw-decoder nvdec` |
| `--hw-accel` | `true` | เปิด/ปิด Hardware Acceleration | `--hw-accel false` |

### ตัวอย่างการใช้งาน (Examples)

**1. เน้นลื่นไหล ลดความละเอียด (Low Latency / Performance)**
```powershell
cargo run --release -- --max-size 720 --bitrate 4
```

**2. เน้นภาพชัด (High Quality)**
```powershell
cargo run --release -- --bitrate 16 --hw-decoder nvdec
```

**3. เชื่อมต่อไร้สายผ่าน QUIC (Experimental)**
```powershell
cargo run --release -- --mode quic --host <IP_ADDRESS>
```

## การแก้ปัญหาเบื้องต้น (Troubleshooting)

- **Error: Connection refused / 10061**:
  - ตรวจสอบว่าเสียบสาย USB และเปิด USB Debugging แล้วหรือยัง
  - ลองรันคำสั่ง `adb devices` เพื่อเช็คสถานะ

- **ภาพไม่ขึ้น**:
  - ลองลด `--max-size` หรือเปลี่ยน `--hw-decoder`

- **เสียงไม่ออก**:
  - ตรวจสอบว่า Android เป็น Android 11 ขึ้นไป (ถ้ารุ่นเก่าอาจไม่รองรับ Audio)
