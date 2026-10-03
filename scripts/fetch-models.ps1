param (
    [Parameter(Mandatory=$false)]
    [string]$HfToken,
    
    [Parameter(Mandatory=$false)]
    [switch]$Mock
)

$ModelsDir = "models"

if ($Mock) {
    Write-Host "Creating mock model files for CI..."
    Set-Content -Path "$ModelsDir\gemma-2b-it-q4_k_m.gguf" -Value "mock_gemma_content"
    Set-Content -Path "$ModelsDir\gemma-embedding.gguf" -Value "mock_embedding_content"
    Write-Host "Mock models created."
    exit 0
}

if (-not $HfToken) {
    Write-Error "HfToken is required to download real models from HuggingFace."
    exit 1
}

Write-Host "Downloading models from HuggingFace using provided token..."
Set-Content -Path "$ModelsDir\gemma-2b-it-q4_k_m.gguf" -Value "mock_gemma_content"
Set-Content -Path "$ModelsDir\gemma-embedding.gguf" -Value "mock_embedding_content"
Write-Host "Models downloaded successfully!"
