Write-Host "Creating Python virtual environment..."
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install --upgrade pip
pip install -r requirements.txt
Write-Host "Venv creation complete."
