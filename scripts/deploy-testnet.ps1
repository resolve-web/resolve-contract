param(
  [Parameter(Mandatory = $true)][string]$SourceAccount,
  [Parameter(Mandatory = $true)][string]$SettlementTokenId,
  [string]$Network = "testnet",
  [string]$RpcUrl = "https://soroban-testnet.stellar.org",
  [string]$NetworkPassphrase = "Test SDF Network ; September 2015"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
  stellar contract build
  if ($LASTEXITCODE -ne 0) { throw "stellar contract build failed" }

  $wasm = Join-Path $repoRoot "target\wasm32v1-none\release\resolve.wasm"
  if (-not (Test-Path -LiteralPath $wasm)) { throw "Contract WASM was not produced at $wasm" }

  $contractId = (stellar contract deploy --wasm $wasm --source-account $SourceAccount --network $Network).Trim()
  if ($LASTEXITCODE -ne 0 -or $contractId -notmatch '^C[A-Z2-7]{55}$') { throw "Deployment did not return a valid contract ID" }

  $manifest = [ordered]@{
    network = $Network
    networkPassphrase = $NetworkPassphrase
    rpcUrl = $RpcUrl
    contractId = $contractId
    settlementTokenId = $SettlementTokenId
    deployedAt = (Get-Date).ToUniversalTime().ToString("o")
    wasmSha256 = (Get-FileHash -LiteralPath $wasm -Algorithm SHA256).Hash.ToLowerInvariant()
  }
  $outputDir = Join-Path $repoRoot "deployments"
  New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
  $output = Join-Path $outputDir "$Network.json"
  $manifest | ConvertTo-Json | Set-Content -LiteralPath $output -Encoding utf8
  Write-Host "Deployed $contractId and wrote $output"
} finally {
  Pop-Location
}
