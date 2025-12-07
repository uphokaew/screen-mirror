# Android Server Implementation Guide

This document outlines how to build the Android server component for scrcpy-custom.

## Overview

The Android server is responsible for:
1. Screen capture using MediaProjection API
2. Hardware encoding with MediaCodec
3. Network streaming (TCP/QUIC)
4. Receiving control commands from PC client

## Architecture

```
┌─────────────────────────────────────┐
│      Android Application            │
│                                     │
│  ┌──────────────────────────────┐  │
│  │   MediaProjection Service    │  │
│  │   - Screen capture           │  │
│  │   - Create virtual display   │  │
│  └───────────┬──────────────────┘  │
│              ↓                      │
│  ┌──────────────────────────────┐  │
│  │   MediaCodec Encoder         │  │
│  │   - Hardware H.264/H.265     │  │
│  │   - Zero-copy pipeline       │  │
│  └───────────┬──────────────────┘  │
│              ↓                      │
│  ┌──────────────────────────────┐  │
│  │   Network Streamer           │  │
│  │   - TCP/QUIC sockets         │  │
│  │   - Packet framing           │  │
│  └──────────────────────────────┘  │
└─────────────────────────────────────┘
```

## Required Components

### 1. MediaProjection Service

```java
public class ScreenCaptureService extends Service {
    private MediaProjection mediaProjection;
    private VirtualDisplay virtualDisplay;
    private MediaCodec encoder;
    
    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        // Get MediaProjection from Intent
        mediaProjection = mediaProjectionManager.getMediaProjection(
            resultCode, data);
        
        // Create virtual display
        virtualDisplay = mediaProjection.createVirtualDisplay(
            "scrcpy-capture",
            width, height, dpi,
            DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
            encoderSurface,  // Surface from MediaCodec
            null, null
        );
        
        return START_STICKY;
    }
}
```

### 2. Hardware Encoder Selection

```java
public class EncoderFactory {
    public static MediaCodec createHardwareEncoder() {
        // Prefer hardware encoders
        String[] preferredEncoders = {
            "c2.qti.avc.encoder",      // Qualcomm
            "OMX.Exynos.AVC.Encoder",  // Samsung
            "OMX.qcom.video.encoder.avc",
            "OMX.MTK.VIDEO.ENCODER.AVC" // MediaTek
        };
        
        for (String codecName : preferredEncoders) {
            try {
                MediaCodec codec = MediaCodec.createByCodecName(codecName);
                Log.i(TAG, "Using encoder: " + codecName);
                return codec;
            } catch (IOException e) {
                // Try next
            }
        }
        
        // Fallback to default
        return MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC);
    }
}
```

### 3. Encoder Configuration

```java
MediaFormat format = MediaFormat.createVideoFormat(
    MediaFormat.MIMETYPE_VIDEO_AVC, width, height);

format.setInteger(MediaFormat.KEY_COLOR_FORMAT,
    MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface);
format.setInteger(MediaFormat.KEY_BIT_RATE, bitrate * 1_000_000);
format.setInteger(MediaFormat.KEY_FRAME_RATE, 60);
format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 1);

// Low latency settings
format.setInteger(MediaFormat.KEY_LATENCY, 0);
format.setInteger(MediaFormat.KEY_PRIORITY, 0); // Realtime

encoder.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
Surface surface = encoder.createInputSurface();
encoder.start();
```

### 4. Network Streamer

```java
public class NetworkStreamer {
    private Socket tcpSocket;  // or QUIC connection
    private DataOutputStream output;
    
    public void sendVideoPacket(ByteBuffer data, long pts, int flags) {
        // Create packet header
        byte[] header = new byte[17];
        header[0] = PACKET_TYPE_VIDEO;
        writeLong(header, 1, pts);
        writeInt(header, 9, sequenceNumber++);
        writeInt(header, 13, data.remaining());
        
        // Send header + data
        output.write(header);
        output.write(data.array(), data.position(), data.remaining());
        output.flush();
    }
}
```

