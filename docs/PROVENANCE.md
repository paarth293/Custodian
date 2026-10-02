# Session Provenance and Review

## Entire Checkpoints
This repository has Entire checkpoints enabled. Every commit to main and active feature branches automatically records a session checkpoint.
These checkpoints capture the full context of the agent's work, providing an auditable trail of how decisions were made and code was written.

## DevRelay Session Export
To export a session for the final DEV post:
1. Open the DevRelay dashboard.
2. Select the relevant session.
3. Click "Export as Markdown".
4. Embed the generated markdown link directly into the DEV post to demonstrate the agent's work.

## GitHub Copilot Code Review
GitHub Copilot is configured as an **additional** automated reviewer for all pull requests in this repository. 
Copilot will automatically review PRs for security, performance, and best practices. Its review supplements human review, but does not replace it.
