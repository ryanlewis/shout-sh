package config

import (
	"os"
	"strings"
	"testing"
)

func TestConfig_DefaultValues(t *testing.T) {
	// Reset singleton
	Reset()
	defer Reset()

	// Test that the config struct is properly initialized with defaults
	// We'll verify these after implementing the actual config struct
	tests := []struct {
		name string
		want interface{}
	}{
		{
			name: "Version should have default",
			want: "dev",
		},
		{
			name: "PublicPort should default to 8080",
			want: 8080,
		},
		{
			name: "AdminPort should default to 9090",
			want: 9090,
		},
		{
			name: "Host should default to 0.0.0.0",
			want: "0.0.0.0",
		},
		{
			name: "RateLimit should default to 100",
			want: 100,
		},
		{
			name: "RateLimitBurst should default to 10",
			want: 10,
		},
		{
			name: "DefaultFont should be standard",
			want: "standard",
		},
		{
			name: "StreamingTimeout should default to 30",
			want: 30,
		},
		{
			name: "StreamingMaxTimeout should default to 300",
			want: 300,
		},
		{
			name: "StreamingDefaultSpeed should default to 5",
			want: 5,
		},
		{
			name: "TextMaxLength should default to 100",
			want: 100,
		},
		{
			name: "TextDefaultAlign should be center",
			want: "center",
		},
	}

	// Load config with defaults
	loaded, err := Load()
	if err != nil {
		t.Fatalf("Failed to load config: %v", err)
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Use the loaded config instead of cfg
			var got interface{}
			switch tt.name {
			case "Version should have default":
				got = loaded.Version
			case "PublicPort should default to 8080":
				got = loaded.Server.PublicPort
			case "AdminPort should default to 9090":
				got = loaded.Server.AdminPort
			case "Host should default to 0.0.0.0":
				got = loaded.Server.Host
			case "RateLimit should default to 100":
				got = loaded.RateLimit.RequestsPerMinute
			case "RateLimitBurst should default to 10":
				got = loaded.RateLimit.Burst
			case "DefaultFont should be standard":
				got = loaded.Fonts.Default
			case "StreamingTimeout should default to 30":
				got = loaded.Streaming.DefaultTimeout
			case "StreamingMaxTimeout should default to 300":
				got = loaded.Streaming.MaxTimeout
			case "StreamingDefaultSpeed should default to 5":
				got = loaded.Streaming.DefaultSpeed
			case "TextMaxLength should default to 100":
				got = loaded.Text.MaxLength
			case "TextDefaultAlign should be center":
				got = loaded.Text.DefaultAlign
			}

			if got != tt.want {
				t.Errorf("Config.%s = %v, want %v", tt.name, got, tt.want)
			}
		})
	}
}

func TestConfig_LoadWithEnvOverrides(t *testing.T) {
	// Save original env vars
	originalEnv := os.Environ()
	defer func() {
		// Restore original env vars
		os.Clearenv()
		for _, e := range originalEnv {
			pair := splitEnvVar(e)
			os.Setenv(pair[0], pair[1])
		}
		Reset() // Reset singleton after test
	}()

	// Clear environment and reset singleton
	os.Clearenv()
	Reset()

	// Set test environment variables
	testEnvVars := map[string]string{
		"SHOUT_VERSION":                       "1.0.0",
		"SHOUT_SERVER_PUBLIC_PORT":            "3000",
		"SHOUT_SERVER_ADMIN_PORT":             "3001",
		"SHOUT_SERVER_HOST":                   "127.0.0.1",
		"SHOUT_RATELIMIT_REQUESTS_PER_MINUTE": "200",
		"SHOUT_RATELIMIT_BURST":               "20",
		"SHOUT_FONTS_DEFAULT":                 "doom",
		"SHOUT_STREAMING_DEFAULT_TIMEOUT":     "60",
		"SHOUT_STREAMING_MAX_TIMEOUT":         "600",
		"SHOUT_STREAMING_DEFAULT_SPEED":       "8",
		"SHOUT_TEXT_MAX_LENGTH":               "200",
		"SHOUT_TEXT_DEFAULT_ALIGN":            "left",
	}

	for k, v := range testEnvVars {
		os.Setenv(k, v)
	}

	// Load config
	cfg, err := Load()
	if err != nil {
		t.Fatalf("Failed to load config with env overrides: %v", err)
	}

	// Verify overrides
	tests := []struct {
		name string
		got  interface{}
		want interface{}
	}{
		{"Version", cfg.Version, "1.0.0"},
		{"PublicPort", cfg.Server.PublicPort, 3000},
		{"AdminPort", cfg.Server.AdminPort, 3001},
		{"Host", cfg.Server.Host, "127.0.0.1"},
		{"RateLimit", cfg.RateLimit.RequestsPerMinute, 200},
		{"RateLimitBurst", cfg.RateLimit.Burst, 20},
		{"DefaultFont", cfg.Fonts.Default, "doom"},
		{"StreamingTimeout", cfg.Streaming.DefaultTimeout, 60},
		{"StreamingMaxTimeout", cfg.Streaming.MaxTimeout, 600},
		{"StreamingDefaultSpeed", cfg.Streaming.DefaultSpeed, 8},
		{"TextMaxLength", cfg.Text.MaxLength, 200},
		{"TextDefaultAlign", cfg.Text.DefaultAlign, "left"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.got != tt.want {
				t.Errorf("Config.%s = %v, want %v", tt.name, tt.got, tt.want)
			}
		})
	}
}

