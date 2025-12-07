package com.scrcpy.custom.server.encoder;

import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.media.MediaFormat;
import android.util.Log;
import android.view.Surface;

import java.io.IOException;
import java.nio.ByteBuffer;

/**
 * Hardware H.264 video encoder
 */
public class VideoEncoder {
    private static final String TAG = "VideoEncoder";
    private static final String MIME_TYPE = "video/avc"; // H.264
    private static final int FRAME_RATE = 60;
    private static final int I_FRAME_INTERVAL = 1; // 1 second
    
    private MediaCodec encoder;
    private Surface inputSurface;
    private Callback callback;
    
    private int width;
    private int height;
    private int bitrate;
    
    private Thread encoderThread;
    private volatile boolean isRunning = false;
    
    public interface Callback {
        void onEncodedData(ByteBuffer data, MediaCodec.BufferInfo bufferInfo);
    }
    
    public VideoEncoder(int width, int height, int bitrate) throws IOException {
        this.width = width;
        this.height = height;
        this.bitrate = bitrate;
        
        initEncoder();
    }
    
    private void initEncoder() throws IOException {
        // Try to find hardware encoder
        String encoderName = findHardwareEncoder();
        if (encoderName != null) {
            encoder = MediaCodec.createByCodecName(encoderName);
            Log.i(TAG, "Using hardware encoder: " + encoderName);
        } else {
            encoder = MediaCodec.createEncoderByType(MIME_TYPE);
            Log.i(TAG, "Using default H.264 encoder");
        }
        
        // Configure encoder
        MediaFormat format = MediaFormat.createVideoFormat(MIME_TYPE, width, height);
        format.setInteger(MediaFormat.KEY_COLOR_FORMAT, 
            MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface);
        format.setInteger(MediaFormat.KEY_BIT_RATE, bitrate);
        format.setInteger(MediaFormat.KEY_FRAME_RATE, FRAME_RATE);
        format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, I_FRAME_INTERVAL);
        
        // Low latency settings
        format.setInteger(MediaFormat.KEY_LATENCY, 0);
        format.setInteger(MediaFormat.KEY_PRIORITY, 0); // Realtime priority
        
        encoder.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
        inputSurface = encoder.createInputSurface();
    }
    
    private String findHardwareEncoder() {
        // Prefer hardware encoders
        String[] preferredEncoders = {
            "c2.qti.avc.encoder",       // Qualcomm
            "OMX.qcom.video.encoder.avc",
            "OMX.Exynos.AVC.Encoder",   // Samsung
            "OMX.MTK.VIDEO.ENCODER.AVC"  // MediaTek
        };
        
        for (String name : preferredEncoders) {
            try {
                MediaCodec codec = MediaCodec.createByCodecName(name);
                codec.release();
                return name;
            } catch (IOException e) {
                // Try next
            }
        }
        
        return null;
    }
    
    public void start() {
        if (encoder == null) {
            return;
        }
        
        encoder.start();
        isRunning = true;
        
        // Start encoder thread
        encoderThread = new Thread(new Runnable() {
            @Override
            public void run() {
                encodeLoop();
            }
        });
        encoderThread.start();
    }
    
    private void encodeLoop() {
        MediaCodec.BufferInfo bufferInfo = new MediaCodec.BufferInfo();
        
        while (isRunning) {
            int outputBufferId = encoder.dequeueOutputBuffer(bufferInfo, 10000);
            
            if (outputBufferId >= 0) {
                ByteBuffer outputBuffer = encoder.getOutputBuffer(outputBufferId);
                
                if (outputBuffer != null && bufferInfo.size > 0) {
                    // Make a copy for async sending
                    byte[] data = new byte[bufferInfo.size];
                    outputBuffer.get(data);
                    ByteBuffer dataCopy = ByteBuffer.wrap(data);
                    
                    // Send to callback
                    if (callback != null) {
                        callback.onEncodedData(dataCopy, bufferInfo);
                    }
                }
                
                encoder.releaseOutputBuffer(outputBufferId, false);
            } else if (outputBufferId == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                MediaFormat newFormat = encoder.getOutputFormat();
                Log.i(TAG, "Output format changed: " + newFormat);
            }
        }
    }
    
    public void stop() {
        isRunning = false;
        
        if (encoderThread != null) {
            try {
                encoderThread.join(1000);
            } catch (InterruptedException e) {
                Log.e(TAG, "Encoder thread join interrupted", e);
            }
        }
        
        if (encoder != null) {
            try {
                encoder.stop();
                encoder.release();
            } catch (Exception e) {
                Log.e(TAG, "Error stopping encoder", e);
            }
            encoder = null;
        }
        
        if (inputSurface != null) {
            inputSurface.release();
            inputSurface = null;
        }
    }
    
    public void setBitrate(int newBitrate) {
        if (encoder == null) {
            return;
        }
        
        this.bitrate = newBitrate;
        
        // Update encoder bitrate dynamically
        android.os.Bundle params = new android.os.Bundle();
        params.putInt(MediaCodec.PARAMETER_KEY_VIDEO_BITRATE, bitrate);
        encoder.setParameters(params);
        
        Log.i(TAG, "Bitrate updated to: " + (bitrate / 1_000_000) + " Mbps");
    }
    
    public void requestKeyframe() {
        if (encoder != null) {
            android.os.Bundle params = new android.os.Bundle();
            params.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0);
            encoder.setParameters(params);
            Log.i(TAG, "Keyframe requested");
        }
    }
    
    public Surface getSurface() {
        return inputSurface;
    }
    
    public void setCallback(Callback callback) {
        this.callback = callback;
    }
}
