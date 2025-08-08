package main

import (
	"os"
	"testing"
)

func TestDirectoryStructure(t *testing.T) {
	t.Run("required directories exist", func(t *testing.T) {
		requiredDirs := []string{
			"handlers",
			"render",
			"fonts",
			"middleware",
			"config",
			"types",
			"constants",
			"cmd",
		}

		for _, dir := range requiredDirs {
			t.Run(dir, func(t *testing.T) {
				info, err := os.Stat(dir)
				if err != nil {
					t.Errorf("directory %s does not exist: %v", dir, err)
					return
				}
				if !info.IsDir() {
					t.Errorf("%s exists but is not a directory", dir)
				}
			})
		}
	})

	t.Run("required files exist", func(t *testing.T) {
		requiredFiles := []struct {
			name string
			desc string
		}{
			{"README.md", "project documentation"},
			{".gitignore", "git ignore configuration"},
			{"LICENSE", "MIT license"},
			{"go.mod", "go module definition"},
		}

		for _, file := range requiredFiles {
			t.Run(file.name, func(t *testing.T) {
				info, err := os.Stat(file.name)
				if err != nil {
					t.Errorf("%s (%s) does not exist: %v", file.name, file.desc, err)
					return
				}
				if info.IsDir() {
					t.Errorf("%s exists but is a directory, not a file", file.name)
				}
				if info.Size() == 0 {
					t.Errorf("%s exists but is empty", file.name)
				}
			})
		}
	})

	t.Run("gitignore contains Go patterns", func(t *testing.T) {
		content, err := os.ReadFile(".gitignore")
		if err != nil {
			t.Fatalf("failed to read .gitignore: %v", err)
		}

		requiredPatterns := []string{
			"*.exe",
			"*.test",
			"*.out",
			".env",
			"shout",
		}

		contentStr := string(content)
		for _, pattern := range requiredPatterns {
			if !contains(contentStr, pattern) {
				t.Errorf(".gitignore missing pattern: %s", pattern)
			}
		}
	})

	t.Run("LICENSE is MIT", func(t *testing.T) {
		content, err := os.ReadFile("LICENSE")
		if err != nil {
			t.Fatalf("failed to read LICENSE: %v", err)
		}

		if !contains(string(content), "MIT License") {
			t.Error("LICENSE does not contain 'MIT License' text")
		}
		if !contains(string(content), "2025") {
			t.Error("LICENSE does not contain year 2025")
		}
	})
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsHelper(s, substr))
}

func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
