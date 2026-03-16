# NuClaw Cross-Platform Installer (PowerShell Wizard Version)
# Supports: Windows 10/11, Windows Server

param(
    [string]$Version = "latest",
    [string]$InstallPath = "",
    [switch]$Quiet = $false
)

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "gyc567/nuclaw"
$DefaultInstallPath = if ($env:NUCLAW_HOME) { $env:NUCLAW_HOME } elseif ($env:USERPROFILE) { "$env:USERPROFILE\.nuclaw" } else { "$env:HOME\.nuclaw" }
if (-not $InstallPath) { $InstallPath = $DefaultInstallPath }

# Colors for output
function Write-ColorOutput {
    param([string]$Message, [string]$Color = "White")
    $colors = @{ Red = "Red"; Green = "Green"; Yellow = "Yellow"; Cyan = "Cyan"; White = "White" }
    Write-Host $Message -ForegroundColor $colors[$Color]
}

# Interactive confirmation helper
function Confirm-Action {
    param([string]$Question)
    if ($Quiet) { return $true }
    $choice = Read-Host "$Question (y/N)"
    return ($choice -eq "y" -or $choice -eq "Y" -or $choice -eq "yes")
}

function Detect-Platform {
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($arch -eq "AMD64") { $arch = "x86_64" }
    elseif ($arch -eq "ARM64") { $arch = "arm64" }
    return @{ OS = "windows"; Arch = $arch }
}

function Get-LatestVersion {
    $url = "https://api.github.com/repos/$Repo/releases/latest"
    try {
        $response = Invoke-RestMethod -Uri $url -UseBasicParsing
        return $response.tag_name -replace '^v', ''
    } catch { return "" }
}

function Install-Binary {
    param($Version, $Arch)
    $filename = "nuclaw-$Version-$Arch-pc-windows-msvc.zip"
    $url = "https://github.com/$Repo/releases/download/v$Version/$filename"
    $temp = if ($env:TEMP) { $env:TEMP } else { "C:\Temp" }
    $output = "$temp\nuclaw.zip"
    
    Write-ColorOutput "Downloading pre-built binary..." Cyan
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $url -OutFile $output -UseBasicParsing
        
        if (-not (Test-Path $InstallPath)) { New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null }
        Expand-Archive -Path $output -DestinationPath $InstallPath -Force
        Remove-Item $output -Force
        return $true
    } catch { return $false }
}

function Build-FromSource {
    Write-ColorOutput "Pre-built binary not available." Yellow
    if (-not (Confirm-Action "Would you like to install Rust and build from source? (Takes ~10 mins)")) {
        Write-ColorOutput "Installation aborted." Red
        exit
    }
    
    # Check Rust
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-ColorOutput "Installing Rust..." Cyan
        Invoke-WebRequest -Uri "https://win.rustup.rs" -OutFile "rustup-init.exe" -UseBasicParsing
        Start-Process -FilePath ".\rustup-init.exe" -ArgumentList "-y" -Wait
        $env:PATH += ";$env:USERPROFILE\.cargo\bin"
    }
    
    $temp = if ($env:TEMP) { $env:TEMP } else { "C:\Temp" }
    $buildDir = "$temp\nuclaw-build"
    if (Test-Path $buildDir) { Remove-Item $buildDir -Recurse -Force }
    git clone --depth 1 "https://github.com/$Repo.git" $buildDir
    
    Push-Location $buildDir
    cargo build --release
    Pop-Location
    
    if (-not (Test-Path $InstallPath)) { New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null }
    Copy-Item "$buildDir\target\release\nuclaw.exe" $InstallPath -Force
    Remove-Item $buildDir -Recurse -Force
}

function Setup-Path {
    if ($env:PATH -notlike "*$InstallPath*") {
        if (Confirm-Action "Would you like to add NuClaw to your User PATH?") {
            $oldPath = [Environment]::GetEnvironmentVariable("Path", "User")
            [Environment]::SetEnvironmentVariable("Path", "$oldPath;$InstallPath", "User")
            Write-ColorOutput "Added to User PATH. Please restart your terminal." Green
        }
    }
}

# Main
Write-ColorOutput ""
Write-ColorOutput "==========================================" Cyan
Write-ColorOutput "    NuClaw Setup Wizard for Windows       " Cyan
Write-ColorOutput "==========================================" Cyan

$platform = Detect-Platform
$Version = if ($Version -eq "latest") { Get-LatestVersion } else { $Version }

if (-not $Version -or -not (Install-Binary -Version $Version -Arch $platform.Arch)) {
    Build-FromSource
}

# Scaffolding
$dirs = @("store", "data", "groups", "logs", "skills")
foreach ($dir in $dirs) {
    $path = Join-Path $InstallPath $dir
    if (-not (Test-Path $path)) { New-Item -ItemType Directory -Path $path -Force | Out-Null }
}

if (Confirm-Action "Would you like to install NuClaw as a background service (Scheduled Task)?") {
    # Service installation logic...
}

Setup-Path

Write-ColorOutput ""
Write-ColorOutput "Installation Complete!" Green
if (Confirm-Action "Would you like to start the LLM/Bot configuration wizard now?") {
    & "$InstallPath\nuclaw.exe" --onboard
} else {
    Write-ColorOutput "To configure later, run: $InstallPath\nuclaw.exe --onboard" White
}
