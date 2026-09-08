# Pure helper library for Defender evidence collection. No cmdlet side effects.
# Dot-sourced by collect-defender-evidence.ps1 and test-defender-evidence.ps1.
# Real Defender event XML (EventRecord.ToXml()) carries the default namespace
# "http://schemas.microsoft.com/win/2004/08/events/event"; XPath must account for it.
Set-StrictMode -Version 2.0

$script:DefenderEventNamespace = 'http://schemas.microsoft.com/win/2004/08/events/event'

function Get-DefenderEventData {
    # Parses a Windows Event log XML string (Defender Operational 1116/1117 shape),
    # with or without the default event namespace. Hardened: DTD prohibited, no resolver.
    # Throws on malformed XML or DTD attempts. Returns Data hashtable, EventId, SystemTime.
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Xml
    )

    $settings = New-Object System.Xml.XmlReaderSettings
    $settings.DtdProcessing = [System.Xml.DtdProcessing]::Prohibit
    $settings.XmlResolver = $null

    $doc = New-Object System.Xml.XmlDocument
    $sr = New-Object System.IO.StringReader($Xml)
    try {
        $reader = [System.Xml.XmlReader]::Create($sr, $settings)
        try { $doc.Load($reader) }
        finally { if ($reader) { $reader.Dispose() } }
    }
    finally { $sr.Dispose() }

    # Namespace-aware XPath: use a mapped prefix when the doc has a (default) namespace.
    $nsmgr = New-Object System.Xml.XmlNamespaceManager($doc.NameTable)
    $p = ''
    if ($doc.DocumentElement.NamespaceURI) {
        $nsmgr.AddNamespace('e', $doc.DocumentElement.NamespaceURI)
        $p = 'e:'
    }

    $data = @{}
    $nodes = $doc.SelectNodes("/${p}Event/${p}EventData/${p}Data", $nsmgr)
    if ($nodes) {
        foreach ($n in $nodes) {
            $nameAttr = $n.Attributes['Name']
            if ($nameAttr -and -not $data.ContainsKey($nameAttr.Value)) {
                $data[$nameAttr.Value] = $n.InnerText
            }
        }
    }

    $eventIdNode = $doc.SelectSingleNode("/${p}Event/${p}System/${p}EventID", $nsmgr)
    $timeNode = $doc.SelectSingleNode("/${p}Event/${p}System/${p}TimeCreated", $nsmgr)
    $systemTime = $null
    if ($timeNode) {
        $attr = $timeNode.Attributes['SystemTime']
        if ($attr) { $systemTime = $attr.Value }
    }

    [pscustomobject]@{
        Data       = $data
        EventId    = if ($eventIdNode) { [int]$eventIdNode.InnerText } else { $null }
        SystemTime = $systemTime
    }
}

function Test-DefenderEventMentionsQuotaBar {
    # True only when the Path field mentions 'quotabar' (case-insensitive).
    # Process Name and other fields are deliberately ignored per spec.
    # Throws on malformed XML so the collector records the failure.
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Xml
    )

    $parsed = Get-DefenderEventData -Xml $Xml
    return ($parsed.Data.Contains('Path') -and $parsed.Data['Path'] -match '(?i)quotabar')
}

function ConvertFrom-DefenderEventXml {
    # Converts one Defender event XML into the allowlisted, redacted event object.
    # Emits ONLY: timestamp, eventId, threatName, detectionTime, actionName,
    # engineVersion, intelligenceVersion, mentionsRunKey, mentionsExe, mentionsArchivedBeta2.
    # Classification booleans derive from the Path field ONLY.
    # Historical events missing engine/intelligence versions stay null; current
    # status values are NEVER substituted into past events.
    # Throws on malformed XML.
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Xml,
        [datetime]$TimestampFallback
    )

    $parsed = Get-DefenderEventData -Xml $Xml
    $data = $parsed.Data

    $timestamp = $null
    if ($parsed.SystemTime) {
        $timestamp = ([datetime]$parsed.SystemTime).ToUniversalTime().ToString('o')
    }
    elseif ($TimestampFallback) {
        $timestamp = $TimestampFallback.ToUniversalTime().ToString('o')
    }

    $pathValue = ''
    if ($data.Contains('Path')) { $pathValue = $data['Path'] }

    [pscustomobject][ordered]@{
        timestamp            = $timestamp
        eventId              = $parsed.EventId
        threatName           = if ($data.Contains('Threat Name')) { $data['Threat Name'] } else { $null }
        detectionTime        = if ($data.Contains('Detection Time')) { $data['Detection Time'] } else { $null }
        actionName           = if ($data.Contains('Action Name')) { $data['Action Name'] } else { $null }
        engineVersion        = if ($data.Contains('Engine Version')) { $data['Engine Version'] } else { $null }
        intelligenceVersion  = if ($data.Contains('Security Intelligence Version')) { $data['Security Intelligence Version'] } else { $null }
        mentionsRunKey       = [bool]($pathValue -match '(?i)currentversion\\run')
        mentionsExe          = [bool]($pathValue -match '(?i)\.exe')
        mentionsArchivedBeta2 = [bool]($pathValue -match '(?i)(?:^|[\\/])releases[\\/]+v0\.4\.0-beta\.2[\\/]')
    }
}

function New-DefenderEvidenceReport {
    # Builds the final report object. Arrays remain arrays at count 0 and 1.
    # Empty events explicitly yield an 'undetermined' conclusion, never 'safe'.
    param(
        $Status,
        $Events,
        $Errors
    )

    $events = @($Events | Where-Object { $_ })
    $errors = @($Errors | Where-Object { $_ })

    $conclusion = 'undetermined: no quotabar-related Defender events in the requested window; absence of events is not evidence of safety'
    if ($events.Count -gt 0) {
        $conclusion = 'undetermined: quotabar-related Defender events found; classification booleans provided, verdict requires human review'
    }

    [ordered]@{
        schemaVersion = '1'
        collectedAt   = [datetime]::UtcNow.ToString('o')
        status        = $Status
        events        = $events
        errors        = $errors
        conclusion    = $conclusion
    }
}
