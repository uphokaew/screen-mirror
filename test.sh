#!/usr/bin/env bash
# Cross-platform test suite for scrcpy-custom
# Tests basic functionality without requiring Android server

set -e

echo "========================================="
echo "scrcpy-custom Test Suite"
echo "========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to print test results
test_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓ PASS${NC}: $2"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗ FAIL${NC}: $2"
        ((TESTS_FAILED++))
    fi
}

# Test 1: Rust toolchain
echo "Test 1: Checking Rust toolchain..."
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(cargo --version)
    test_result 0 "Rust installed: $RUST_VERSION"
else
    test_result 1 "Rust not found"
    exit 1
fi

# Test 2: FFmpeg
echo ""
echo "Test 2: Checking FFmpeg..."
if command -v ffmpeg &> /dev/null; then
    FFMPEG_VERSION=$(ffmpeg -version | head -n1)
    test_result 0 "FFmpeg installed: $FFMPEG_VERSION"
else
    test_result 1 "FFmpeg not found"
fi

# Test 3: ADB
echo ""
echo "Test 3: Checking ADB..."
if command -v adb &> /dev/null; then
    ADB_VERSION=$(adb version | head -n1)
    test_result 0 "ADB installed: $ADB_VERSION"
else
    echo -e "${YELLOW}⚠ WARNING${NC}: ADB not found (optional for USB mode)"
fi

# Test 4: Project structure
echo ""
echo "Test 4: Checking project structure..."
REQUIRED_FILES=(
    "Cargo.toml"
    "src/main.rs"
    "src/lib.rs"
    "src/network/mod.rs"
    "src/video/decoder.rs"
    "src/audio/player.rs"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        test_result 0 "Found: $file"
    else
        test_result 1 "Missing: $file"
    fi
done

# Test 5: Compilation test
echo ""
echo "Test 5: Testing compilation..."
if cargo check --quiet 2>&1; then
    test_result 0 "Code compiles successfully"
else
    test_result 1 "Compilation failed"
fi

# Test 6: Unit tests
echo ""
echo "Test 6: Running unit tests..."
if cargo test --lib --quiet 2>&1; then
    test_result 0 "Unit tests passed"
else
    test_result 1 "Unit tests failed"
fi

# Test 7: Network module tests
echo ""
echo "Test 7: Testing network module..."
if cargo test --lib network --quiet 2>&1; then
    test_result 0 "Network tests passed"
else
    test_result 1 "Network tests failed"
fi

# Test 8: Build release binary
echo ""
echo "Test 8: Building release binary..."
if cargo build --release --quiet 2>&1; then
    BINARY_SIZE=$(du -h target/release/scrcpy-custom* | head -n1 | cut -f1)
    test_result 0 "Release build successful (size: $BINARY_SIZE)"
else
    test_result 1 "Release build failed"
fi

# Test 9: Check hardware acceleration support
echo ""
echo "Test 9: Checking hardware acceleration..."
if ffmpeg -hwaccels 2>/dev/null | grep -q "cuda\|qsv\|vaapi"; then
    HW_ACCEL=$(ffmpeg -hwaccels 2>/dev/null | grep -E "cuda|qsv|vaapi" | tr '\n' ', ')
    test_result 0 "Hardware acceleration available: $HW_ACCEL"
else
    echo -e "${YELLOW}⚠ WARNING${NC}: No hardware acceleration detected"
fi

# Test 10: Platform detection
echo ""
echo "Test 10: Platform detection..."
OS_TYPE=$(uname -s)
case "$OS_TYPE" in
    Linux*)
        test_result 0 "Platform: Linux"
        # Check VAAPI on Linux
        if command -v vainfo &> /dev/null; then
            echo "  → VAAPI support detected"
        fi
        ;;
    Darwin*)
        test_result 0 "Platform: macOS"
        echo "  → Metal backend will be used"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        test_result 0 "Platform: Windows (Git Bash)"
        ;;
    *)
        test_result 1 "Unknown platform: $OS_TYPE"
        ;;
esac

# Summary
echo ""
echo "========================================="
echo "Test Summary"
echo "========================================="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed! ✓${NC}"
    echo ""
    echo "Next steps:"
    echo "1. Connect Android device via USB"
    echo "2. Run: adb forward tcp:5555 tcp:5555"
    echo "3. Run: cargo run --release -- --mode tcp"
    exit 0
else
    echo -e "${RED}Some tests failed. Please fix issues before running.${NC}"
    exit 1
fi
