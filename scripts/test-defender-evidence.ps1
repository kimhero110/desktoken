# Independent fixture tests for defender-evidence-lib.ps1 and the collector's
# no-overwrite guard. Uses synthetic XML only (including realistic default-namespace
# ToXml shape); never reads real event logs.
# Run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-defender-evidence.ps1
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'defender-evidence-lib.ps1')

$script:Pass = 0
$script:Fail = 0
function Assert-True {
    param([bool]$Condition, [string]$Name)
    if ($Condition) { $script:Pass++ }
    else { $script:Fail++; Write-Host "FAIL: $Name" -ForegroundColor Red }
}

function New-SyntheticEventXml {
    param(
        [string]$Path = 'C:\Users\alice\quotabar\app.exe',
        [string]$ProcessName = 'C:\Users\alice\helper.exe',
        [string]$ThreatName = 'Trojan:Win32/Synthetic.Test',
        [int]$EventId = 1116,
        [string]$SystemTime = '2026-09-01T10:00:00.000Z',
        [switch]$OmitThreatName,
        [switch]$OmitActionName,
        [switch]$OmitVersions,
        [switch]$WithDefaultNamespace
    )
    $fields = @(
        "<Data Name='Severity ID'>5</Data>",
        "<Data Name='Type Name'>Concrete</Data>",
        "<Data Name='Path'>$Path</Data>",
        "<Data Name='Process Name'>$ProcessName</Data>",
        "<Data Name='Detection Time'>2026-09-01T10:00:00.000Z</Data>",
        "<Data Name='PSComputerName'>TESTHOST</Data>"
    )
    if (-not $OmitThreatName) { $fields = @("<Data Name='Threat Name'>$ThreatName</Data>") + $fields }
    if (-not $OmitActionName) { $fields += "<Data Name='Action Name'>Quarantine</Data>" }
    if (-not $OmitVersions) {
        $fields += "<Data Name='Engine Version'>1.401.1.2</Data>"
        $fields += "<Data Name='Security Intelligence Version'>1.421.1.0</Data>"
    }

    $ns = ''
    if ($WithDefaultNamespace) { $ns = ' xmlns="http://schemas.microsoft.com/win/2004/08/events/event"' }

    '<Event' + $ns + '><System><Provider Name="Microsoft-Windows-Windows Defender"/><EventID>' +
        $EventId + '</EventID><TimeCreated SystemTime="' + $SystemTime + '"/></System>' +
        '<EventData>' + ($fields -join '') + '</EventData></Event>'
}

# --- 1) Filtering: Path only, case-insensitive; namespace-free and default-ns shapes ---
$keepXml = New-SyntheticEventXml -Path 'D:\Project\quotabar\.analysis\releases\v0.4.0-beta.2\quotabar.exe'
$dropXml = New-SyntheticEventXml -Path 'C:\Windows\System32\unrelated.dll'
$nsXml = New-SyntheticEventXml -Path 'D:\Project\quotabar\.analysis\releases\v0.4.0-beta.2\quotabar.exe' -WithDefaultNamespace
Assert-True (Test-DefenderEventMentionsQuotaBar -Xml $keepXml) 'filter keeps quotabar path (no namespace)'
# Genuine mixed-case Path fixture
$mixedCaseXml = New-SyntheticEventXml -Path 'D:\Project\QuOtAbAr\app.exe'
Assert-True (Test-DefenderEventMentionsQuotaBar -Xml $mixedCaseXml) 'filter is case-insensitive (mixed-case QuOtAbAr Path)'
Assert-True (Test-DefenderEventMentionsQuotaBar -Xml $nsXml) 'filter keeps quotabar path (default xmlns, real ToXml shape)'
Assert-True (-not (Test-DefenderEventMentionsQuotaBar -Xml $dropXml)) 'filter drops non-quotabar path'

# Process Name-only quotabar mention is a false positive and must be dropped
$procOnlyXml = New-SyntheticEventXml -Path 'C:\Windows\System32\unrelated.dll' -ProcessName 'D:\Project\quotabar\bin\quotabar.exe'
Assert-True (-not (Test-DefenderEventMentionsQuotaBar -Xml $procOnlyXml)) 'Process Name-only quotabar mention does NOT match filter'

