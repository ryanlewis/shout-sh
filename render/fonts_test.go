package render

import (
	"os"
	"path/filepath"
	"sync"
	"testing"

	"github.com/ryanlewis/shout-sh/config"
)

func TestNewFontCache(t *testing.T) {
	cache := NewFontCache()

	if cache == nil {
		t.Fatal("NewFontCache returned nil")
	}

	if cache.fonts == nil {
		t.Error("fonts map not initialized")
	}

	if len(cache.fonts) != 0 {
		t.Errorf("expected empty fonts map, got %d fonts", len(cache.fonts))
	}
}

func TestFontCacheLoadFonts(t *testing.T) {
	// Setup test environment
	originalPath := os.Getenv("SHOUT_FONTS_PATH")
	defer func() {
		if originalPath != "" {
			os.Setenv("SHOUT_FONTS_PATH", originalPath)
		} else {
			os.Unsetenv("SHOUT_FONTS_PATH")
		}
		config.Reset()
	}()

	// Create temp directory with test fonts
	tempDir := t.TempDir()

	// Copy actual font files for testing
	sourceFonts := []string{"standard", "doom", "slant"}
	for _, fontName := range sourceFonts {
		srcPath := filepath.Join("../fonts", fontName+".flf")
		if _, err := os.Stat(srcPath); err == nil {
			data, err := os.ReadFile(srcPath)
			if err != nil {
				t.Fatalf("Failed to read font file %s: %v", srcPath, err)
			}
			destPath := filepath.Join(tempDir, fontName+".flf")
			if err := os.WriteFile(destPath, data, 0644); err != nil {
				t.Fatalf("Failed to write test font file: %v", err)
			}
		}
	}

	// Set environment for test
	os.Setenv("SHOUT_FONTS_PATH", tempDir)
	os.Setenv("SHOUT_FONTS_ALLOWED", "standard,doom,slant,missing")
	config.Reset()

	cfg := config.MustLoad()

	cache := NewFontCache()
	err := cache.LoadFonts(cfg.Fonts)

	if err != nil {
		t.Fatalf("LoadFonts failed: %v", err)
	}

	// Check that at least one font loaded
	if len(cache.fonts) == 0 {
		t.Error("No fonts loaded")
	}

	// Check that specific fonts loaded
	if _, exists := cache.GetFont("standard"); !exists {
		t.Error("Standard font not loaded")
	}

	// Missing font should not cause error
	if _, exists := cache.GetFont("missing"); exists {
		t.Error("Missing font should not be loaded")
	}
}

func TestFontCacheGetFont(t *testing.T) {
	cache := NewFontCache()

	// Test getting font from empty cache
	font, exists := cache.GetFont("test")
	if exists {
		t.Error("GetFont should return false for empty cache")
	}
	if font != nil {
		t.Error("GetFont should return nil for missing font")
	}

	// Manually add a test font
	cache.mu.Lock()
	cache.fonts["test"] = &Font{Name: "test"}
	cache.mu.Unlock()

	// Test getting existing font
	font, exists = cache.GetFont("test")
	if !exists {
		t.Error("GetFont should return true for existing font")
	}
	if font == nil {
		t.Error("GetFont should return font for existing font")
	} else if font.Name != "test" {
		t.Errorf("GetFont returned wrong font, got %s want test", font.Name)
	}
}

func TestFontCacheGetFontOrDefault(t *testing.T) {
	cache := NewFontCache()

	// Add default and custom fonts
	cache.mu.Lock()
	cache.fonts["standard"] = &Font{Name: "standard"}
	cache.fonts["doom"] = &Font{Name: "doom"}
	cache.mu.Unlock()

	// Test getting existing font
	font := cache.GetFontOrDefault("doom", "standard")
	if font == nil {
		t.Fatal("GetFontOrDefault returned nil for existing font")
	}
	if font.Name != "doom" {
		t.Errorf("GetFontOrDefault returned wrong font, got %s want doom", font.Name)
	}

	// Test fallback to default
	font = cache.GetFontOrDefault("missing", "standard")
	if font == nil {
		t.Fatal("GetFontOrDefault returned nil when default exists")
	}
	if font.Name != "standard" {
		t.Errorf("GetFontOrDefault didn't fall back to default, got %s want standard", font.Name)
	}

	// Test both missing
	font = cache.GetFontOrDefault("missing", "also-missing")
	if font != nil {
		t.Error("GetFontOrDefault should return nil when both missing")
	}
}

