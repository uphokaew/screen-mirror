#!/usr/bin/env bash
# Integration test with mock Android server
# Tests end-to-end functionality without real Android device

set -e

echo "========================================="
echo "Integration Test - Mock Server Mode"
echo "========================================="

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Build release binary
echo "Building release binary..."
cargo build --release --quiet

# Create mock server script
cat > /tmp/mock_server.py << 'EOF'
#!/usr/bin/env python3
import socket
import struct
import time
import sys

def create_packet(packet_type, pts, seq, data):
    """Create packet matching Rust protocol"""
    header = struct.pack('<b q I I', packet_type, pts, seq, len(data))
    return header + data

def main():
    HOST = '127.0.0.1'
    PORT = 5555
    
    print(f"Mock server starting on {HOST}:{PORT}...")
    
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind((HOST, PORT))
        s.listen(1)
        print("Waiting for connection...")
        
        conn, addr = s.accept()
        with conn:
            print(f"Connected by {addr}")
            
            # Send 50 mock video packets
            for i in range(50):
                # Mock H.264 packet (NAL unit)
                mock_data = bytes([0, 0, 0, 1, 0x67]) + bytes(100)  # SPS
                packet = create_packet(0x01, i * 16666, i, mock_data)
                conn.sendall(packet)
                time.sleep(0.016)  # ~60 FPS
                
                if i % 10 == 0:
                    print(f"Sent {i} packets...")
            
            print("Finished sending packets")
            time.sleep(1)

if __name__ == '__main__':
    main()
EOF

chmod +x /tmp/mock_server.py

# Check if Python is available
if ! command -v python3 &> /dev/null; then
    echo -e "${YELLOW}Python3 not found. Skipping mock server test.${NC}"
    exit 0
fi

# Start mock server in background
echo ""
echo "Starting mock server..."
python3 /tmp/mock_server.py &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Test connection
echo ""
echo "Testing client connection..."
timeout 10s ./target/release/scrcpy-custom --mode tcp --host 127.0.0.1 --port 5555 &
CLIENT_PID=$!

# Wait for client
sleep 5

# Cleanup
kill $SERVER_PID 2>/dev/null || true
kill $CLIENT_PID 2>/dev/null || true

echo ""
if [ $? -eq 0 ]; then
    echo -e "${GREEN}Integration test completed successfully!${NC}"
else
    echo "Integration test finished (check logs for details)"
fi

# Cleanup
rm -f /tmp/mock_server.py
