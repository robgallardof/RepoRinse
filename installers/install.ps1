[CmdletBinding()]
param(
    [string]$Binary = (Join-Path $PSScriptRoot '..\reporinse.exe'),
    [string]$Destination = (Join-Path $env:LOCALAPPDATA 'RepoRinse\bin'),
    [switch]$AddToPath
)
$ErrorActionPreference = 'Stop'
$source = (Resolve-Path -LiteralPath $Binary).Path
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw 'Binary not found' }
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
Copy-Item -LiteralPath $source -Destination (Join-Path $Destination 'reporinse.exe') -Force
if ($AddToPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $Destination) {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$Destination", 'User')
    }
}
Write-Output "Installed to $Destination. Install project hooks with reporinse install --provider git --all --yes."
