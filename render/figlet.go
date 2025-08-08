package render

import (
	"fmt"

	"github.com/ryanlewis/shout-sh/types"
)

const DefaultFont = "standard"

// GenerateASCII generates ASCII art from text using the specified font.
// If the requested font is not available, it falls back to the default font.
// If no fonts are loaded at all, it returns an error.
//
// Parameters:
//   - text: the text to render as ASCII art
//   - opts: rendering options including font selection
//   - cache: the font cache containing loaded fonts
//
// Returns:
//   - string: the generated ASCII art
//   - error: error if generation fails or no fonts are available
//
// Example:
//
//	ascii, err := GenerateASCII("HELLO", opts, fontCache)
//	if err != nil {
//	    log.Printf("Failed to generate ASCII: %v", err)
//	    return
//	}
//	fmt.Println(ascii)
func GenerateASCII(text string, opts types.RenderOptions, cache *FontCache) (string, error) {
	// Validate cache
	if cache == nil {
		return "", fmt.Errorf("font cache is nil")
	}

	// Handle empty text
	if text == "" {
		return "", nil
	}

	// Try to get the requested font, falling back to default
	font := cache.GetFontOrDefault(opts.Font, DefaultFont)
	if font == nil {
		return "", fmt.Errorf("no fonts loaded")
	}

	// Render the text using the selected font
	ascii, err := font.Render(text)
	if err != nil {
		return "", fmt.Errorf("failed to render text: %w", err)
	}

	return ascii, nil
}
