#!/usr/bin/env bash
set -e

# Ensure act is installed
if ! command -v act &> /dev/null; then
    echo "act is not installed. Installing to ~/.local/bin..."
    mkdir -p ~/.local/bin
    curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | bash -s -- -b ~/.local/bin
    export PATH="$HOME/.local/bin:$PATH"
fi

# Ensure Docker is running (act requires it)
if ! docker info >/dev/null 2>&1; then
    echo "❌ Error: Docker is not running or not accessible. Please start Docker before running this."
    exit 1
fi

echo "🚀 Running CI locally via act..."

# We run the lint, test, and Linux build jobs explicitly. 
# (macOS and Windows builds are skipped locally since cross-compiling in Docker requires additional setup).
act -j lint
act -j test
act -j build --matrix os:ubuntu-latest

echo "✅ All selected CI jobs passed!"
