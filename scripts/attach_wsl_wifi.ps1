# Strict mode to enforce robust coding practices
Set-StrictMode -Version Latest
$ErrorActionPreference = "Continue"

<#
.SYNOPSIS
    WSL2 Auto-Config Script for Aether WiFi Auditing & Analysis.

.DESCRIPTION
    This script seamlessly prepares the runtime environment for the Aether application.
    It performs the following:
    1. Ensures the target WSL2 instance (Kali) is awake and running.
    2. Uses 'usbipd' to locate a designated USB WiFi adapter.
    3. Binds the USB WiFi adapter on the Windows host.
    4. Attaches the adapter directly to the WSL instance.
    5. Gracefully handles errors (e.g., missing usbipd, missing adapter, adapter already attached).

.EXAMPLE
    .\attach_wsl_wifi.ps1 -WslHost "kali-linux" -AdapterName "802.11"
#>

param (
    # The name of the WSL instance (default: kali-linux)
    [string]$WslHost = "kali-linux",
    
    # A substring of the USB WiFi adapter name as it appears in usbipd list (e.g., "802.11", "Network", "Wireless")
    [string]$AdapterName = "802.11",

    # Enable detailed logging
    [switch]$VerboseOutput
)

# Logger function
function Write-Log {
    param (
        [string]$Message,
        [string]$Level = "INFO"
    )
    if ($Level -eq "VERBOSE" -and -not $VerboseOutput) { return }
    
    $timestamp = Get-Date -Format "HH:mm:ss"
    if ($Level -eq "INFO") {
        Write-Host "[$timestamp] [i] $Message" -ForegroundColor Cyan
    }
    elseif ($Level -eq "OK") {
        Write-Host "[$timestamp] [+] $Message" -ForegroundColor Green
    }
    elseif ($Level -eq "WARN") {
        Write-Host "[$timestamp] [*] $Message" -ForegroundColor Yellow
    }
    elseif ($Level -eq "ERROR") {
        Write-Host "[$timestamp] [!] $Message" -ForegroundColor Red
    }
    elseif ($Level -eq "VERBOSE") {
        Write-Host "[$timestamp] [v] $Message" -ForegroundColor DarkGray
    }
}

try {
    Write-Log "Initializing Aether WSL2 USB Interface Auto-Config..." "INFO"

    # Step 1: Check if usbipd-win is installed
    Write-Log "Checking for usbipd-win installation..." "VERBOSE"
    if (-not (Get-Command "usbipd" -ErrorAction SilentlyContinue)) {
        Write-Log "usbipd is not installed or not in PATH." "ERROR"
        Write-Log "Please install it: winget install --interactive --exact dorssel.usbipd-win" "WARN"
        exit 1
    }
    Write-Log "usbipd is installed." "OK"

    # Step 2: Wake up / Ensure the WSL instance is running
    Write-Log "Ensuring WSL instance '$WslHost' is awake and ready..." "VERBOSE"
    # Execute a simple command inside WSL to wake it up silently
    wsl.exe -d $WslHost --exec echo "Waking up WSL" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Log "Failed to interact with WSL instance '$WslHost'. Is it installed?" "ERROR"
        exit 1
    }
    Write-Log "WSL instance '$WslHost' is running." "OK"

    # Step 3: Find the USB WiFi adapter using usbipd
    Write-Log "Searching for USB WiFi adapter containing '$AdapterName'..." "VERBOSE"
    
    # We parse the output of 'usbipd list'. It typically looks like:
    # BUSID  VID:PID    DEVICE                                                        STATE
    # 2-3    0cf3:9271  Qualcomm Atheros AR9271 Wireless Network Adapter              Not attached
    
    $usbipdList = usbipd list
    $adapterLine = $usbipdList | Where-Object { $_ -match $AdapterName } | Select-Object -First 1

    if (-not $adapterLine) {
        Write-Log "No USB adapter matching '$AdapterName' found! Is it plugged in?" "ERROR"
        Write-Log "Available devices:" "INFO"
        $usbipdList | ForEach-Object { Write-Log $_ "INFO" }
        exit 1
    }

    # Extract the BUSID (the first token)
    # The Regex matches the first contiguous string of non-whitespace characters
    $busId = ($adapterLine -split '\s+')[0]
    $deviceDesc = ($adapterLine -split '\s+', 3)[2]
    
    Write-Log "Found Target Adapter: $deviceDesc (BUSID: $busId)" "OK"

    # Check the current state of the adapter
    if ($adapterLine -match "Attached") {
        # It's already attached to a WSL instance
        Write-Log "Adapter (BUSID: $busId) is already attached to WSL." "OK"
        Write-Log "Aether interface environment ready." "OK"
        exit 0
    }

    # Step 4: Bind the adapter
    # Usually, it says "Not shared" or "Shared". If "Not shared", we bind it.
    if ($adapterLine -match "Not shared") {
        Write-Log "Binding adapter (BUSID: $busId) to usbipd..." "INFO"
        
        # NOTE: 'usbipd bind' requires administrator privileges.
        # Check if running as admin.
        $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        
        if (-not $isAdmin) {
            # Elevate using Start-Process to run bind command
            Write-Log "Binding requires Administrator privileges. Elevating..." "WARN"
            Start-Process pwsh -ArgumentList "-Command usbipd bind --busid $busId" -Verb RunAs -Wait
             
            # Re-verify bind success
            $verifyList = usbipd list
            $verifyLine = $verifyList | Where-Object { ($_ -match $busId) -and ($_ -match "Shared") } | Select-Object -First 1
            if (-not $verifyLine) {
                Write-Log "Failed to bind the adapter. Ensure you granted admin permissions." "ERROR"
                exit 1
            }
        }
        else {
            usbipd bind --busid $busId
            if ($LASTEXITCODE -ne 0) {
                Write-Log "Failed to bind adapter (BUSID: $busId)." "ERROR"
                exit 1
            }
        }
        Write-Log "Adapter bound successfully." "OK"
    }

    # Step 5: Attach to the WSL instance
    Write-Log "Attaching adapter (BUSID: $busId) to WSL instance '$WslHost'..." "INFO"
    usbipd attach --wsl $WslHost --busid $busId
    
    if ($LASTEXITCODE -eq 0) {
        Write-Log "Adapter successfully attached to WSL!" "OK"
        Write-Log "Aether interface environment ready." "OK"
        exit 0
    }
    else {
        Write-Log "Failed to attach adapter to WSL." "ERROR"
        Write-Log "Ensure the adapter is not actively being used by Windows and try again." "WARN"
        exit 1
    }

}
catch {
    Write-Log "An unexpected error occurred: $_" "ERROR"
    exit 1
}