func TestFontCacheListFonts(t *testing.T) {
	cache := NewFontCache()

	// Test empty cache
	fonts := cache.ListFonts()
	if len(fonts) != 0 {
		t.Errorf("ListFonts should return empty slice for empty cache, got %d fonts", len(fonts))
	}

	// Add fonts
	cache.mu.Lock()
	cache.fonts["doom"] = &Font{Name: "doom"}
	cache.fonts["standard"] = &Font{Name: "standard"}
	cache.fonts["3d"] = &Font{Name: "3d"}
	cache.mu.Unlock()

	// Test listing fonts
	fonts = cache.ListFonts()
	if len(fonts) != 3 {
		t.Errorf("ListFonts returned %d fonts, want 3", len(fonts))
	}

	// Check fonts are sorted
	expected := []string{"3d", "doom", "standard"}
	for i, name := range expected {
		if fonts[i] != name {
			t.Errorf("Font at index %d: got %s, want %s", i, fonts[i], name)
		}
	}
}

func TestFontCacheConcurrency(t *testing.T) {
	cache := NewFontCache()

	// Add initial fonts
	cache.mu.Lock()
	for i := 0; i < 10; i++ {
		name := string(rune('a' + i))
		cache.fonts[name] = &Font{Name: name}
	}
	cache.mu.Unlock()

	// Test concurrent reads and writes
	var wg sync.WaitGroup

	// Concurrent readers
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()

			// Read operations
			cache.GetFont(string(rune('a' + (id % 10))))
			cache.GetFontOrDefault("missing", "a")
			cache.ListFonts()
		}(i)
	}

	// Concurrent writers
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()

			name := string(rune('z' - id))
			cache.mu.Lock()
			cache.fonts[name] = &Font{Name: name}
			cache.mu.Unlock()
		}(i)
	}

	wg.Wait()

	// Verify cache is still consistent
	fonts := cache.ListFonts()
	if len(fonts) < 10 {
		t.Errorf("Cache corrupted after concurrent access, got %d fonts", len(fonts))
	}
}

func TestValidateFont(t *testing.T) {
	tests := []struct {
		name    string
		path    string
		wantErr bool
	}{
		{
			name:    "valid font file",
			path:    "../fonts/standard.flf",
			wantErr: false,
		},
		{
			name:    "missing file",
			path:    "../fonts/nonexistent.flf",
			wantErr: true,
		},
		{
			name:    "directory instead of file",
			path:    "../fonts",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateFont(tt.path)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateFont() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestFontRender(t *testing.T) {
	// Setup test environment
	originalPath := os.Getenv("SHOUT_FONTS_PATH")
	defer func() {
		if originalPath != "" {
			os.Setenv("SHOUT_FONTS_PATH", originalPath)
		} else {
			os.Unsetenv("SHOUT_FONTS_PATH")
		}
		config.Reset()
	}()

	os.Setenv("SHOUT_FONTS_PATH", "../fonts")
	os.Setenv("SHOUT_FONTS_ALLOWED", "standard,doom")
	config.Reset()

	cfg := config.MustLoad()

	cache := NewFontCache()
	err := cache.LoadFonts(cfg.Fonts)
	if err != nil {
		t.Fatalf("Failed to load fonts: %v", err)
	}

	font, exists := cache.GetFont("standard")
	if !exists {
		t.Fatal("Standard font not loaded")
	}

	// Test rendering
	result, err := font.Render("TEST")
	if err != nil {
		t.Fatalf("Failed to render text: %v", err)
	}

	if result == "" {
		t.Error("Render returned empty string")
	}

	// The result should contain the text in some form
	// Note: exact output depends on the font
	if len(result) < len("TEST") {
		t.Error("Rendered text seems too short")
	}
}

func TestLoadFontsWithInvalidPath(t *testing.T) {
	cfg := config.FontConfig{
		Path:    "/nonexistent/path",
		Allowed: []string{"standard"},
	}

	cache := NewFontCache()
	err := cache.LoadFonts(cfg)

	// Should not error even with invalid path, just warn
	if err != nil {
		t.Fatalf("LoadFonts should not error on invalid path: %v", err)
	}

	// But no fonts should be loaded
	if len(cache.fonts) != 0 {
		t.Errorf("Expected no fonts loaded, got %d", len(cache.fonts))
	}
}

func TestLoadFontsEmptyAllowedList(t *testing.T) {
	cfg := config.FontConfig{
		Path:    "../fonts",
		Allowed: []string{},
	}

	cache := NewFontCache()
	err := cache.LoadFonts(cfg)

	if err != nil {
		t.Fatalf("LoadFonts failed with empty allowed list: %v", err)
	}

	// No fonts should be loaded
	if len(cache.fonts) != 0 {
		t.Errorf("Expected no fonts with empty allowed list, got %d", len(cache.fonts))
	}
}

