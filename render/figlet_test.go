package render

import (
	"strings"
	"testing"

	"github.com/ryanlewis/shout-sh/config"
	"github.com/ryanlewis/shout-sh/types"
)

func TestGenerateASCII(t *testing.T) {
	// Setup: Load fonts for testing
	cache := NewFontCache()
	cfg := config.FontConfig{
		Path:    "../fonts",
		Allowed: []string{"standard", "doom", "big", "small"},
	}
	err := cache.LoadFonts(cfg)
	if err != nil {
		t.Fatalf("Failed to load fonts: %v", err)
	}

	tests := []struct {
		name        string
		text        string
		opts        types.RenderOptions
		wantErr     bool
		checkOutput func(t *testing.T, output string)
	}{
		{
			name: "basic text with default font",
			text: "HELLO",
			opts: types.RenderOptions{
				Font: "standard",
			},
			wantErr: false,
			checkOutput: func(t *testing.T, output string) {
				if output == "" {
					t.Error("Expected non-empty output")
				}
				// ASCII art for H should be multi-line and contain specific patterns
				lines := strings.Split(output, "\n")
				if len(lines) < 3 {
					t.Errorf("Expected multi-line output, got %d lines", len(lines))
				}
			},
		},
		{
			name: "text with doom font",
			text: "DOOM",
			opts: types.RenderOptions{
				Font: "doom",
			},
			wantErr: false,
			checkOutput: func(t *testing.T, output string) {
				if output == "" {
					t.Error("Expected non-empty output")
				}
				// Doom font creates larger ASCII art
				if len(output) < 50 {
					t.Error("Doom font should create larger output")
				}
			},
		},
		{
			name: "empty text returns empty string",
			text: "",
			opts: types.RenderOptions{
				Font: "standard",
			},
			wantErr: false,
			checkOutput: func(t *testing.T, output string) {
				if output != "" {
					t.Error("Expected empty output for empty input")
				}
			},
		},
		{
			name: "non-existent font falls back to default",
			text: "TEST",
			opts: types.RenderOptions{
				Font: "nonexistent",
			},
			wantErr: false,
			checkOutput: func(t *testing.T, output string) {
				if output == "" {
					t.Error("Expected non-empty output with fallback font")
				}
			},
		},
		{
			name: "special characters",
			text: "Hello, World!",
			opts: types.RenderOptions{
				Font: "standard",
			},
			wantErr: false,
			checkOutput: func(t *testing.T, output string) {
				if output == "" {
					t.Error("Expected non-empty output")
				}
			},
		},
		{
			name: "numbers",
			text: "12345",
			opts: types.RenderOptions{
				Font: "big",
			},
			wantErr: false,
			checkOutput: func(t *testing.T, output string) {
				if output == "" {
					t.Error("Expected non-empty output")
				}
			},
		},
		{
			name: "long text",
			text: "This is a very long text that should still render properly",
			opts: types.RenderOptions{
				Font: "small",
			},
			wantErr: false,
			checkOutput: func(t *testing.T, output string) {
				if output == "" {
					t.Error("Expected non-empty output")
				}
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			output, err := GenerateASCII(tt.text, tt.opts, cache)

			if (err != nil) != tt.wantErr {
				t.Errorf("GenerateASCII() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if tt.checkOutput != nil {
				tt.checkOutput(t, output)
			}
		})
	}
}

func TestGenerateASCII_WithNilCache(t *testing.T) {
	opts := types.RenderOptions{
		Font: "standard",
	}

	_, err := GenerateASCII("TEST", opts, nil)
	if err == nil {
		t.Error("Expected error when cache is nil")
	}
	if !strings.Contains(err.Error(), "font cache is nil") {
		t.Errorf("Expected 'font cache is nil' error, got: %v", err)
	}
}

func TestGenerateASCII_EmptyCache(t *testing.T) {
	cache := NewFontCache()
	// Don't load any fonts

	opts := types.RenderOptions{
		Font: "standard",
	}

	_, err := GenerateASCII("TEST", opts, cache)
	if err == nil {
		t.Error("Expected error when no fonts are loaded")
	}
	if !strings.Contains(err.Error(), "no fonts loaded") {
		t.Errorf("Expected 'no fonts loaded' error, got: %v", err)
	}
}

func TestGenerateASCII_ConcurrentAccess(t *testing.T) {
	// Setup: Load fonts for testing
	cache := NewFontCache()
	cfg := config.FontConfig{
		Path:    "../fonts",
		Allowed: []string{"standard", "doom"},
	}
	err := cache.LoadFonts(cfg)
	if err != nil {
		t.Fatalf("Failed to load fonts: %v", err)
	}

	// Run multiple goroutines generating ASCII art concurrently
	done := make(chan bool)
	for i := 0; i < 10; i++ {
		go func(id int) {
			opts := types.RenderOptions{
				Font: "standard",
			}
			if id%2 == 0 {
				opts.Font = "doom"
			}

			text := "TEST"
			output, err := GenerateASCII(text, opts, cache)
			if err != nil {
				t.Errorf("Goroutine %d: unexpected error: %v", id, err)
			}
			if output == "" {
				t.Errorf("Goroutine %d: expected non-empty output", id)
			}
			done <- true
		}(i)
	}

	// Wait for all goroutines to complete
	for i := 0; i < 10; i++ {
		<-done
	}
}

func TestGenerateASCII_DefaultFontFallback(t *testing.T) {
	// Setup: Load only the default font
	cache := NewFontCache()
	cfg := config.FontConfig{
		Path:    "../fonts",
		Allowed: []string{"standard"},
	}
	err := cache.LoadFonts(cfg)
	if err != nil {
		t.Fatalf("Failed to load fonts: %v", err)
	}

	// Try to use a non-existent font, should fall back to default
	opts := types.RenderOptions{
		Font: "nonexistent",
	}

	output, err := GenerateASCII("FALLBACK", opts, cache)
	if err != nil {
		t.Errorf("Expected successful fallback, got error: %v", err)
	}
	if output == "" {
		t.Error("Expected non-empty output with fallback")
	}
}

func BenchmarkGenerateASCII(b *testing.B) {
	// Setup
	cache := NewFontCache()
	cfg := config.FontConfig{
		Path:    "../fonts",
		Allowed: []string{"standard"},
	}
	err := cache.LoadFonts(cfg)
	if err != nil {
		b.Fatalf("Failed to load fonts: %v", err)
	}

	opts := types.RenderOptions{
		Font: "standard",
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, _ = GenerateASCII("BENCHMARK", opts, cache)
	}
}
