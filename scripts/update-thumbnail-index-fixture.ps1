# Refreshes crates/artwork-cache/tests/fixtures/snes-boxart-names.txt: the list
# of SNES Named_Boxarts file names (without ".png") in libretro-thumbnails, used
# by the offline boxart coverage test. Manual use only: run, review the diff,
# commit. Uses the unauthenticated GitHub API (60 requests/hour, 2 per run).
# File names only; no artwork is downloaded or committed.
$ErrorActionPreference = "Stop"

$RepoApi = "https://api.github.com/repos/libretro-thumbnails/Nintendo_-_Super_Nintendo_Entertainment_System"
$Headers = @{ Accept = "application/vnd.github+json" }

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Dest = Join-Path $RepoRoot "crates/artwork-cache/tests/fixtures/snes-boxart-names.txt"

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$Commit = (Invoke-RestMethod -Uri "$RepoApi/commits/master" -Headers $Headers).sha
$Tree = Invoke-RestMethod -Uri "$RepoApi/git/trees/$($Commit)?recursive=1" -Headers $Headers
if ($Tree.truncated) { throw "GitHub tree listing is truncated" }

# Paths look like "Named_Boxarts/<name>.png"; ordinal sort matches `LC_ALL=C sort`.
[string[]]$Names = $Tree.tree |
    Where-Object { $_.type -eq "blob" -and $_.path -match '^Named_Boxarts/[^/]+\.png$' } |
    ForEach-Object { $_.path.Substring("Named_Boxarts/".Length, $_.path.Length - "Named_Boxarts/".Length - ".png".Length) } |
    Select-Object -Unique
[Array]::Sort($Names, [StringComparer]::Ordinal)

$Date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd")
$Lines = @("# libretro-thumbnails SNES Named_Boxarts snapshot $Date commit $Commit") + $Names
# LF endings, UTF-8 without BOM: identical bytes to the .sh script.
[IO.File]::WriteAllText($Dest, ($Lines -join "`n") + "`n", (New-Object Text.UTF8Encoding $false))

Write-Host "wrote $($Names.Count) names to $Dest"