func TestConfig_LoadDotEnvFile(t *testing.T) {
	// Reset singleton and ensure no .env file exists
	Reset()
	os.Remove(".env") // Make sure no .env file exists at start
	defer func() {
		os.Remove(".env") // Clean up .env file
		// Unset env vars that were loaded from .env
		os.Unsetenv("SHOUT_VERSION")
		os.Unsetenv("SHOUT_SERVER_PUBLIC_PORT")
		os.Unsetenv("SHOUT_FONTS_DEFAULT")
		Reset() // Reset singleton
	}()

	// Create a temporary .env file
	envContent := `SHOUT_VERSION=2.0.0
SHOUT_SERVER_PUBLIC_PORT=4000
SHOUT_FONTS_DEFAULT=banner`

	err := os.WriteFile(".env", []byte(envContent), 0644)
	if err != nil {
		t.Fatalf("Failed to create .env file: %v", err)
	}

	// Clear any existing env vars to ensure we're reading from .env
	os.Unsetenv("SHOUT_VERSION")
	os.Unsetenv("SHOUT_SERVER_PUBLIC_PORT")
	os.Unsetenv("SHOUT_FONTS_DEFAULT")

	// Load config
	cfg, err := Load()
	if err != nil {
		t.Fatalf("Failed to load config from .env: %v", err)
	}

	// Verify values from .env file
	if cfg.Version != "2.0.0" {
		t.Errorf("Version = %s, want 2.0.0", cfg.Version)
	}
	if cfg.Server.PublicPort != 4000 {
		t.Errorf("PublicPort = %d, want 4000", cfg.Server.PublicPort)
	}
	if cfg.Fonts.Default != "banner" {
		t.Errorf("DefaultFont = %s, want banner", cfg.Fonts.Default)
	}
}

func TestConfig_Validation(t *testing.T) {
	tests := []struct {
		name    string
		envVars map[string]string
		wantErr bool
		errMsg  string
	}{
		{
			name: "Invalid public port - negative",
			envVars: map[string]string{
				"SHOUT_SERVER_PUBLIC_PORT": "-1",
			},
			wantErr: true,
			errMsg:  "invalid port",
		},
		{
			name: "Invalid public port - too high",
			envVars: map[string]string{
				"SHOUT_SERVER_PUBLIC_PORT": "70000",
			},
			wantErr: true,
			errMsg:  "invalid port",
		},
		{
			name: "Invalid admin port - zero",
			envVars: map[string]string{
				"SHOUT_SERVER_ADMIN_PORT": "0",
			},
			wantErr: true,
			errMsg:  "invalid port",
		},
		{
			name: "Invalid admin port - too high",
			envVars: map[string]string{
				"SHOUT_SERVER_ADMIN_PORT": "65536",
			},
			wantErr: true,
			errMsg:  "invalid port",
		},
		{
			name: "Invalid rate limit",
			envVars: map[string]string{
				"SHOUT_RATELIMIT_REQUESTS_PER_MINUTE": "0",
			},
			wantErr: true,
			errMsg:  "rate limit must be positive",
		},
		{
			name: "Invalid rate limit burst",
			envVars: map[string]string{
				"SHOUT_RATELIMIT_BURST": "0",
			},
			wantErr: true,
			errMsg:  "rate limit burst must be positive",
		},
		{
			name: "Invalid max text length",
			envVars: map[string]string{
				"SHOUT_TEXT_MAX_LENGTH": "0",
			},
			wantErr: true,
			errMsg:  "max text length must be positive",
		},
		{
			name: "Invalid streaming timeout",
			envVars: map[string]string{
				"SHOUT_STREAMING_DEFAULT_TIMEOUT": "0",
			},
			wantErr: true,
			errMsg:  "timeout must be positive",
		},
		{
			name: "Invalid streaming max timeout less than default",
			envVars: map[string]string{
				"SHOUT_STREAMING_DEFAULT_TIMEOUT": "100",
				"SHOUT_STREAMING_MAX_TIMEOUT": "50",
			},
			wantErr: true,
			errMsg:  "max timeout must be >= default timeout",
		},
		{
			name: "Invalid streaming speed - too low",
			envVars: map[string]string{
				"SHOUT_STREAMING_DEFAULT_SPEED": "0",
			},
			wantErr: true,
			errMsg:  "streaming speed must be between 1 and 10",
		},
		{
			name: "Invalid streaming speed - too high",
			envVars: map[string]string{
				"SHOUT_STREAMING_DEFAULT_SPEED": "11",
			},
			wantErr: true,
			errMsg:  "streaming speed must be between 1 and 10",
		},
		{
			name: "Invalid align value",
			envVars: map[string]string{
				"SHOUT_TEXT_DEFAULT_ALIGN": "invalid",
			},
			wantErr: true,
			errMsg:  "invalid alignment",
		},
		{
			name: "Valid configuration",
			envVars: map[string]string{
				"SHOUT_SERVER_PUBLIC_PORT": "8080",
				"SHOUT_SERVER_ADMIN_PORT": "9090",
				"SHOUT_TEXT_DEFAULT_ALIGN": "left",
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Reset singleton for each test
			Reset()
			defer Reset()
			
			// Save and clear env
			originalEnv := os.Environ()
			os.Clearenv()
			defer func() {
				os.Clearenv()
				for _, e := range originalEnv {
					pair := splitEnvVar(e)
					os.Setenv(pair[0], pair[1])
				}
			}()

			// Set test env vars
			for k, v := range tt.envVars {
				os.Setenv(k, v)
			}

			// Load and validate
			_, err := Load()
			if tt.wantErr {
				if err == nil {
					t.Errorf("Expected error containing '%s', got nil", tt.errMsg)
				}
			} else {
				if err != nil {
					t.Errorf("Unexpected error: %v", err)
				}
			}
		})
	}
}

