$ffmpeg_root = "C:\ffmpeg"
$pkg_config_dir = "$ffmpeg_root\lib\pkgconfig"

# Create directory if not exists
New-Item -ItemType Directory -Force -Path $pkg_config_dir | Out-Null

$libs = @("libavcodec", "libavdevice", "libavfilter", "libavformat", "libavutil", "libswresample", "libswscale")

foreach ($lib in $libs) {
    $content = @"
prefix=$ffmpeg_root
exec_prefix=`${prefix}
libdir=`${prefix}/lib
includedir=`${prefix}/include

Name: $lib
Description: FFmpeg library $lib
Version: 6.0.0
Libs: -L`${libdir} -l$($lib.Substring(3))
Cflags: -I`${includedir}
"@
    
    # Replace backslashes with forward slashes for pkg-config compatibility
    $content = $content -replace "\\", "/"
    
    $path = Join-Path $pkg_config_dir "$lib.pc"
    Set-Content -Path $path -Value $content
    Write-Host "Created $path"
}

Write-Host "pkg-config files created successfully."
