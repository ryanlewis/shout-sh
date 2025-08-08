package types

import (
	"encoding/json"
	"testing"
	"time"
)

func TestRenderOptions(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected RenderOptions
	}{
		{
			name: "parse all fields",
			input: `{
				"font": "doom",
				"color": "rainbow",
				"maxwidth": 80,
				"timeout": 10,
				"speed": 5,
				"align": "center",
				"border": "double"
			}`,
			expected: RenderOptions{
				Font:     "doom",
				Color:    "rainbow",
				MaxWidth: 80,
				Timeout:  10,
				Speed:    5,
				Align:    "center",
				Border:   "double",
			},
		},
		{
			name:     "empty json",
			input:    `{}`,
			expected: RenderOptions{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var opts RenderOptions
			err := json.Unmarshal([]byte(tt.input), &opts)
			if err != nil {
				t.Fatalf("Failed to unmarshal: %v", err)
			}

			if opts.Font != tt.expected.Font {
				t.Errorf("Font mismatch: got %s, want %s", opts.Font, tt.expected.Font)
			}
			if opts.Color != tt.expected.Color {
				t.Errorf("Color mismatch: got %s, want %s", opts.Color, tt.expected.Color)
			}
			if opts.MaxWidth != tt.expected.MaxWidth {
				t.Errorf("MaxWidth mismatch: got %d, want %d", opts.MaxWidth, tt.expected.MaxWidth)
			}
			if opts.Timeout != tt.expected.Timeout {
				t.Errorf("Timeout mismatch: got %d, want %d", opts.Timeout, tt.expected.Timeout)
			}
			if opts.Speed != tt.expected.Speed {
				t.Errorf("Speed mismatch: got %d, want %d", opts.Speed, tt.expected.Speed)
			}
			if opts.Align != tt.expected.Align {
				t.Errorf("Align mismatch: got %s, want %s", opts.Align, tt.expected.Align)
			}
			if opts.Border != tt.expected.Border {
				t.Errorf("Border mismatch: got %s, want %s", opts.Border, tt.expected.Border)
			}
		})
	}
}

func TestConnectionManager(t *testing.T) {
	cm := NewConnectionManager(2)

	// Test initial state
	if cm.GetActiveCount() != 0 {
		t.Errorf("Initial active count should be 0, got %d", cm.GetActiveCount())
	}

	// Test acquire
	if !cm.TryAcquire() {
		t.Error("First acquire should succeed")
	}
	if cm.GetActiveCount() != 1 {
		t.Errorf("Active count should be 1, got %d", cm.GetActiveCount())
	}

	if !cm.TryAcquire() {
		t.Error("Second acquire should succeed")
	}
	if cm.GetActiveCount() != 2 {
		t.Errorf("Active count should be 2, got %d", cm.GetActiveCount())
	}

	// Test max limit
	if cm.TryAcquire() {
		t.Error("Third acquire should fail (max is 2)")
	}

	// Test release
	cm.Release()
	if cm.GetActiveCount() != 1 {
		t.Errorf("Active count should be 1 after release, got %d", cm.GetActiveCount())
	}

	// Should be able to acquire again
	if !cm.TryAcquire() {
		t.Error("Should be able to acquire after release")
	}
}

func TestConfig(t *testing.T) {
	cfg := Config{
		Server: ServerConfig{
			PublicPort:        8080,
			AdminPort:         9090,
			ReadTimeout:       10 * time.Second,
			IdleTimeout:       120 * time.Second,
			MaxStreamDuration: 30 * time.Second,
		},
		RateLimit: RateLimitConfig{
			RequestsPerMinute: 100,
			BurstSize:         10,
		},
		Fonts: FontConfig{
			Directory:   "./fonts",
			DefaultFont: "standard",
			Fonts:       []string{"doom", "standard", "3d"},
		},
		Streaming: StreamingConfig{
			MaxConcurrentStreams: 100,
			DefaultTimeout:       5 * time.Second,
			DefaultSpeed:         5,
			MinSpeed:             1,
			MaxSpeed:             10,
		},
		Text: TextConfig{
			MaxLength:     100,
			DefaultAlign:  "left",
			DefaultBorder: "none",
		},
		Version: "1.0.0",
	}

	// Test server config
	if cfg.Server.PublicPort != 8080 {
		t.Errorf("PublicPort should be 8080, got %d", cfg.Server.PublicPort)
	}
	if cfg.Server.AdminPort != 9090 {
		t.Errorf("AdminPort should be 9090, got %d", cfg.Server.AdminPort)
	}

	// Test rate limit config
	if cfg.RateLimit.RequestsPerMinute != 100 {
		t.Errorf("RequestsPerMinute should be 100, got %d", cfg.RateLimit.RequestsPerMinute)
	}

	// Test fonts config
	if cfg.Fonts.DefaultFont != "standard" {
		t.Errorf("DefaultFont should be 'standard', got %s", cfg.Fonts.DefaultFont)
	}
	if len(cfg.Fonts.Fonts) != 3 {
		t.Errorf("Should have 3 fonts, got %d", len(cfg.Fonts.Fonts))
	}

	// Test streaming config
	if cfg.Streaming.MaxConcurrentStreams != 100 {
		t.Errorf("MaxConcurrentStreams should be 100, got %d", cfg.Streaming.MaxConcurrentStreams)
	}
	if cfg.Streaming.DefaultSpeed != 5 {
		t.Errorf("DefaultSpeed should be 5, got %d", cfg.Streaming.DefaultSpeed)
	}

	// Test text config
	if cfg.Text.MaxLength != 100 {
		t.Errorf("MaxLength should be 100, got %d", cfg.Text.MaxLength)
	}
	if cfg.Text.DefaultAlign != "left" {
		t.Errorf("DefaultAlign should be 'left', got %s", cfg.Text.DefaultAlign)
	}

	// Test version
	if cfg.Version != "1.0.0" {
		t.Errorf("Version should be '1.0.0', got %s", cfg.Version)
	}
}

func TestMetrics(t *testing.T) {
	m := &Metrics{
		StaticRequests:  100,
		PartyRequests:   50,
		FontRequests:    10,
		RejectedStreams: 5,
		TotalErrors:     2,
	}

	if m.StaticRequests != 100 {
		t.Errorf("StaticRequests should be 100, got %d", m.StaticRequests)
	}
	if m.PartyRequests != 50 {
		t.Errorf("PartyRequests should be 50, got %d", m.PartyRequests)
	}
	if m.FontRequests != 10 {
		t.Errorf("FontRequests should be 10, got %d", m.FontRequests)
	}
	if m.RejectedStreams != 5 {
		t.Errorf("RejectedStreams should be 5, got %d", m.RejectedStreams)
	}
	if m.TotalErrors != 2 {
		t.Errorf("TotalErrors should be 2, got %d", m.TotalErrors)
	}
}