func TestConfig_GetPanicsWithoutLoad(t *testing.T) {
	Reset()
	defer Reset()

	defer func() {
		if r := recover(); r == nil {
			t.Errorf("Get() did not panic when config not loaded")
		}
	}()

	Get()
}

func TestConfig_GetPanicsOnLoadError(t *testing.T) {
	Reset()
	defer Reset()
	
	// Set invalid config to cause load error
	os.Setenv("SHOUT_SERVER_PUBLIC_PORT", "-1")
	defer os.Unsetenv("SHOUT_SERVER_PUBLIC_PORT")
	
	// Try to load (will fail)
	_, _ = Load()
	
	// Now Get() should panic because load failed
	defer func() {
		if r := recover(); r == nil {
			t.Errorf("Get() did not panic when config loading failed")
		}
	}()
	
	Get()
}

func TestConfig_GetReturnsLoadedConfig(t *testing.T) {
	Reset()
	defer Reset()
	
	// Successfully load config
	cfg1, err := Load()
	if err != nil {
		t.Fatalf("Failed to load config: %v", err)
	}
	
	// Get should return the same instance
	cfg2 := Get()
	if cfg1 != cfg2 {
		t.Error("Get() did not return the same instance as Load()")
	}
	if cfg2.Version != "dev" {
		t.Errorf("Get() returned config with wrong version: %s", cfg2.Version)
	}
}

func TestConfig_MustLoad(t *testing.T) {
	// Ensure clean state
	Reset()
	os.Remove(".env") // Make sure no .env file exists
	defer Reset()

	cfg := MustLoad()
	if cfg == nil {
		t.Fatal("MustLoad() returned nil")
	}
	if cfg.Version != "dev" {
		t.Errorf("MustLoad() config has wrong version: got %s, want dev", cfg.Version)
	}
}

func TestConfig_MustLoadPanicsOnError(t *testing.T) {
	Reset()
	defer Reset()

	// Set invalid env to cause validation error
	os.Setenv("SHOUT_SERVER_PUBLIC_PORT", "-1")
	defer os.Unsetenv("SHOUT_SERVER_PUBLIC_PORT")

	defer func() {
		if r := recover(); r == nil {
			t.Errorf("MustLoad() did not panic on validation error")
		}
	}()

	MustLoad()
}

func TestConfig_LoadFromEnv(t *testing.T) {
	envVars := map[string]string{
		"SHOUT_VERSION":            "test-1.0",
		"SHOUT_SERVER_PUBLIC_PORT": "5000",
	}

	cfg, err := LoadFromEnv(envVars)
	if err != nil {
		t.Fatalf("LoadFromEnv failed: %v", err)
	}

	if cfg.Version != "test-1.0" {
		t.Errorf("Version = %s, want test-1.0", cfg.Version)
	}
	if cfg.Server.PublicPort != 5000 {
		t.Errorf("PublicPort = %d, want 5000", cfg.Server.PublicPort)
	}
}

func TestConfig_LoadErrorHandling(t *testing.T) {
	Reset()
	defer Reset()
	
	// Set invalid env to cause parse error
	os.Setenv("SHOUT_SERVER_PUBLIC_PORT", "not-a-number")
	defer os.Unsetenv("SHOUT_SERVER_PUBLIC_PORT")
	
	cfg, err := Load()
	if err == nil {
		t.Error("Expected error when parsing invalid port, got nil")
	}
	if cfg != nil {
		t.Error("Expected nil config on error")
	}
	if !strings.Contains(err.Error(), "failed to parse") {
		t.Errorf("Expected parse error, got: %v", err)
	}
}

// Helper function to split environment variable string
func splitEnvVar(envVar string) []string {
	for i := 0; i < len(envVar); i++ {
		if envVar[i] == '=' {
			return []string{envVar[:i], envVar[i+1:]}
		}
	}
	return []string{envVar, ""}
}