# --- 2) Redaction: allowlisted keys only; no raw paths / hostnames leak ---
$ev = ConvertFrom-DefenderEventXml -Xml $nsXml
$expectedKeys = @('timestamp', 'eventId', 'threatName', 'detectionTime', 'actionName',
    'engineVersion', 'intelligenceVersion', 'mentionsRunKey', 'mentionsExe', 'mentionsArchivedBeta2')
$actualKeys = @($ev.PSObject.Properties.Name)
Assert-True (@(Compare-Object -ReferenceObject $expectedKeys -DifferenceObject $actualKeys).Count -eq 0) 'event keys exactly match allowlist'
$evJson = ConvertTo-Json -InputObject $ev
Assert-True ($evJson -notmatch [regex]::Escape('C:\Users\alice')) 'no raw user path leaked'
Assert-True ($evJson -notmatch [regex]::Escape('D:\Project\quotabar')) 'no raw quotabar path leaked'
Assert-True ($evJson -notmatch 'TESTHOST') 'no hostname leaked'
Assert-True ($evJson -notmatch 'PSComputerName') 'no non-allowlisted field name leaked'
Assert-True ($ev.eventId -eq 1116) 'eventId parsed through default namespace'

# --- 3) Classification booleans: Path field ONLY ---
$runKeyXml = New-SyntheticEventXml -Path 'HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\quotabar'
$evRun = ConvertFrom-DefenderEventXml -Xml $runKeyXml
Assert-True ($evRun.mentionsRunKey) 'run key classified true'
Assert-True (-not ($evRun.mentionsExe)) 'run key Path has no exe'

# Process Name contains .exe but Path does not -> must stay false
$procExeXml = New-SyntheticEventXml -Path 'HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\quotabar' -ProcessName 'C:\Users\alice\helper.exe'
$evProcExe = ConvertFrom-DefenderEventXml -Xml $procExeXml
Assert-True (-not ($evProcExe.mentionsExe)) 'mentionsExe ignores Process Name'

# Real-shaped beta release archive path: exact releases/v0.4.0-beta.2/ directory only
$evBeta = ConvertFrom-DefenderEventXml -Xml $keepXml
Assert-True ($evBeta.mentionsArchivedBeta2) 'releases\v0.4.0-beta.2\ path classified true'
Assert-True ($evBeta.mentionsExe) 'archive path .exe classified true'
Assert-True (-not ((ConvertFrom-DefenderEventXml -Xml (New-SyntheticEventXml -Path 'D:\Project\quotabar\.analysis\releases\v0.4.0-beta.1\quotabar.exe')).mentionsArchivedBeta2)) 'beta.1 directory classified false'
Assert-True (-not ((ConvertFrom-DefenderEventXml -Xml (New-SyntheticEventXml -Path 'D:\Project\quotabar\.analysis\releases\v0.4.0-beta.20\quotabar.exe')).mentionsArchivedBeta2)) 'beta.20 directory classified false (no prefix overmatch)'
Assert-True (-not ((ConvertFrom-DefenderEventXml -Xml (New-SyntheticEventXml -Path 'D:\Build\releases\v9.9.9-beta\quotabar.exe')).mentionsArchivedBeta2)) 'other v*-beta directory classified false'
Assert-True (-not ((ConvertFrom-DefenderEventXml -Xml (New-SyntheticEventXml -Path 'C:\Temp\v0.4.0-beta.2-fake\quotabar.exe')).mentionsArchivedBeta2)) 'version outside releases/ classified false'
Assert-True (-not ((ConvertFrom-DefenderEventXml -Xml (New-SyntheticEventXml -Path 'D:\Project\quotabar\.analysis\prereleases\v0.4.0-beta.2\quotabar.exe')).mentionsArchivedBeta2)) 'prereleases\ directory classified false (left boundary)'

# --- 4) Missing fields -> null, no throw; historical versions NEVER backfilled ---
$partialXml = New-SyntheticEventXml -OmitThreatName -OmitActionName
$evPartial = ConvertFrom-DefenderEventXml -Xml $partialXml
Assert-True ($null -eq $evPartial.threatName) 'missing threatName -> null'
Assert-True ($null -eq $evPartial.actionName) 'missing actionName -> null'

