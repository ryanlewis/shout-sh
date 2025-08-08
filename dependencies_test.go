package main

import (
	"testing"

	// Core dependencies - verify they can be imported
	_ "github.com/caarlos0/env/v11"
	_ "github.com/gofiber/fiber/v2"
	_ "github.com/joho/godotenv"
	_ "github.com/ryanlewis/go-figure"
)

// TestDependencies verifies all required dependencies are available
func TestDependencies(t *testing.T) {
	tests := []struct {
		name    string
		pkgPath string
	}{
		{"Fiber v2 web framework", "github.com/gofiber/fiber/v2"},
		{"go-figure ASCII art library", "github.com/ryanlewis/go-figure"},
		{"godotenv .env file loader", "github.com/joho/godotenv"},
		{"caarlos0/env environment parser", "github.com/caarlos0/env/v11"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// If the import succeeds (which it does by being in the imports section),
			// the test passes. This ensures go mod download works correctly.
			t.Logf("✓ %s (%s) is available", tt.name, tt.pkgPath)
		})
	}
}

// TestDependencyVersions ensures we're using the expected major versions
func TestDependencyVersions(t *testing.T) {
	// This test will fail to compile if we're not using the right major versions
	// For example, if we accidentally used fiber v1 instead of v2
	t.Log("All dependencies are using correct major versions")
}