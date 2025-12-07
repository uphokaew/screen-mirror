# Cross-platform test suite for scrcpy-custom (Windows PowerShell)
# Tests basic functionality without requiring Android server

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "scrcpy-custom Test Suite (Windows)" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host ""

$TestsPassed = 0
$TestsFailed = 0

function Test-Result {
    param($Success, $Message)
    if ($Success) {
        Write-Host "✓ PASS: " -ForegroundColor Green -NoNewline
        Write-Host $Message
        $script:TestsPassed++
    } else {
        Write-Host "✗ FAIL: " -ForegroundColor Red -NoNewline
        Write-Host $Message
        $script:TestsFailed++
    }
}

# Test 1: Rust toolchain
Write-Host "Test 1: Checking Rust toolchain..."
try {
    $rustVersion = cargo --version
    Test-Result $true "Rust installed: $rustVersion"
} catch {
    Test-Result $false "Rust not found"
    exit 1
}

# Test 2: FFmpeg
Write-Host ""
Write-Host "Test 2: Checking FFmpeg..."
try {
    $ffmpegVersion = (ffmpeg -version 2>&1 | Select-Object -First 1)
    Test-Result $true "FFmpeg installed: $ffmpegVersion"
} catch {
    Test-Result $false "FFmpeg not found"
}

# Test 3: ADB
Write-Host ""
Write-Host "Test 3: Checking ADB..."
try {
    $adbVersion = (adb version 2>&1 | Select-Object -First 1)
    Test-Result $true "ADB installed: $adbVersion"
} catch {
    Write-Host "⚠ WARNING: ADB not found (optional for USB mode)" -ForegroundColor Yellow
}

# Test 4: Project structure
Write-Host ""
Write-Host "Test 4: Checking project structure..."
$requiredFiles = @(
    "Cargo.toml",
    "src\main.rs",
    "src\lib.rs",
    "src\network\mod.rs",
    "src\video\decoder.rs",
    "src\audio\player.rs"
)

foreach ($file in $requiredFiles) {
    if (Test-Path $file) {
        Test-Result $true "Found: $file"
    } else {
        Test-Result $false "Missing: $file"
    }
}

# Test 5: Compilation test
Write-Host ""
Write-Host "Test 5: Testing compilation..."
$compileResult = cargo check 2>&1
if ($LASTEXITCODE -eq 0) {
    Test-Result $true "Code compiles successfully"
} else {
    Test-Result $false "Compilation failed"
}

# Test 6: Unit tests
Write-Host ""
Write-Host "Test 6: Running unit tests..."
$testResult = cargo test --lib 2>&1
if ($LASTEXITCODE -eq 0) {
    Test-Result $true "Unit tests passed"
} else {
    Test-Result $false "Unit tests failed"
}

# Test 7: Build release binary
Write-Host ""
Write-Host "Test 7: Building release binary..."
$buildResult = cargo build --release 2>&1
if ($LASTEXITCODE -eq 0) {
    $binaryPath = "target\release\scrcpy-custom.exe"
    if (Test-Path $binaryPath) {
        $size = (Get-Item $binaryPath).Length / 1MB
        Test-Result $true "Release build successful (size: $([math]::Round($size, 2)) MB)"
    }
} else {
    Test-Result $false "Release build failed"
}

# Test 8: Check GPU
Write-Host ""
Write-Host "Test 8: Checking GPU..."
try {
    $gpu = Get-WmiObject Win32_VideoController | Select-Object -First 1 -ExpandProperty Name
    Test-Result $true "GPU detected: $gpu"
    
    # Check for NVIDIA
    if ($gpu -like "*NVIDIA*") {
        Write-Host "  → NVDEC support likely available" -ForegroundColor Green
    }
    # Check for Intel
    elseif ($gpu -like "*Intel*") {
        Write-Host "  → QSV support likely available" -ForegroundColor Green
    }
} catch {
    Write-Host "  ⚠ Could not detect GPU" -ForegroundColor Yellow
}

# Test 9: Check Visual C++ Runtime
Write-Host ""
Write-Host "Test 9: Checking Visual C++ Runtime..."
$vcRedist = Get-ItemProperty "HKLM:\Software\Microsoft\VisualStudio\*\VC\Runtimes\*" -ErrorAction SilentlyContinue
if ($vcRedist) {
    Test-Result $true "Visual C++ Runtime installed"
} else {
    Write-Host "  ⚠ Visual C++ Runtime may be missing" -ForegroundColor Yellow
}

# Test 10: Platform info
Write-Host ""
Write-Host "Test 10: Platform detection..."
$osInfo = Get-WmiObject Win32_OperatingSystem
Test-Result $true "Platform: $($osInfo.Caption) $($osInfo.OSArchitecture)"

# Summary
Write-Host ""
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Test Summary" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Passed: " -NoNewline
Write-Host $TestsPassed -ForegroundColor Green
Write-Host "Failed: " -NoNewline
Write-Host $TestsFailed -ForegroundColor Red
Write-Host ""

if ($TestsFailed -eq 0) {
    Write-Host "All tests passed! ✓" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "1. Connect Android device via USB"
    Write-Host "2. Run: adb forward tcp:5555 tcp:5555"
    Write-Host "3. Run: cargo run --release -- --mode tcp"
    exit 0
} else {
    Write-Host "Some tests failed. Please fix issues before running." -ForegroundColor Red
    exit 1
}
