# Creates services/mlhost/.venv with the host's runtime dependencies.
# Pass -WithModel to also install llama-cpp-python for local inference.
param ([switch]$WithModel)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

python -m venv .venv
$python = Join-Path (Join-Path '.venv' 'Scripts') 'python.exe'
& $python -m pip install --upgrade pip
& $python -m pip install -r requirements.txt
if ($WithModel) { & $python -m pip install -r requirements-model.txt }

Write-Host 'Virtual environment ready. Run the host with: .venv\Scripts\python -m mlhost'
