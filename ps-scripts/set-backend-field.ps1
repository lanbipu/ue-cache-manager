param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$SectionName,
    [Parameter(Mandatory=$true)] [string]$NodeName,
    [Parameter(Mandatory=$true)] [string]$FieldName,
    [Parameter(Mandatory=$true)] [string]$FieldValue,
    [string]$Username, [string]$Password
)
$ErrorActionPreference = 'Stop'
$script = {
    param($FilePath, $SectionName, $NodeName, $FieldName, $FieldValue)
    if (-not (Test-Path -LiteralPath $FilePath)) { throw "file not found: $FilePath" }
    $lines = Get-Content -LiteralPath $FilePath
    $inSection = $false
    $handled = $false
    $out = New-Object System.Collections.Generic.List[string]
    foreach ($line in $lines) {
        $trim = $line.Trim()
        if ($trim.StartsWith('[') -and $trim.EndsWith(']')) {
            $inSection = ($trim.Trim('[',']') -ieq $SectionName)
            $out.Add($line); continue
        }
        if ($inSection -and -not $handled) {
            $eq = $line.IndexOf('=')
            if ($eq -gt 0) {
                $name = $line.Substring(0, $eq).Trim()
                $rest = $line.Substring($eq + 1).TrimStart()
                if (($name -ieq $NodeName) -and $rest.StartsWith('(') -and $rest.EndsWith(')')) {
                    $body = $rest.Substring(1, $rest.Length - 2)
                    $orderedKeys = New-Object System.Collections.Generic.List[string]
                    $fields = @{}
                    foreach ($pair in $body -split ',') {
                        $p = $pair.Trim()
                        if (-not $p) { continue }
                        $peq = $p.IndexOf('=')
                        if ($peq -lt 0) { continue }
                        $k = $p.Substring(0, $peq).Trim()
                        $v = $p.Substring($peq + 1).Trim()
                        if (-not $fields.ContainsKey($k)) { $orderedKeys.Add($k) }
                        $fields[$k] = $v
                    }
                    if ($fields.ContainsKey($FieldName)) {
                        $fields[$FieldName] = $FieldValue
                    } else {
                        $orderedKeys.Add($FieldName)
                        $fields[$FieldName] = $FieldValue
                    }
                    $parts = foreach ($k in $orderedKeys) { "$k=$($fields[$k])" }
                    $out.Add("$NodeName=($([string]::Join(', ', $parts)))")
                    $handled = $true
                    continue
                }
            }
        }
        $out.Add($line)
    }
    if (-not $handled) { throw "section [$SectionName] node $NodeName not found" }
    Set-Content -LiteralPath $FilePath -Value $out -Encoding UTF8
}
try {
    if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default `
            -ScriptBlock $script -ArgumentList $FilePath, $SectionName, $NodeName, $FieldName, $FieldValue
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script `
            -ArgumentList $FilePath, $SectionName, $NodeName, $FieldName, $FieldValue
    }
    @{ ok = $true; message = "set $NodeName.$FieldName=$FieldValue on $HostName" } | ConvertTo-Json -Compress
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
