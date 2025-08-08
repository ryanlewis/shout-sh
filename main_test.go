package main

import (
	"os"
	"regexp"
	"testing"
)

// TestProjectStructure verifies the basic project structure is set up correctly
func TestProjectStructure(t *testing.T) {
	t.Run("go.mod exists", func(t *testing.T) {
		if _, err := os.Stat("go.mod"); os.IsNotExist(err) {
			t.Error("go.mod file not found")
		}
	})

	t.Run("correct module name", func(t *testing.T) {
		content, err := os.ReadFile("go.mod")
		if err != nil {
			t.Fatalf("failed to read go.mod: %v", err)
		}

		expectedModule := "github.com/ryanlewis/shout-sh"
		if !containsString(string(content), expectedModule) {
			t.Errorf("go.mod does not contain expected module name: %s", expectedModule)
		}
	})

	t.Run("Go version specified", func(t *testing.T) {
		content, err := os.ReadFile("go.mod")
		if err != nil {
			t.Fatalf("failed to read go.mod: %v", err)
		}

		if !containsString(string(content), "go 1.") {
			t.Error("go.mod does not specify Go version")
		}
	})

	t.Run("main.go exists", func(t *testing.T) {
		if _, err := os.Stat("main.go"); os.IsNotExist(err) {
			t.Error("main.go file not found")
		}
	})
}

// TestToolVersions verifies the .tool-versions file is configured correctly
func TestToolVersions(t *testing.T) {
	t.Run(".tool-versions exists", func(t *testing.T) {
		if _, err := os.Stat(".tool-versions"); os.IsNotExist(err) {
			t.Error(".tool-versions file not found")
		}
	})

	t.Run("specifies golang version", func(t *testing.T) {
		content, err := os.ReadFile(".tool-versions")
		if err != nil {
			t.Fatalf("failed to read .tool-versions: %v", err)
		}

		// Match golang followed by a semantic version (e.g., golang 1.24.6)
		versionPattern := regexp.MustCompile(`golang\s+\d+\.\d+(\.\d+)?`)
		if !versionPattern.MatchString(string(content)) {
			t.Error(".tool-versions does not contain a valid golang version specification")
		}
	})
}

// TestDevelopmentTools verifies development tools can be installed
func TestDevelopmentTools(t *testing.T) {
	t.Run("go toolchain available", func(t *testing.T) {
		// This test will pass if the test itself is running
		// which means Go is properly installed
		t.Log("Go toolchain is available")
	})
}

// Helper function to check if a string contains a substring
func containsString(haystack, needle string) bool {
	return len(needle) > 0 && len(haystack) >= len(needle) &&
		stringIndex(haystack, needle) >= 0
}

// Helper function to find substring index
func stringIndex(s, substr string) int {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return i
		}
	}
	return -1
}
