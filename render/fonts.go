package render

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"sort"
	"sync"

	"github.com/ryanlewis/go-figure"
	"github.com/ryanlewis/shout-sh/config"
)

// Font represents a loaded FIGlet font ready for rendering.
//
// Usage example:
//
//	font, exists := cache.GetFont("doom")
//	if exists {
//	    output, err := font.Render("HELLO")
//	}
type Font struct {
	Name     string
	fontPath string
}

// Render generates ASCII art text using this font.
//
// Parameters:
//   - text: the text to render
//
// Returns:
//   - string: the rendered ASCII art
//   - error: error if rendering fails
//
// Example:
//
//	output, err := font.Render("HELLO")
//	if err != nil {
//	    // handle error
//	}
//	fmt.Println(output)
func (f *Font) Render(text string) (string, error) {
	if f == nil {
		return "", fmt.Errorf("font is nil")
	}

	// Open font file
	file, err := os.Open(f.fontPath)
	if err != nil {
		return "", fmt.Errorf("failed to open font file: %w", err)
	}
	defer file.Close()

	// Create figure with custom font
	fig := figure.NewFigureWithFont(text, file, true)
	return fig.String(), nil
}

// FontCache manages loaded fonts with thread-safe access.
// Fonts are loaded once and cached for the lifetime of the application.
//
// The type is safe for concurrent use.
//
// Usage example:
//
//	cache := NewFontCache()
//	err := cache.LoadFonts(config.Fonts)
//	if err != nil {
//	    // handle error
//	}
//	font := cache.GetFontOrDefault("doom", "standard")
type FontCache struct {
	mu    sync.RWMutex
	fonts map[string]*Font
}

// NewFontCache creates a new empty font cache.
//
// Returns:
//   - *FontCache: a new font cache instance
//
// Example:
//
//	cache := NewFontCache()
func NewFontCache() *FontCache {
	return &FontCache{
		fonts: make(map[string]*Font),
	}
}

// LoadFonts loads all configured fonts from disk into the cache.
// Fonts that fail to load are logged but don't cause the function to fail.
// This ensures the service can start even if some fonts are missing.
//
// Parameters:
//   - cfg: font configuration with paths and allowed fonts
//
// Returns:
//   - error: error if no fonts could be loaded
//
// Example:
//
//	err := cache.LoadFonts(config.Get().Fonts)
//	if err != nil {
//	    log.Fatal("Failed to load fonts:", err)
//	}
func (fc *FontCache) LoadFonts(cfg config.FontConfig) error {
	fc.mu.Lock()
	defer fc.mu.Unlock()

	loadedCount := 0

	for _, fontName := range cfg.Allowed {
		fontPath := filepath.Join(cfg.Path, fontName+".flf")

		// Validate font file exists and is readable
		if err := ValidateFont(fontPath); err != nil {
			log.Printf("Warning: Could not load font %s: %v", fontName, err)
			continue
		}

		// Store font with path for on-demand loading
		fc.fonts[fontName] = &Font{
			Name:     fontName,
			fontPath: fontPath,
		}

		loadedCount++
		log.Printf("Loaded font: %s", fontName)
	}

	log.Printf("Loaded %d fonts successfully", loadedCount)
	return nil
}

// GetFont retrieves a font from the cache by name.
//
// Parameters:
//   - name: the name of the font to retrieve
//
// Returns:
//   - *Font: the font if found, nil otherwise
//   - bool: true if the font exists, false otherwise
//
// Example:
//
//	font, exists := cache.GetFont("doom")
//	if !exists {
//	    // font not found
//	}
func (fc *FontCache) GetFont(name string) (*Font, bool) {
	fc.mu.RLock()
	defer fc.mu.RUnlock()

	font, exists := fc.fonts[name]
	return font, exists
}

// GetFontOrDefault retrieves a font from the cache with fallback to a default.
// If the requested font doesn't exist, it returns the default font.
// If neither exists, it returns nil.
//
// Parameters:
//   - name: the name of the font to retrieve
//   - defaultName: the name of the default font to use as fallback
//
// Returns:
//   - *Font: the font if found, default if name not found, nil if both missing
//
// Example:
//
//	font := cache.GetFontOrDefault("custom", "standard")
//	if font == nil {
//	    // no fonts available
//	}
func (fc *FontCache) GetFontOrDefault(name, defaultName string) *Font {
	fc.mu.RLock()
	defer fc.mu.RUnlock()

	if font, exists := fc.fonts[name]; exists {
		return font
	}

	if font, exists := fc.fonts[defaultName]; exists {
		return font
	}

	return nil
}

// ListFonts returns a sorted list of all loaded font names.
//
// Returns:
//   - []string: sorted list of font names
//
// Example:
//
//	fonts := cache.ListFonts()
//	for _, name := range fonts {
//	    fmt.Println(name)
//	}
func (fc *FontCache) ListFonts() []string {
	fc.mu.RLock()
	defer fc.mu.RUnlock()

	names := make([]string, 0, len(fc.fonts))
	for name := range fc.fonts {
		names = append(names, name)
	}

	sort.Strings(names)
	return names
}

// ValidateFont checks if a font file exists and is readable.
// This function verifies that the file exists, is a regular file (not a directory),
// and can be opened for reading.
//
// Parameters:
//   - path: the path to the font file
//
// Returns:
//   - error: nil if valid, error describing the problem otherwise
//
// Example:
//
//	err := ValidateFont("/path/to/font.flf")
//	if err != nil {
//	    log.Printf("Invalid font: %v", err)
//	}
func ValidateFont(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("font file does not exist: %s", path)
		}
		return fmt.Errorf("cannot access font file: %w", err)
	}

	if info.IsDir() {
		return fmt.Errorf("font path is a directory, not a file: %s", path)
	}

	// Try to open the file to ensure it's readable
	file, err := os.Open(path)
	if err != nil {
		return fmt.Errorf("cannot read font file: %w", err)
	}
	file.Close()

	return nil
}

