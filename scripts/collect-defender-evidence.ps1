# Bounded, read-only Defender evidence collector for quotabar support.
# Reads only: Get-MpComputerStatus (version/engine/real-time status) and
# Get-WinEvent Defender Operational IDs 1116/1117 within -Days.
# No scans, downloads, protection changes, binary execution, or uploads.
# Output is allowlisted/redacted JSON; failures record command + exception TYPE only.
[CmdletBinding()]
param(
    [ValidateRange(1, 30)][int]$Days = 7,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'defender-evidence-lib.ps1')

# User-explicit destination only: refuse before any Defender cmdlets run.
# FileMode.CreateNew at write time still guards against a race where the file
# appears between this check and the write.
$resolved = $PSCmdlet.GetUnresolvedProviderPathFromPSPath($OutputPath)
if (Test-Path -LiteralPath $resolved) {
    throw "OutputPath '$resolved' already exists; refusing to overwrite. Choose a new destination."
}
$parent = Split-Path -Parent $resolved
if ($parent -and -not (Test-Path -LiteralPath $parent)) {
    [void][System.IO.Directory]::CreateDirectory($parent)
}

$errors = New-Object System.Collections.Generic.List[object]

# 1) Status allowlist: engine/real-time only. Failure -> command + exception type.
$status = $null
try {
    $s = Get-MpComputerStatus -ErrorAction Stop
    $status = [ordered]@{
        engineVersion          = if ($s.PSObject.Properties['AMEngineVersion']) { [string]$s.AMEngineVersion } else { $null }
        intelligenceVersion    = if ($s.PSObject.Properties['AntivirusSignatureVersion']) { [string]$s.AntivirusSignatureVersion } else { $null }
        realTimeProtectionEnabled = [bool]$s.RealTimeProtectionEnabled
    }
}
catch {
    $errors.Add([ordered]@{ command = 'Get-MpComputerStatus'; error = $_.Exception.GetType().FullName })
}

# 2) Events: Defender Operational 1116/1117 within Days, filtered to quotabar.
$events = @()
try {
    $rawEvents = Get-WinEvent -FilterHashtable @{
        LogName   = 'Microsoft-Windows-Windows Defender/Operational'
        Id        = @(1116, 1117)
        StartTime = [datetime]::UtcNow.AddDays(-$Days)
    } -ErrorAction Stop

    foreach ($evt in @($rawEvents)) {
        try {
            $xml = $evt.ToXml()
            if (-not (Test-DefenderEventMentionsQuotaBar -Xml $xml)) { continue }
            $events += ConvertFrom-DefenderEventXml -Xml $xml -TimestampFallback $evt.TimeCreated
        }
        catch {
            $errors.Add([ordered]@{ command = 'ConvertFrom-DefenderEventXml'; error = $_.Exception.GetType().FullName })
        }
    }
}
catch {
    $errors.Add([ordered]@{ command = 'Get-WinEvent'; error = $_.Exception.GetType().FullName })
}

# 3) Report: UTF-8 (no BOM), arrays preserved, explicit conclusion.
# FileMode.CreateNew: fails atomically if another process created the file meanwhile.
$report = New-DefenderEvidenceReport -Status $status -Events $events -Errors $errors
$json = ConvertTo-Json -InputObject $report -Depth 6

$fs = $null
try {
    $fs = New-Object System.IO.FileStream($resolved, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write)
}
catch {
    throw "Failed to create output file at '$resolved' (write failed; destination may already exist). Output left unwritten."
}
try {
    $sw = New-Object System.IO.StreamWriter($fs, (New-Object System.Text.UTF8Encoding($false)))
    try { $sw.Write($json) }
    finally { $sw.Dispose() }
}
finally { if ($fs) { $fs.Dispose() } }

Write-Output "Evidence written to: $resolved (events=$($report['events'].Count) errors=$($report['errors'].Count))"
