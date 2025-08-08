package config

import (
	"fmt"
	"os"
	"sync"

	"github.com/caarlos0/env/v11"
	"github.com/joho/godotenv"
)

var (
	once     sync.Once
	instance *Config
	loadErr  error
)

// Config represents the complete application configuration.
// All settings use environment variables with SHOUT_ prefix.
// Default values are specified as struct tags.
type Config struct {
	Version string `env:"SHOUT_VERSION" envDefault:"dev"`

	Server    ServerConfig    `envPrefix:"SHOUT_SERVER_"`
	RateLimit RateLimitConfig `envPrefix:"SHOUT_RATELIMIT_"`
	Fonts     FontConfig      `envPrefix:"SHOUT_FONTS_"`
	Streaming StreamingConfig `envPrefix:"SHOUT_STREAMING_"`
	Text      TextConfig      `envPrefix:"SHOUT_TEXT_"`
}

// ServerConfig contains HTTP server settings
type ServerConfig struct {
	PublicPort int    `env:"PUBLIC_PORT" envDefault:"8080"`
	AdminPort  int    `env:"ADMIN_PORT" envDefault:"9090"`
	Host       string `env:"HOST" envDefault:"0.0.0.0"`
}

// RateLimitConfig contains rate limiting settings
type RateLimitConfig struct {
	RequestsPerMinute int `env:"REQUESTS_PER_MINUTE" envDefault:"100"`
	Burst             int `env:"BURST" envDefault:"10"`
}

// FontConfig contains font-related settings
type FontConfig struct {
	Default string   `env:"DEFAULT" envDefault:"standard"`
	Path    string   `env:"PATH" envDefault:"./fonts"`
	Allowed []string `env:"ALLOWED" envDefault:"standard,doom,banner,slant,3d,speed,starwars"`
}

// StreamingConfig contains streaming/animation settings
type StreamingConfig struct {
	DefaultTimeout int `env:"DEFAULT_TIMEOUT" envDefault:"30"`
	MaxTimeout     int `env:"MAX_TIMEOUT" envDefault:"300"`
	DefaultSpeed   int `env:"DEFAULT_SPEED" envDefault:"5"`
	BufferSize     int `env:"BUFFER_SIZE" envDefault:"4096"`
}

// TextConfig contains text processing settings
type TextConfig struct {
	MaxLength     int    `env:"MAX_LENGTH" envDefault:"100"`
	DefaultAlign  string `env:"DEFAULT_ALIGN" envDefault:"center"`
	DefaultBorder string `env:"DEFAULT_BORDER" envDefault:"none"`
}

// Load reads configuration from environment variables and .env file.
// It uses godotenv to load .env file (if exists) and caarlos0/env to parse
// environment variables into the config struct.
//
// The function is safe for concurrent use and returns a singleton instance.
//
// Example:
//
//	cfg, err := config.Load()
//	if err != nil {
//	    log.Fatal("Failed to load config:", err)
//	}
//	fmt.Printf("Server will run on port %d\n", cfg.Server.PublicPort)
func Load() (*Config, error) {
	once.Do(func() {
		instance = &Config{}

		// Try to load .env file if it exists (ignore error if not found)
		_ = godotenv.Load()

		// Parse environment variables into config struct
		if err := env.Parse(instance); err != nil {
			loadErr = fmt.Errorf("failed to parse environment variables: %w", err)
			instance = nil // Clear instance on error
			return
		}

		// Validate configuration
		if err := instance.Validate(); err != nil {
			loadErr = fmt.Errorf("configuration validation failed: %w", err)
			instance = nil // Clear instance on error
			return
		}
	})

	return instance, loadErr
}

// Get returns the singleton config instance.
// It panics if Load() hasn't been called or if loading failed.
//
// Example:
//
//	cfg := config.Get()
//	port := cfg.Server.PublicPort
func Get() *Config {
	if instance == nil {
		panic("config not loaded: call Load() first")
	}
	if loadErr != nil {
		panic(fmt.Sprintf("config loading failed: %v", loadErr))
	}
	return instance
}

// MustLoad loads the configuration and panics if it fails.
// This is useful in main() where configuration is critical.
//
// Example:
//
//	func main() {
//	    cfg := config.MustLoad()
//	    // Start server with cfg...
//	}
func MustLoad() *Config {
	cfg, err := Load()
	if err != nil {
		panic(fmt.Sprintf("failed to load configuration: %v", err))
	}
	return cfg
}

// Validate checks if the configuration values are valid.
// Returns an error if any validation fails.
func (c *Config) Validate() error {
	// Validate ports
	if c.Server.PublicPort < 1 || c.Server.PublicPort > 65535 {
		return fmt.Errorf("invalid port: public port must be between 1 and 65535, got %d", c.Server.PublicPort)
	}
	if c.Server.AdminPort < 1 || c.Server.AdminPort > 65535 {
		return fmt.Errorf("invalid port: admin port must be between 1 and 65535, got %d", c.Server.AdminPort)
	}

	// Validate rate limits
	if c.RateLimit.RequestsPerMinute < 1 {
		return fmt.Errorf("rate limit must be positive, got %d", c.RateLimit.RequestsPerMinute)
	}
	if c.RateLimit.Burst < 1 {
		return fmt.Errorf("rate limit burst must be positive, got %d", c.RateLimit.Burst)
	}

	// Validate text settings
	if c.Text.MaxLength < 1 {
		return fmt.Errorf("max text length must be positive, got %d", c.Text.MaxLength)
	}

	// Validate alignment
	validAlignments := map[string]bool{
		"left":   true,
		"center": true,
		"right":  true,
	}
	if !validAlignments[c.Text.DefaultAlign] {
		return fmt.Errorf("invalid alignment: must be left, center, or right, got %s", c.Text.DefaultAlign)
	}

	// Validate streaming settings
	if c.Streaming.DefaultTimeout < 1 {
		return fmt.Errorf("streaming timeout must be positive, got %d", c.Streaming.DefaultTimeout)
	}
	if c.Streaming.MaxTimeout < c.Streaming.DefaultTimeout {
		return fmt.Errorf("max timeout must be >= default timeout, got max=%d, default=%d",
			c.Streaming.MaxTimeout, c.Streaming.DefaultTimeout)
	}
	if c.Streaming.DefaultSpeed < 1 || c.Streaming.DefaultSpeed > 10 {
		return fmt.Errorf("streaming speed must be between 1 and 10, got %d", c.Streaming.DefaultSpeed)
	}

	return nil
}

// Reset resets the singleton instance (useful for testing).
// This should only be used in tests.
func Reset() {
	once = sync.Once{}
	instance = nil
	loadErr = nil
	// Also remove any .env file that might affect subsequent tests
	_ = os.Remove(".env")
}

// LoadFromEnv loads configuration from a specific set of environment variables.
// This is useful for testing with specific configurations.
func LoadFromEnv(envVars map[string]string) (*Config, error) {
	// Save current environment
	originalEnv := os.Environ()

	// Clear environment
	os.Clearenv()

	// Set provided environment variables
	for k, v := range envVars {
		os.Setenv(k, v)
	}

	// Reset singleton
	Reset()

	// Load configuration
	cfg, err := Load()

	// Restore original environment
	os.Clearenv()
	for _, e := range originalEnv {
		for i := 0; i < len(e); i++ {
			if e[i] == '=' {
				os.Setenv(e[:i], e[i+1:])
				break
			}
		}
	}

	return cfg, err
}
