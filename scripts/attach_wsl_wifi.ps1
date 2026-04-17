param (
    # The name of the WSL instance (default: kali-linux)
    [string]$WslHost = "kali-linux",

    # A substring of the USB WiFi adapter name as it appears in usbipd state output.
    [string]$AdapterName = "802.11",

    # Enable detailed logging
    [switch]$VerboseOutput
)

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

    function Get-UsbipdState {
        $stateJson = usbipd state
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to query usbipd state."
        }

        return $stateJson | ConvertFrom-Json
    }

    function Confirm-WslWirelessInterface {
        $driverLoaded = $false
        wsl.exe -d $WslHost --exec bash -lc "modprobe 8814au >/dev/null 2>&1" | Out-Null
        if ($LASTEXITCODE -eq 0) {
            $driverLoaded = $true
        }
        else {
            wsl.exe -d $WslHost --exec bash -lc "modprobe rtl8814au >/dev/null 2>&1" | Out-Null
            if ($LASTEXITCODE -eq 0) {
                $driverLoaded = $true
            }
        }

        if (-not $driverLoaded) {
            Write-Log "Unable to load the RTL8814AU driver inside WSL. The adapter may still be missing until the driver is built for the current kernel." "WARN"
        }

        $linkOutput = wsl.exe -d $WslHost --exec ip -o link show
        if ($LASTEXITCODE -ne 0) {
            Write-Log "Failed to enumerate Linux network interfaces inside WSL." "ERROR"
            exit 1
        }

        $wifiInterfaces = @(
            $linkOutput |
                Where-Object { $_ -match '^\d+:\s+((wlan|wlx)[^:]+):' } |
                ForEach-Object { $matches[1] }
        )

        if ($wifiInterfaces.Count -eq 0) {
            Write-Log "Adapter attached to WSL, but no Linux wireless interface was created. Ensure the 8814AU driver is built for the current WSL kernel." "ERROR"
            exit 1
        }

        Write-Log ("Detected WSL wireless interface(s): {0}" -f ($wifiInterfaces -join ", ")) "OK"
    }

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

    # Step 3: Find the USB WiFi adapter using machine-readable usbipd state
    Write-Log "Searching for USB WiFi adapter containing '$AdapterName'..." "VERBOSE"

    $usbipdState = Get-UsbipdState
    $adapter = $usbipdState.Devices | Where-Object {
        $_.Description -and $_.Description -match [regex]::Escape($AdapterName)
    } | Select-Object -First 1

    if (-not $adapter) {
        Write-Log "No USB adapter matching '$AdapterName' found! Is it plugged in?" "ERROR"
        Write-Log "Available devices:" "INFO"
        $usbipdState.Devices |
            Where-Object { $_.Description } |
            ForEach-Object { Write-Log ("{0} - {1}" -f ($_.BusId ?? "<detached>"), $_.Description) "INFO" }
        exit 1
    }

    $busId = $adapter.BusId
    $deviceDesc = $adapter.Description

    if (-not $busId) {
        Write-Log "Adapter '$deviceDesc' is not currently connected to a USB bus." "ERROR"
        exit 1
    }
    
    Write-Log "Found Target Adapter: $deviceDesc (BUSID: $busId)" "OK"

    # Check whether the adapter is already attached to a client.
    if ($adapter.ClientIPAddress) {
        Write-Log "Adapter (BUSID: $busId) is already attached to WSL." "OK"
        Confirm-WslWirelessInterface
        Write-Log "Aether interface environment ready." "OK"
        exit 0
    }

    # Step 4: Bind the adapter if needed.
    if (-not $adapter.PersistedGuid) {
        Write-Log "Binding adapter (BUSID: $busId) to usbipd..." "INFO"
        
        # NOTE: 'usbipd bind' requires administrator privileges.
        # Check if running as admin.
        $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        
        if (-not $isAdmin) {
            # Elevate using Start-Process to run bind command
            Write-Log "Binding requires Administrator privileges. Elevating..." "WARN"
            Start-Process pwsh -ArgumentList "-NoProfile", "-Command", "usbipd bind --busid $busId" -Verb RunAs -Wait
             
            # Re-verify bind success
            $verifyState = Get-UsbipdState
            $verifyAdapter = $verifyState.Devices | Where-Object { $_.BusId -eq $busId } | Select-Object -First 1
            if (-not $verifyAdapter -or -not $verifyAdapter.PersistedGuid) {
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
        Confirm-WslWirelessInterface
        Write-Log "Aether interface environment ready." "OK"
        exit 0
    }
    else {
        Write-Log "Failed to attach adapter to WSL." "ERROR"
        Write-Log "If usbipd reported a firewall issue, allow inbound/outbound TCP 3240 for WSL and any third-party firewall, then retry." "WARN"
        Write-Log "If the adapter keeps reconnecting in Device Manager, uninstall the conflicting flashrom/libusb driver for this Realtek adapter and let Windows restore the Realtek network driver." "WARN"
        exit 1
    }

}
catch {
    Write-Log "An unexpected error occurred: $_" "ERROR"
    exit 1
}
