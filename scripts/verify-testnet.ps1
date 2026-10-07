param([string]$Manifest = "deployments/testnet.json")

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $repoRoot $Manifest
if (-not (Test-Path -LiteralPath $manifestPath)) { throw "Missing deployment manifest: $manifestPath" }
$deployment = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($deployment.contractId -notmatch '^C[A-Z2-7]{55}$') { throw "Manifest contract ID is invalid" }

$nextId = stellar contract invoke `
  --id $deployment.contractId `
  --network $deployment.network `
  -- `
  next_market_id
if ($LASTEXITCODE -ne 0) { throw "Contract read smoke test failed" }
Write-Host "Resolve contract $($deployment.contractId) is reachable; next market id: $nextId"
