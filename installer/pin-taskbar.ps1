[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ShortcutPath)

$ErrorActionPreference = 'SilentlyContinue'

if (-not (Test-Path -LiteralPath $ShortcutPath)) {
    exit 0
}

$shell = New-Object -ComObject Shell.Application
$folderPath = Split-Path -Parent $ShortcutPath
$shortcutName = Split-Path -Leaf $ShortcutPath
$folder = $shell.Namespace($folderPath)
if ($null -eq $folder) {
    exit 0
}

$item = $folder.ParseName($shortcutName)
if ($null -eq $item) {
    exit 0
}

foreach ($verb in $item.Verbs()) {
    $name = ($verb.Name -replace '&', '').Trim()
    if ($name -match 'Pin to taskbar') {
        $verb.DoIt()
        Start-Sleep -Milliseconds 500
        exit 0
    }
}

exit 0
