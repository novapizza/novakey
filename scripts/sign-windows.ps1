<#
.SYNOPSIS
    Authenticode-sign Windows artifacts with signtool + Azure Trusted Signing.

.DESCRIPTION
    Signs one or more files (.exe/.dll/.msi/.msix) using signtool.exe with the
    Azure Trusted Signing dlib. No cert is stored locally — the signing key
    lives in your Azure Trusted Signing account; auth is via Azure Identity.

    Prerequisites:
      * Windows SDK (provides signtool.exe), or set env SIGNTOOL to its path.
      * Trusted Signing client (provides Azure.CodeSigning.Dlib.dll) — NuGet pkg,
        not a dotnet tool:
            nuget install Microsoft.Trusted.Signing.Client -OutputDirectory <dir>
        Set env TRUSTED_SIGNING_DLIB to the dll path, or let this script probe.
      * Azure auth for the signing service principal (DefaultAzureCredential):
            AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET
        (or a managed identity / az login on the build agent).

    Trusted Signing account details come from a metadata json — see
    scripts/trusted-signing-metadata.json. Override with -Metadata.

.EXAMPLE
    ./scripts/sign-windows.ps1 -Path build/NovaKey.exe

.EXAMPLE
    $env:AZURE_TENANT_ID='...'; $env:AZURE_CLIENT_ID='...'; $env:AZURE_CLIENT_SECRET='...'
    ./scripts/sign-windows.ps1 -Path dist/*.msi -Metadata scripts/trusted-signing-metadata.json
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string[]]$Path,

    [string]$Metadata = "$PSScriptRoot/trusted-signing-metadata.json",

    # Azure Trusted Signing public timestamp authority.
    [string]$TimestampUrl = 'http://timestamp.acs.microsoft.com'
)

$ErrorActionPreference = 'Stop'

function Resolve-Signtool {
    if ($env:SIGNTOOL -and (Test-Path $env:SIGNTOOL)) { return $env:SIGNTOOL }
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    # Probe the Windows Kits x64 bin, newest SDK first.
    $roots = @("${env:ProgramFiles(x86)}\Windows Kits\10\bin", "${env:ProgramFiles}\Windows Kits\10\bin")
    foreach ($root in $roots) {
        if (Test-Path $root) {
            $found = Get-ChildItem -Path $root -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match '\\x64\\' } |
                Sort-Object FullName -Descending | Select-Object -First 1
            if ($found) { return $found.FullName }
        }
    }
    throw "signtool.exe not found. Install the Windows SDK or set `$env:SIGNTOOL."
}

function Resolve-Dlib {
    if ($env:TRUSTED_SIGNING_DLIB -and (Test-Path $env:TRUSTED_SIGNING_DLIB)) { return $env:TRUSTED_SIGNING_DLIB }
    $probe = @(
        "$env:USERPROFILE\.dotnet\tools\.store\microsoft.trusted.signing.client",
        "$env:USERPROFILE\.nuget\packages\microsoft.trusted.signing.client"
    )
    foreach ($p in $probe) {
        if (Test-Path $p) {
            $found = Get-ChildItem -Path $p -Recurse -Filter Azure.CodeSigning.Dlib.dll -ErrorAction SilentlyContinue |
                Sort-Object FullName -Descending | Select-Object -First 1
            if ($found) { return $found.FullName }
        }
    }
    throw "Azure.CodeSigning.Dlib.dll not found. Install Microsoft.Trusted.Signing.Client or set `$env:TRUSTED_SIGNING_DLIB."
}

if (-not (Test-Path $Metadata)) { throw "Metadata file not found: $Metadata" }

$signtool = Resolve-Signtool
$dlib     = Resolve-Dlib
$files    = $Path | ForEach-Object { Get-Item -Path $_ } | Select-Object -ExpandProperty FullName

Write-Host "signtool : $signtool"
Write-Host "dlib     : $dlib"
Write-Host "metadata : $Metadata"
Write-Host "files    : $($files -join ', ')"

$signArgs = @(
    'sign',
    '/v',
    '/fd', 'SHA256',
    '/tr', $TimestampUrl,
    '/td', 'SHA256',
    '/dlib', $dlib,
    '/dmdf', $Metadata
) + $files

Write-Host "▶ Signing..."
& $signtool @signArgs
if ($LASTEXITCODE -ne 0) { throw "signtool sign failed (exit $LASTEXITCODE)" }

Write-Host "▶ Verifying..."
foreach ($f in $files) {
    & $signtool verify /pa /v $f
    if ($LASTEXITCODE -ne 0) { throw "signtool verify failed for $f (exit $LASTEXITCODE)" }
}
Write-Host "✓ Signed + verified: $($files.Count) file(s)"