### 5. Control Message Handler

```java
public class ControlHandler extends Thread {
    @Override
    public void run() {
        while (running) {
            // Read control message
            byte[] header = new byte[17];
            input.readFully(header);
            
            if (header[0] == PACKET_TYPE_CONTROL) {
                int length = readInt(header, 13);
                byte[] data = new byte[length];
                input.readFully(data);
                
                // Deserialize control message
                ControlMessage msg = deserialize(data);
                
                switch (msg.type) {
                    case SET_BITRATE:
                        updateEncoderBitrate(msg.value);
                        break;
                    case REQUEST_KEYFRAME:
                        requestKeyframe();
                        break;
                }
            }
        }
    }
    
    private void updateEncoderBitrate(int newBitrate) {
        Bundle params = new Bundle();
        params.putInt(MediaCodec.PARAMETER_KEY_VIDEO_BITRATE, 
                     newBitrate * 1_000_000);
        encoder.setParameters(params);
    }
}
```

## Protocol Implementation

### Packet Format

Must match the Rust client protocol (see `src/network/protocol.rs`):

```
Packet Header (17 bytes):
- Type (1 byte): 0x01=Video, 0x02=Audio, 0x03=Control
- PTS (8 bytes): Presentation timestamp (microseconds, little-endian)
- Sequence (4 bytes): Packet sequence number (little-endian)
- Length (4 bytes): Payload length (little-endian)

Payload:
- Raw H.264/H.265 NALUs for video
- Raw AAC/Opus frames for audio
- Serialized ControlMessage for control
```

### Example Video Packet

```java
// From MediaCodec.dequeueOutputBuffer()
MediaCodec.BufferInfo bufferInfo = new MediaCodec.BufferInfo();
int outputBufferId = encoder.dequeueOutputBuffer(bufferInfo, timeout);

if (outputBufferId >= 0) {
    ByteBuffer outputBuffer = encoder.getOutputBuffer(outputBufferId);
    
    // Extract PTS
    long pts = bufferInfo.presentationTimeUs;
    
    // Check if keyframe
    boolean isKeyframe = (bufferInfo.flags & 
        MediaCodec.BUFFER_FLAG_KEY_FRAME) != 0;
    
    // Send packet
    streamer.sendVideoPacket(outputBuffer, pts, bufferInfo.flags);
    
    encoder.releaseOutputBuffer(outputBufferId, false);
}
```

## Build Instructions

1. **Create Android Studio Project**:
   - Min SDK: 21 (Android 5.0)
   - Target SDK: 33 (Android 13)

2. **Add Permissions** (AndroidManifest.xml):
```xml
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
```

3. **Dependencies** (build.gradle):
```gradle
dependencies {
    implementation 'androidx.appcompat:appcompat:1.6.1'
    // For QUIC (optional)
    implementation 'io.netty:netty-all:4.1.100.Final'
}
```

4. **Build**:
```bash
./gradlew assembleRelease
```

5. **Install**:
```bash
adb install app/build/outputs/apk/release/app-release.apk
```

## Testing

1. Start Android server app
2. Grant MediaProjection permission
3. Connect via PC client:
```powershell
cargo run --release -- --mode tcp --host <device-ip> --port 5555
```

## Performance Tips

1. **Zero-Copy**: Use Surface input directly, avoid CPU copies
2. **Hardware Encoder**: Always prefer hardware over software
3. **Bitrate Control**: Implement dynamic bitrate adjustment
4. **Thread Priority**: Set encoder thread to PRIORITY_URGENT_AUDIO
5. **Buffer Size**: Use small buffers to minimize latency

## References

- [MediaProjection Documentation](https://developer.android.com/reference/android/media/projection/MediaProjection)
- [MediaCodec Best Practices](https://developer.android.com/reference/android/media/MediaCodec)
- Client Protocol: `src/network/protocol.rs`

---

**Note**: This is a guide. Actual implementation will vary based on Android version and device capabilities.
