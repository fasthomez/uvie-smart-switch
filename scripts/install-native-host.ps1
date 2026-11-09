# Run in PowerShell as the same Windows user who runs Chrome/Edge and UVie.
param(
  [Parameter(Mandatory=$true)][string]$ExtensionId,
  [ValidateSet('Chrome','Edge','Brave')][string]$Browser='Chrome',
  [string]$HostExe=(Join-Path $PSScriptRoot '..\target\release\uvie-native-host.exe')
)
$ErrorActionPreference = 'Stop'
if ($ExtensionId -notmatch '^[a-p]{32}$') { throw 'Extension ID must be the 32-character ID shown in chrome://extensions or edge://extensions.' }
$exe = (Resolve-Path $HostExe).Path
if (-not $exe.EndsWith('.exe')) { throw 'Expected a Windows .exe native messaging host.' }
$hostName = 'com.fasthomez.uvie_smart_switch'
$dir = Join-Path $env:LOCALAPPDATA 'UVie\NativeMessaging'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$manifest = Join-Path $dir ("$Browser-$hostName.json")
@{
  name = $hostName
  description = 'UVie Smart Switch browser companion'
  path = $exe
  type = 'stdio'
  allowed_origins = @("chrome-extension://$ExtensionId/")
} | ConvertTo-Json -Depth 5 | Set-Content -Path $manifest -Encoding UTF8
# Chrome Native Messaging manifest requires UTF-8 JSON, preferably without BOM.
$text = Get-Content -Raw -Path $manifest
[System.IO.File]::WriteAllText($manifest, $text, [System.Text.UTF8Encoding]::new($false))
$vendor = switch ($Browser) {
  'Chrome' { 'Google\Chrome' }
  'Edge'   { 'Microsoft\Edge' }
  'Brave'  { 'BraveSoftware\Brave-Browser' }
}
$reg = "HKCU:\Software\$vendor\NativeMessagingHosts\$hostName"
New-Item -Path $reg -Force | Out-Null
Set-Item -Path $reg -Value $manifest
Write-Host "Registered $hostName for $Browser (Extension: $ExtensionId)" -ForegroundColor Green
Write-Host "Manifest: $manifest"
Write-Host 'Reload the extension in the browser to connect.'
