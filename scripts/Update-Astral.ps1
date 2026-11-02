#requires -Version 7.0
<#
.SYNOPSIS
Verify a published Astral installer; optionally install it. Unsigned candidates are refused.
.DESCRIPTION
Publisher trust is supplied by the operator, never by the downloaded project or release.
No fallback to unsigned execution. Does not upload projects, settings or credentials.
#>
[CmdletBinding(SupportsShouldProcess=$true,ConfirmImpact='High')]
param(
 [Parameter(Mandatory=$true)][ValidatePattern('^v[0-9]+\.[0-9]+\.[0-9]+$')][string]$Tag,
 [Parameter(Mandatory=$true)][ValidatePattern('^[A-Fa-f0-9]{40}$')][string]$PublisherThumbprint,
 [switch]$Install,
 [switch]$AllowPrerelease
)
$ErrorActionPreference='Stop'
if (-not $IsWindows) { throw 'This updater supports Windows only.' }
$astralVersion=$Tag.Substring(1)
$astralName="Astral-$astralVersion-Setup-windows-x64.exe"
$astralRelease=Invoke-RestMethod -Uri "https://api.github.com/repos/nqtexd/astral/releases/tags/$Tag" -Headers @{'User-Agent'='Astral-verified-updater';Accept='application/vnd.github+json'}
if ($astralRelease.draft -or ($astralRelease.prerelease -and -not $AllowPrerelease)) { throw 'Draft/prerelease updates require explicit selection; drafts cannot be installed.' }
$astralAssets=@($astralRelease.assets | Where-Object name -CEQ $astralName)
if ($astralAssets.Count -ne 1) { throw 'Expected exactly one versioned Windows installer.' }
$astralAsset=$astralAssets[0]
if ($astralAsset.browser_download_url -cne "https://github.com/nqtexd/astral/releases/download/$Tag/$astralName") { throw 'Unexpected installer origin/path.' }
if ($astralAsset.digest -notmatch '^sha256:([a-f0-9]{64})$') { throw 'Release lacks a valid SHA-256 asset digest.' }
$astralExpected=$Matches[1]
if ($astralAsset.size -lt 1 -or $astralAsset.size -gt 536870912) { throw 'Installer size exceeds the 512 MiB update budget.' }
$astralTemporary=Join-Path ([IO.Path]::GetTempPath()) ('Astral-update-'+[guid]::NewGuid())
New-Item -ItemType Directory -Path $astralTemporary | Out-Null
try {
 $astralFile=Join-Path $astralTemporary $astralName
 # Bounded streaming avoids trusting Content-Length from a redirect/CDN.
 $astralHttp=[Net.Http.HttpClient]::new()
 try {
  $astralResponse=$astralHttp.GetAsync($astralAsset.browser_download_url,[Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
  $astralResponse.EnsureSuccessStatusCode() | Out-Null
  $astralInput=$astralResponse.Content.ReadAsStream()
  $astralOutput=[IO.File]::Create($astralFile)
  try {
   $astralBuffer=[byte[]]::new(1048576);$astralCount=0L
   while (($astralRead=$astralInput.Read($astralBuffer,0,$astralBuffer.Length)) -gt 0) {
    $astralCount+=$astralRead
    if ($astralCount -gt $astralAsset.size -or $astralCount -gt 536870912) { throw 'Installer exceeded its declared size.' }
    $astralOutput.Write($astralBuffer,0,$astralRead)
   }
  } finally { $astralOutput.Dispose();$astralInput.Dispose() }
 } finally { $astralHttp.Dispose() }
 if ((Get-Item -LiteralPath $astralFile).Length -ne $astralAsset.size -or (Get-FileHash -LiteralPath $astralFile -Algorithm SHA256).Hash.ToLowerInvariant() -cne $astralExpected) { throw 'Installer size/hash mismatch; update refused.' }
 $astralSignature=Get-AuthenticodeSignature -LiteralPath $astralFile
 if ($astralSignature.Status -ne 'Valid' -or $null -eq $astralSignature.SignerCertificate -or $astralSignature.SignerCertificate.Thumbprint -ine $PublisherThumbprint) { throw 'Installer is unsigned, invalid, or signed by an untrusted publisher; update refused.' }
 [pscustomobject]@{Tag=$Tag;SHA256=$astralExpected;Publisher=$astralSignature.SignerCertificate.Subject;Signature='Valid';Installed=$false}
 if ($Install -and $PSCmdlet.ShouldProcess("Astral $astralVersion",'Close Astral and install verified update')) {
  $astralProcess=Start-Process -FilePath $astralFile -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') -Wait -PassThru -WindowStyle Hidden
  if ($astralProcess.ExitCode -ne 0) { throw "Installer failed with exit code $($astralProcess.ExitCode). Keep the prior portable version and inspect the installer log." }
  Write-Output "Installed Astral $astralVersion. Projects and user preferences remain outside the installation directory."
 }
} finally {
 # Only this verified, generated child folder is removed.
 $astralResolved=[IO.Path]::GetFullPath($astralTemporary)
 $astralTempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
 if (-not $astralResolved.StartsWith($astralTempRoot,[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($astralResolved) -notmatch '^Astral-update-[a-f0-9-]{36}$') { throw 'Refusing unexpected cleanup path.' }
 Remove-Item -LiteralPath $astralResolved -Recurse -Force
}
