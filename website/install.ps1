# Vibe Ensemble MCP Server Installation Script for Windows
# Usage: iex ((New-Object System.Net.WebClient).DownloadString('https://get.vibeensemble.dev/install.ps1'))

param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\VibeEnsemble",
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

# Configuration
$GitHubRepo = "siy/vibe-ensemble-mcp"
$TempDir = "$env:TEMP\vibe-ensemble-install"

# Logging functions
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

# Detect architecture
function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64-pc-windows-msvc" }
        "ARM64" { return "aarch64-pc-windows-msvc" }
        default {
            Write-Error "Unsupported architecture: $arch"
            exit 1
        }
    }
}

# Check if running as administrator
function Test-Administrator {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Get latest release version
function Get-LatestVersion {
    Write-Info "Fetching latest release version..."
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$GitHubRepo/releases/latest"
        return $response.tag_name
    }
    catch {
        Write-Error "Failed to fetch latest version: $($_.Exception.Message)"
        exit 1
    }
}

# Download and extract binaries
function Install-Binaries {
    param(
        [string]$Version,
        [string]$Platform
    )
    
    $filename = "vibe-ensemble-mcp-windows-$Platform.zip"
    $downloadUrl = "https://github.com/$GitHubRepo/releases/download/$Version/$filename"
    
    Write-Info "Downloading Vibe Ensemble MCP Server $Version for $Platform..."
    
    # Create temp directory
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    
    $zipPath = Join-Path $TempDir $filename
    
    try {
        # Download with progress
        $webClient = New-Object System.Net.WebClient
        $webClient.DownloadFile($downloadUrl, $zipPath)
        Write-Success "Download completed"
    }
    catch {
        Write-Error "Failed to download from $downloadUrl : $($_.Exception.Message)"
        exit 1
    }
    
    # Extract
    Write-Info "Extracting binaries..."
    try {
        Expand-Archive -Path $zipPath -DestinationPath $TempDir -Force
        Write-Success "Extraction completed"
    }
    catch {
        Write-Error "Failed to extract $zipPath : $($_.Exception.Message)"
        exit 1
    }
    
    # Create install directory
    Write-Info "Installing binaries to $InstallDir..."
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    
    # Install binaries
    $binaries = @("vibe-ensemble-server.exe", "vibe-ensemble-mcp.exe")
    
    foreach ($binary in $binaries) {
        $sourcePath = Join-Path $TempDir $binary
        $destPath = Join-Path $InstallDir $binary
        
        if (Test-Path $sourcePath) {
            Write-Info "Installing $binary..."
            Copy-Item $sourcePath $destPath -Force
            Write-Success "$binary installed successfully"
        }
        else {
            Write-Warning "$binary not found in archive"
        }
    }
}

# Add to PATH
function Add-ToPath {
    Write-Info "Adding $InstallDir to PATH..."
    
    # Get current user PATH
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    
    # Check if already in PATH
    if ($currentPath -split ";" | Where-Object { $_ -eq $InstallDir }) {
        Write-Info "Directory already in PATH"
        return
    }
    
    # Add to PATH
    $newPath = if ($currentPath) { "$currentPath;$InstallDir" } else { $InstallDir }
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    
    # Update current session PATH
    $env:PATH = "$env:PATH;$InstallDir"
    
    Write-Success "Added to PATH"
}

# Verify installation
function Test-Installation {
    Write-Info "Verifying installation..."
    
    $serverPath = Join-Path $InstallDir "vibe-ensemble-server.exe"
    $mcpPath = Join-Path $InstallDir "vibe-ensemble-mcp.exe"
    
    if (Test-Path $serverPath) {
        Write-Success "vibe-ensemble-server.exe installed"
    }
    else {
        Write-Error "vibe-ensemble-server.exe not found"
        exit 1
    }
    
    if (Test-Path $mcpPath) {
        Write-Success "vibe-ensemble-mcp.exe installed"
    }
    else {
        Write-Error "vibe-ensemble-mcp.exe not found"
        exit 1
    }
}

# Cleanup
function Remove-TempFiles {
    Write-Info "Cleaning up temporary files..."
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force
    }
    Write-Success "Cleanup completed"
}

# Print next steps
function Show-NextSteps {
    Write-Host ""
    Write-Success "🎉 Vibe Ensemble MCP Server installed successfully!"
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Blue
    Write-Host "1. Restart your terminal or run: refreshenv"
    Write-Host "2. Start the server:"
    Write-Host "   vibe-ensemble-server" -ForegroundColor Green
    Write-Host ""
    Write-Host "3. Configure Claude Code MCP settings:"
    Write-Host '   {'
    Write-Host '     "mcpServers": {'
    Write-Host '       "vibe-ensemble": {'
    Write-Host '         "command": "vibe-ensemble-mcp",'
    Write-Host '         "args": ["--transport=stdio"]'
    Write-Host '       }'
    Write-Host '     }'
    Write-Host '   }'
    Write-Host ""
    Write-Host "4. Access the web dashboard: http://127.0.0.1:8081"
    Write-Host "5. Check health: http://127.0.0.1:8080/health"
    Write-Host ""
    Write-Host "Documentation: https://vibeensemble.dev" -ForegroundColor Blue
    Write-Host "GitHub: https://github.com/$GitHubRepo" -ForegroundColor Blue
    Write-Host ""
}

# Main installation function
function Install-VibeEnsemble {
    Write-Host "Vibe Ensemble MCP Server Installer" -ForegroundColor Blue
    Write-Host "============================================"
    Write-Host ""
    
    # Check PowerShell version
    if ($PSVersionTable.PSVersion.Major -lt 5) {
        Write-Error "PowerShell 5.0 or higher is required"
        exit 1
    }
    
    # Detect platform
    $platform = Get-Architecture
    Write-Info "Detected platform: $platform"
    
    # Get version
    if ($Version -eq "latest") {
        $Version = Get-LatestVersion
    }
    Write-Info "Installing version: $Version"
    
    try {
        Install-Binaries -Version $Version -Platform $platform
        Add-ToPath
        Test-Installation
        Remove-TempFiles
        Show-NextSteps
    }
    catch {
        Write-Error "Installation failed: $($_.Exception.Message)"
        Remove-TempFiles
        exit 1
    }
}

# Run installation
Install-VibeEnsemble