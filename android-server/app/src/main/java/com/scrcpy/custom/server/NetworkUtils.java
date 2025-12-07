package com.scrcpy.custom.server;

import java.net.InetAddress;
import java.net.NetworkInterface;
import java.util.Enumeration;

/**
 * Network utility methods
 */
public class NetworkUtils {
    
    /**
     * Get local IP address (WiFi or Mobile data)
     */
    public static String getLocalIPAddress() {
        try {
            Enumeration<NetworkInterface> interfaces = NetworkInterface.getNetworkInterfaces();
            while (interfaces.hasMoreElements()) {
                NetworkInterface networkInterface = interfaces.nextElement();
                Enumeration<InetAddress> addresses = networkInterface.getInetAddresses();
                
                while (addresses.hasMoreElements()) {
                    InetAddress address = addresses.nextElement();
                    
                    // Skip loopback addresses
                    if (address.isLoopbackAddress()) {
                        continue;
                    }
                    
                    // Get IPv4 address
                    String hostAddress = address.getHostAddress();
                    if (hostAddress != null && hostAddress.indexOf(':') < 0) {
                        return hostAddress;
                    }
                }
            }
        } catch (Exception e) {
            e.printStackTrace();
        }
        
        return null;
    }
}
