# Windows build script for ripley-agent
# This script builds the agent for Windows and optionally creates an installer

param(
    [switch]$Release = $true,
    [switch]$CreateInstaller = $false
)

$ErrorActionPreference = "Stop"

Write-Host "Building ripley-agent for Windows..." -ForegroundColor Green

# Navigate to agent directory
Push-Location $PSScriptRoot

try {
    # Build the agent
    $buildType = if ($Release) { "release" } else { "debug" }
    Write-Host "Building in $buildType mode..." -ForegroundColor Yellow
    
    if ($Release) {
        cargo build --release
    } else {
        cargo build
    }
    
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed"
    }
    
    Write-Host "Build successful!" -ForegroundColor Green
    
    # Create output directory
    $outputDir = "dist"
    if (-not (Test-Path $outputDir)) {
        New-Item -ItemType Directory -Path $outputDir | Out-Null
    }
    
    # Copy executable
    $exePath = if ($Release) { "target\release\ripley-agent.exe" } else { "target\debug\ripley-agent.exe" }
    $destPath = Join-Path $outputDir "ripley-agent.exe"
    
    Copy-Item $exePath $destPath -Force
    Write-Host "Copied executable to $destPath" -ForegroundColor Green
    
    # Create zip archive
    $zipPath = Join-Path $outputDir "ripley-agent-windows.zip"
    if (Test-Path $zipPath) {
        Remove-Item $zipPath -Force
    }
    Compress-Archive -Path $destPath -DestinationPath $zipPath
    Write-Host "Created archive: $zipPath" -ForegroundColor Green
    
    # Optionally create installer (requires WiX or similar)
    if ($CreateInstaller) {
        Write-Host "Installer creation not yet implemented" -ForegroundColor Yellow
        Write-Host "To create an installer, use WiX Toolset or Inno Setup" -ForegroundColor Yellow
    }
    
    Write-Host "`nBuild complete! Output files:" -ForegroundColor Green
    Write-Host "  - Executable: $destPath" -ForegroundColor Cyan
    Write-Host "  - Archive: $zipPath" -ForegroundColor Cyan
    
} catch {
    Write-Host "Error: $_" -ForegroundColor Red
    exit 1
} finally {
    Pop-Location
}
