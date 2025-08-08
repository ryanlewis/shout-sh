#!/bin/bash

# Download required FIGlet fonts from official sources
# Based on the figlet repository at https://github.com/cmatsuoka/figlet

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Base URL for the figlet fonts repository
BASE_URL="https://raw.githubusercontent.com/cmatsuoka/figlet/master/fonts"

# Alternative URL for contributed fonts
CONTRIB_URL="https://raw.githubusercontent.com/xero/figlet-fonts/master"

echo "Starting FIGlet font download..."
echo "================================"
echo ""

# Download standard fonts from figlet repo
STANDARD_FONTS=(
    "big.flf"
    "standard.flf"
    "slant.flf"
    "small.flf"
    "shadow.flf"
)

for font in "${STANDARD_FONTS[@]}"; do
    echo -n "Downloading $font... "
    
    if curl -sL "${BASE_URL}/${font}" -o "${font}" 2>/dev/null; then
        # Validate the font header
        if head -n 1 "${font}" | grep -q "^flf2a"; then
            echo "✓ Success"
        else
            echo "✗ Invalid FIGlet header"
            rm -f "${font}"
        fi
    else
        echo "✗ Failed to download"
    fi
done

# Download contributed fonts from xero/figlet-fonts
# doom.flf
echo -n "Downloading doom.flf... "
if curl -sL "${CONTRIB_URL}/Doom.flf" -o "doom.flf" 2>/dev/null; then
    if head -n 1 "doom.flf" | grep -q "^flf2a"; then
        echo "✓ Success"
    else
        echo "✗ Invalid FIGlet header"
        rm -f "doom.flf"
    fi
else
    echo "✗ Failed to download"
fi

# 3d.flf
echo -n "Downloading 3d.flf... "
if curl -sL "${CONTRIB_URL}/3d.flf" -o "3d.flf" 2>/dev/null; then
    if head -n 1 "3d.flf" | grep -q "^flf2a"; then
        echo "✓ Success"
    else
        echo "✗ Invalid FIGlet header"
        rm -f "3d.flf"
    fi
else
    echo "✗ Failed to download"
fi

# bloody.flf
echo -n "Downloading bloody.flf... "
if curl -sL "${CONTRIB_URL}/Bloody.flf" -o "bloody.flf" 2>/dev/null; then
    if head -n 1 "bloody.flf" | grep -q "^flf2a"; then
        echo "✓ Success"
    else
        echo "✗ Invalid FIGlet header"
        rm -f "bloody.flf"
    fi
else
    echo "✗ Failed to download"
fi

echo ""
echo "Font download complete!"
echo ""

# Display summary
echo "Downloaded fonts:"
echo "-----------------"
for file in *.flf; do
    if [ -f "$file" ]; then
        size=$(du -h "$file" | cut -f1)
        echo "  • $file ($size)"
    fi
done

echo ""
echo "Running validation tests..."
cd ..
go test ./fonts -v