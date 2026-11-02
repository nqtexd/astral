param([Parameter(Mandatory=$true)][string]$IsccPath,[string]$CertificateThumbprint,[switch]$Production)
$ErrorActionPreference='Stop'
$astralRoot=Split-Path -Parent $PSScriptRoot
$astralManifest=Get-Content -LiteralPath (Join-Path $astralRoot 'Cargo.toml') -Raw
$astralVersion=[regex]::Match($astralManifest,'version\s*=\s*"([0-9.]+)"').Groups[1].Value
$astralPackage=Join-Path $astralRoot "release/Astral-$astralVersion-windows-x64"
if (-not (Test-Path -LiteralPath $IsccPath -PathType Leaf)) { throw 'Provide the staged Inno Setup 7.1.1 compiler (ISCC.exe).' }
if (-not (Test-Path -LiteralPath (Join-Path $astralPackage 'Astral-Studio.exe'))) { throw 'Build and verify the portable package first.' }
if ($Production -and -not $CertificateThumbprint) { throw 'Production installers require an owned code-signing certificate; unsigned builds remain candidates.' }
function Sign-AstralArtifact([string]$Path) {
    if (-not $CertificateThumbprint) { return }
    $astralCertificate=Get-Item -LiteralPath "Cert:\CurrentUser\My\$CertificateThumbprint"
    if (-not $astralCertificate.HasPrivateKey -or $astralCertificate.NotAfter -lt (Get-Date)) { throw 'Signing certificate is expired or lacks a private key.' }
    $astralSignature=Set-AuthenticodeSignature -LiteralPath $Path -Certificate $astralCertificate -HashAlgorithm SHA256 -TimestampServer 'http://timestamp.digicert.com'
    if ($astralSignature.Status -ne 'Valid') { throw "Signing failed: $($astralSignature.Status)" }
}
# Runtime binaries retain their upstream signatures. Sign only Astral-owned binaries.
Sign-AstralArtifact (Join-Path $astralPackage 'Astral-Studio.exe')
Sign-AstralArtifact (Join-Path $astralPackage 'astral.exe')
if ($CertificateThumbprint) {
    Get-ChildItem -LiteralPath $astralPackage -Recurse -File | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object FullName | ForEach-Object { "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $([IO.Path]::GetRelativePath($astralPackage,$_.FullName).Replace('\','/'))" } | Set-Content -LiteralPath (Join-Path $astralPackage 'SHA256SUMS.txt') -Encoding utf8
}
& $IsccPath "/DAppVersion=$astralVersion" "/DPackageDir=$astralPackage" (Join-Path $astralRoot 'platforms/windows/Astral.iss')
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
$astralInstaller=Join-Path $astralRoot "release/Astral-$astralVersion-Setup-windows-x64.exe"
Sign-AstralArtifact $astralInstaller
Get-FileHash -LiteralPath $astralInstaller -Algorithm SHA256
if (-not $CertificateThumbprint) { Write-Output 'Unsigned installer candidate: clean VM install/update/uninstall and signing gates remain pending.' }
