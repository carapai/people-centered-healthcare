# Registers facility-edge as a Windows service (facility-edge.md § OS service). The MSI does the
# same; CI's edge-build workflow uses this script for its service smoke test.
#
#   ./install-service.ps1 -Binary 'C:\Program Files\Health\facility-edge.exe' `
#       -Environment 'HEALTH_DATA_DIR=C:\ProgramData\Health\store', 'HEALTH_UPDATE_KEY_FILE=…'
param(
    [Parameter(Mandatory)] [string] $Binary,
    # The service's environment, one NAME=value per entry (secrets go in files, never here).
    [Parameter(Mandatory)] [string[]] $Environment
)
$ErrorActionPreference = 'Stop'
$name = 'facility-edge'

function Invoke-Sc {
    & sc.exe @args | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "sc.exe $args failed with $LASTEXITCODE" }
}

New-Service -Name $name -BinaryPathName "`"$Binary`" --service" `
    -DisplayName 'Health Facility Edge Node' -StartupType Automatic | Out-Null
Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\$name" `
    -Name Environment -Type MultiString -Value $Environment
# Restart, restart, restart, then stay stopped and alert; the count resets after 24 hours.
Invoke-Sc failure $name reset= 86400 actions= restart/5000/restart/5000/restart/5000
# A non-zero exit code (a service-specific error) is a failure too, not only a crash.
Invoke-Sc failureflag $name 1
