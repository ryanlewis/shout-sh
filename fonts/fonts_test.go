package fonts

import (
	"bufio"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// TestRequiredFontsExist verifies all required font files are present
func TestRequiredFontsExist(t *testing.T) {
	requiredFonts := []string{
		"doom.flf",
		"3d.flf",
		"big.flf",
		"bloody.flf",
		"standard.flf",
		"slant.flf",
		"small.flf",
		"shadow.flf",
	}

	for _, fontName := range requiredFonts {
		fontPath := filepath.Join(".", fontName)
		t.Run(fontName, func(t *testing.T) {
			if _, err := os.Stat(fontPath); os.IsNotExist(err) {
				t.Errorf("Required font file %s does not exist", fontName)
			}
		})
	}
}

// TestFontFileHeaders validates that font files have proper FIGlet headers
func TestFontFileHeaders(t *testing.T) {
	requiredFonts := []string{
		"doom.flf",
		"3d.flf",
		"big.flf",
		"bloody.flf",
		"standard.flf",
		"slant.flf",
		"small.flf",
		"shadow.flf",
	}

	for _, fontName := range requiredFonts {
		fontPath := filepath.Join(".", fontName)
		t.Run(fontName, func(t *testing.T) {
			file, err := os.Open(fontPath)
			if err != nil {
				if os.IsNotExist(err) {
					t.Skipf("Font file %s does not exist yet", fontName)
					return
				}
				t.Fatalf("Failed to open font file %s: %v", fontName, err)
			}
			defer file.Close()

			scanner := bufio.NewScanner(file)
			if !scanner.Scan() {
				t.Errorf("Font file %s is empty", fontName)
				return
			}

			firstLine := scanner.Text()
			// FIGlet font files should start with "flf2a" signature
			if !strings.HasPrefix(firstLine, "flf2a") {
				t.Errorf("Font file %s does not have valid FIGlet header (expected 'flf2a', got '%s')", fontName, firstLine[:min(10, len(firstLine))])
			}
		})
	}
}

// TestFontFileContent checks that font files are not empty and have reasonable size
func TestFontFileContent(t *testing.T) {
	requiredFonts := []string{
		"doom.flf",
		"3d.flf",
		"big.flf",
		"bloody.flf",
		"standard.flf",
		"slant.flf",
		"small.flf",
		"shadow.flf",
	}

	for _, fontName := range requiredFonts {
		fontPath := filepath.Join(".", fontName)
		t.Run(fontName, func(t *testing.T) {
			info, err := os.Stat(fontPath)
			if err != nil {
				if os.IsNotExist(err) {
					t.Skipf("Font file %s does not exist yet", fontName)
					return
				}
				t.Fatalf("Failed to stat font file %s: %v", fontName, err)
			}

			// FIGlet font files are typically at least 1KB
			if info.Size() < 1024 {
				t.Errorf("Font file %s seems too small (%d bytes)", fontName, info.Size())
			}

			// But shouldn't be unreasonably large (>1MB is suspicious)
			if info.Size() > 1024*1024 {
				t.Errorf("Font file %s seems too large (%d bytes)", fontName, info.Size())
			}
		})
	}
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
