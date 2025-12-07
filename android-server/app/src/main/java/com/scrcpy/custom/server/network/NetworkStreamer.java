package com.scrcpy.custom.server.network;

import android.media.MediaCodec;
import android.util.Log;

import java.io.DataOutputStream;
import java.io.IOException;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.ByteBuffer;

/**
 * Network streamer for sending encoded video over TCP
 * Implements protocol compatible with Rust client
 */
public class NetworkStreamer {
    private static final String TAG = "NetworkStreamer";
    
    private ServerSocket serverSocket;
    private Socket clientSocket;
    private DataOutputStream outputStream;
    private Thread serverThread;
    
    private int port;
    private volatile boolean isRunning = false;
    private int sequenceNumber = 0;
    
    // Packet types (must match Rust protocol)
    private static final byte PACKET_TYPE_VIDEO = 0x01;
    private static final byte PACKET_TYPE_AUDIO = 0x02;
    private static final byte PACKET_TYPE_CONTROL = 0x03;
    
    public NetworkStreamer(int port) {
        this.port = port;
    }
    
    public void start() {
        isRunning = true;
        
        serverThread = new Thread(new Runnable() {
            @Override
            public void run() {
                acceptConnections();
            }
        });
        serverThread.start();
    }
    
    private void acceptConnections() {
        try {
            serverSocket = new ServerSocket(port);
            Log.i(TAG, "Server listening on port " + port);
            
            while (isRunning) {
                // Accept client connection
                clientSocket = serverSocket.accept();
                Log.i(TAG, "Client connected: " + clientSocket.getInetAddress());
                
                outputStream = new DataOutputStream(clientSocket.getOutputStream());
                
                // Keep connection alive
                while (isRunning && clientSocket.isConnected()) {
                    Thread.sleep(100);
                }
            }
        } catch (IOException | InterruptedException e) {
            if (isRunning) {
                Log.e(TAG, "Server error", e);
            }
        } finally {
            closeConnection();
        }
    }
    
    public void sendVideoPacket(ByteBuffer data, MediaCodec.BufferInfo bufferInfo) {
        if (outputStream == null || !clientSocket.isConnected()) {
            return;
        }
        
        try {
            // Create packet matching Rust protocol
            // Header: Type(1) + PTS(8) + Sequence(4) + Length(4) = 17 bytes
            
            byte[] header = new byte[17];
            int offset = 0;
            
            // Type
            header[offset++] = PACKET_TYPE_VIDEO;
            
            // PTS (8 bytes, little-endian)
            long pts = bufferInfo.presentationTimeUs;
            writeLongLE(header, offset, pts);
            offset += 8;
            
            // Sequence (4 bytes, little-endian)
            writeIntLE(header, offset, sequenceNumber++);
            offset += 4;
            
            // Length (4 bytes, little-endian)
            writeIntLE(header, offset, bufferInfo.size);
            
            // Send header
            outputStream.write(header);
            
            // Send data
            byte[] videoData = new byte[bufferInfo.size];
            data.position(bufferInfo.offset);
            data.get(videoData);
            outputStream.write(videoData);
            outputStream.flush();
            
        } catch (IOException e) {
            Log.e(TAG, "Failed to send packet", e);
            closeConnection();
        }
    }
    
    public void stop() {
        isRunning = false;
        closeConnection();
        
        if (serverThread != null) {
            try {
                serverThread.join(1000);
            } catch (InterruptedException e) {
                Log.e(TAG, "Server thread join interrupted", e);
            }
        }
    }
    
    private void closeConnection() {
        try {
            if (outputStream != null) {
                outputStream.close();
                outputStream = null;
            }
        } catch (IOException e) {
            Log.e(TAG, "Error closing output stream", e);
        }
        
        try {
            if (clientSocket != null) {
                clientSocket.close();
                clientSocket = null;
            }
        } catch (IOException e) {
            Log.e(TAG, "Error closing client socket", e);
        }
        
        try {
            if (serverSocket != null) {
                serverSocket.close();
                serverSocket = null;
            }
        } catch (IOException e) {
            Log.e(TAG, "Error closing server socket", e);
        }
    }
    
    // Helper methods for little-endian encoding
    private void writeLongLE(byte[] buffer, int offset, long value) {
        buffer[offset] = (byte) (value & 0xFF);
        buffer[offset + 1] = (byte) ((value >> 8) & 0xFF);
        buffer[offset + 2] = (byte) ((value >> 16) & 0xFF);
        buffer[offset + 3] = (byte) ((value >> 24) & 0xFF);
        buffer[offset + 4] = (byte) ((value >> 32) & 0xFF);
        buffer[offset + 5] = (byte) ((value >> 40) & 0xFF);
        buffer[offset + 6] = (byte) ((value >> 48) & 0xFF);
        buffer[offset + 7] = (byte) ((value >> 56) & 0xFF);
    }
    
    private void writeIntLE(byte[] buffer, int offset, int value) {
        buffer[offset] = (byte) (value & 0xFF);
        buffer[offset + 1] = (byte) ((value >> 8) & 0xFF);
        buffer[offset + 2] = (byte) ((value >> 16) & 0xFF);
        buffer[offset + 3] = (byte) ((value >> 24) & 0xFF);
    }
}
