package com.scrcpy.custom.server;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.media.projection.MediaProjectionManager;
import android.os.Bundle;
import android.view.View;
import android.widget.Button;
import android.widget.TextView;
import android.widget.Toast;

import androidx.appcompat.app.AppCompatActivity;

import com.scrcpy.custom.server.service.ScreenCaptureService;

/**
 * Main activity for scrcpy-custom Android server
 * Handles MediaProjection permission and service lifecycle
 */
public class MainActivity extends AppCompatActivity {
    private static final int REQUEST_MEDIA_PROJECTION = 1;
    
    private MediaProjectionManager projectionManager;
    private Button btnStartCapture;
    private TextView tvStatus;
    private TextView tvIpAddress;
    
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);
        
        // Initialize UI
        btnStartCapture = findViewById(R.id.btnStartCapture);
        tvStatus = findViewById(R.id.tvStatus);
        tvIpAddress = findViewById(R.id.tvIpAddress);
        
        // Get MediaProjection manager
        projectionManager = (MediaProjectionManager) 
            getSystemService(Context.MEDIA_PROJECTION_SERVICE);
        
        // Display IP address
        displayIPAddress();
        
        // Start capture button
        btnStartCapture.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View v) {
                requestMediaProjection();
            }
        });
    }
    
    private void requestMediaProjection() {
        // Request screen capture permission
        Intent intent = projectionManager.createScreenCaptureIntent();
        startActivityForResult(intent, REQUEST_MEDIA_PROJECTION);
    }
    
    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        
        if (requestCode == REQUEST_MEDIA_PROJECTION) {
            if (resultCode == Activity.RESULT_OK) {
                // Permission granted, start service
                startCaptureService(resultCode, data);
                updateStatus("Capture started");
            } else {
                // Permission denied
                Toast.makeText(this, "Screen capture permission denied", 
                    Toast.LENGTH_SHORT).show();
                updateStatus("Permission denied");
            }
        }
    }
    
    private void startCaptureService(int resultCode, Intent data) {
        Intent serviceIntent = new Intent(this, ScreenCaptureService.class);
        serviceIntent.putExtra("resultCode", resultCode);
        serviceIntent.putExtra("data", data);
        
        // Start foreground service
        startForegroundService(serviceIntent);
        
        btnStartCapture.setEnabled(false);
        btnStartCapture.setText("Capturing...");
    }
    
    private void displayIPAddress() {
        String ipAddress = NetworkUtils.getLocalIPAddress();
        if (ipAddress != null) {
            tvIpAddress.setText("Server IP: " + ipAddress);
        } else {
            tvIpAddress.setText("No network connection");
        }
    }
    
    private void updateStatus(String status) {
        tvStatus.setText("Status: " + status);
    }
    
    @Override
    protected void onDestroy() {
        super.onDestroy();
        // Stop service when activity is destroyed
        Intent serviceIntent = new Intent(this, ScreenCaptureService.class);
        stopService(serviceIntent);
    }
}
