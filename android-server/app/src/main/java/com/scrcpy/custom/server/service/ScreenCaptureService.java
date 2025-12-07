package com.scrcpy.custom.server.service;

import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.media.MediaFormat;
import android.media.projection.MediaProjection;
import android.media.projection.MediaProjectionManager;
import android.os.Build;
import android.os.IBinder;
import android.util.DisplayMetrics;
import android.util.Log;
import android.view.Surface;
import android.view.WindowManager;

import androidx.core.app.NotificationCompat;

import com.scrcpy.custom.server.MainActivity;
import com.scrcpy.custom.server.encoder.VideoEncoder;
import com.scrcpy.custom.server.network.NetworkStreamer;

import java.io.IOException;
import java.nio.ByteBuffer;

/**
 * Foreground service for screen capture and streaming
 */
public class ScreenCaptureService extends Service {
    private static final String TAG = "ScreenCaptureService";
    private static final String CHANNEL_ID = "screen_capture_channel";
    private static final int NOTIFICATION_ID = 1;
    
    private MediaProjection mediaProjection;
    private VirtualDisplay virtualDisplay;
    private VideoEncoder videoEncoder;
    private NetworkStreamer networkStreamer;
    
    private int width = 1080;
    private int height = 1920;
    private int dpi = 320;
    private int bitrate = 8_000_000; // 8 Mbps default
    
    @Override
    public void onCreate() {
        super.onCreate();
        createNotificationChannel();
    }
    
    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent == null) {
            return START_NOT_STICKY;
        }
        
        // Start foreground notification
        startForeground(NOTIFICATION_ID, createNotification());
        
        // Get MediaProjection result
        int resultCode = intent.getIntExtra("resultCode", 0);
        Intent data = intent.getParcelableExtra("data");
        
        if (resultCode != 0 && data != null) {
            startCapture(resultCode, data);
        }
        
        return START_STICKY;
    }
    
    private void startCapture(int resultCode, Intent data) {
        try {
            // Get screen metrics
            DisplayMetrics displayMetrics = getDisplayMetrics();
            width = displayMetrics.widthPixels;
            height = displayMetrics.heightPixels;
            dpi = displayMetrics.densityDpi;
            
            // Start network streamer
            networkStreamer = new NetworkStreamer(5555); // TCP port
            networkStreamer.start();
            
            // Initialize video encoder
            videoEncoder = new VideoEncoder(width, height, bitrate);
            videoEncoder.setCallback(new VideoEncoder.Callback() {
                @Override
                public void onEncodedData(ByteBuffer data, MediaCodec.BufferInfo bufferInfo) {
                    // Send encoded data to network
                    if (networkStreamer != null) {
                        networkStreamer.sendVideoPacket(data, bufferInfo);
                    }
                }
            });
            videoEncoder.start();
            
            // Get encoder surface
            Surface surface = videoEncoder.getSurface();
            
            // Create MediaProjection
            MediaProjectionManager manager = (MediaProjectionManager) 
                getSystemService(MEDIA_PROJECTION_SERVICE);
            mediaProjection = manager.getMediaProjection(resultCode, data);
            
            // Create virtual display
            virtualDisplay = mediaProjection.createVirtualDisplay(
                "scrcpy-capture",
                width, height, dpi,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                surface,
                null, null
            );
            
            Log.i(TAG, "Screen capture started: " + width + "x" + height);
            
        } catch (IOException e) {
            Log.e(TAG, "Failed to start capture", e);
            stopSelf();
        }
    }
    
    private DisplayMetrics getDisplayMetrics() {
        WindowManager windowManager = (WindowManager) getSystemService(WINDOW_SERVICE);
        DisplayMetrics metrics = new DisplayMetrics();
        windowManager.getDefaultDisplay().getRealMetrics(metrics);
        return metrics;
    }
    
    @Override
    public void onDestroy() {
        super.onDestroy();
        stopCapture();
    }
    
    private void stopCapture() {
        if (virtualDisplay != null) {
            virtualDisplay.release();
            virtualDisplay = null;
        }
        
        if (mediaProjection != null) {
            mediaProjection.stop();
            mediaProjection = null;
        }
        
        if (videoEncoder != null) {
            videoEncoder.stop();
            videoEncoder = null;
        }
        
        if (networkStreamer != null) {
            networkStreamer.stop();
            networkStreamer = null;
        }
        
        Log.i(TAG, "Screen capture stopped");
    }
    
    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
    
    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel channel = new NotificationChannel(
                CHANNEL_ID,
                "Screen Capture",
                NotificationManager.IMPORTANCE_LOW
            );
            channel.setDescription("Screen mirroring service");
            
            NotificationManager notificationManager = getSystemService(NotificationManager.class);
            notificationManager.createNotificationChannel(channel);
        }
    }
    
    private android.app.Notification createNotification() {
        Intent notificationIntent = new Intent(this, MainActivity.class);
        PendingIntent pendingIntent = PendingIntent.getActivity(
            this, 0, notificationIntent,
            PendingIntent.FLAG_IMMUTABLE
        );
        
        return new NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("scrcpy-custom")
            .setContentText("Screen mirroring active")
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentIntent(pendingIntent)
            .build();
    }
}
