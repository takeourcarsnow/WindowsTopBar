@echo off
echo Building topbar in release mode...
cargo build --release
if %errorlevel% neq 0 exit /b %errorlevel%

echo Creating MSI installer...
cargo wix --bin-path "C:\Program Files (x86)\WiX Toolset v3.14\bin"
if %errorlevel% neq 0 exit /b %errorlevel%

echo Build complete. MSI is in target\wix\