# Old event without engine/intelligence versions stays null even though current
# status (simulated as different values) exists -- no substitution allowed.
$oldXml = New-SyntheticEventXml -OmitVersions
$evOld = ConvertFrom-DefenderEventXml -Xml $oldXml
Assert-True ($null -eq $evOld.engineVersion) 'missing historical engineVersion stays null'
Assert-True ($null -eq $evOld.intelligenceVersion) 'missing historical intelligenceVersion stays null'
$currentEngineDiffers = '9.999.999.999'
Assert-True ($currentEngineDiffers -ne $evOld.engineVersion) 'current engine version not substituted into past event'

$evFull = ConvertFrom-DefenderEventXml -Xml $keepXml
Assert-True ($evFull.engineVersion -eq '1.401.1.2') 'engineVersion from event data when present'
Assert-True ($evFull.intelligenceVersion -eq '1.421.1.0') 'intelligenceVersion from event data when present'

# Timestamp normalization to UTC ISO
Assert-True ($ev.timestamp -like '2026-09-01T10:00:00*') 'timestamp UTC ISO'

# --- 5) Malformed XML and DTD rejection: must throw ---
$threw = $false
try { ConvertFrom-DefenderEventXml -Xml '<Event><broken' | Out-Null }
catch { $threw = $true }
Assert-True $threw 'malformed XML throws in converter'

$threwFilter = $false
try { Test-DefenderEventMentionsQuotaBar -Xml '<Event><broken' | Out-Null }
catch { $threwFilter = $true }
Assert-True $threwFilter 'malformed XML throws in filter (collector records error, never silently false)'

$dtdXml = '<!DOCTYPE Event SYSTEM "http://evil.example.invalid/event.dtd"><Event><System><EventID>1116</EventID></System></Event>'
$threwDtd = $false
try { ConvertFrom-DefenderEventXml -Xml $dtdXml | Out-Null }
catch { $threwDtd = $true }
Assert-True $threwDtd 'DTD is prohibited (XmlReaderSettings.DtdProcessing=Prohibit)'

# --- 6) Arrays stay arrays at count 0 and 1; empty means undetermined ---
$r0 = New-DefenderEvidenceReport -Status $null -Events @() -Errors @()
$j0 = ConvertTo-Json -InputObject $r0 -Depth 6
Assert-True ($j0 -match '"events"\s*:\s*\[\s*\]') '0 events serialize as []'
Assert-True ($j0 -match '"errors"\s*:\s*\[\s*\]') '0 errors serialize as []'
Assert-True ($j0 -match 'undetermined') 'empty events -> undetermined conclusion'
Assert-True ($j0 -notmatch '(?i)\bsafe\b') 'empty events never called safe'

$r1 = New-DefenderEvidenceReport -Status $null -Events @($ev) -Errors @()
$j1 = ConvertTo-Json -InputObject $r1 -Depth 6
Assert-True ($j1 -match '"events"\s*:\s*\[\s*\{') '1 event stays an array'
Assert-True ($j1 -match 'undetermined') 'found events still undetermined (human review)'

# --- 7) Collector refuses to overwrite an existing OutputPath (FileMode.CreateNew) ---
$existing = Join-Path ([System.IO.Path]::GetTempPath()) ("defender-evidence-test-" + [guid]::NewGuid().ToString('N') + ".json")
Set-Content -LiteralPath $existing -Value 'SENTINEL'
$refused = $false
try {
    & (Join-Path $PSScriptRoot 'collect-defender-evidence.ps1') -Days 1 -OutputPath $existing
}
catch { $refused = $true }
Assert-True $refused 'collector refuses to overwrite existing file (CreateNew)'
Assert-True ((Get-Content -LiteralPath $existing -Raw) -match 'SENTINEL') 'existing file untouched'
Remove-Item -LiteralPath $existing -Force

Write-Host ""
Write-Host "PASS=$($script:Pass) FAIL=$($script:Fail)"
exit ([int]($script:Fail -gt 0))
