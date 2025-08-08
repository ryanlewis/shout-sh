package types

import (
	"sync/atomic"
	"time"
)

// RenderOptions represents options for rendering ASCII art.
// These options control the visual appearance and behavior of the generated text.
//
// Usage example:
//
//	opts := RenderOptions{
//	    Font:  "doom",
//	    Color: "rainbow",
//	    Speed: 5,
//	}
type RenderOptions struct {
	Font     string `json:"font" query:"f,font"`
	Color    string `json:"color" query:"c,color"`
	MaxWidth int    `json:"maxwidth" query:"mw,maxwidth"`
	Timeout  int    `json:"timeout" query:"t,timeout"`
	Speed    int    `json:"speed" query:"s,speed"`
	Align    string `json:"align" query:"a,align"`
	Border   string `json:"border" query:"b,border"`
}

// ConnectionManager manages concurrent streaming connections.
// It enforces a maximum number of simultaneous streams to prevent resource exhaustion.
//
// The type is safe for concurrent use.
//
// Usage example:
//
//	cm := NewConnectionManager(100)
//	if cm.TryAcquire() {
//	    defer cm.Release()
//	    // Handle streaming connection
//	}
type ConnectionManager struct {
	activeStreams int64
	maxStreams    int64
}

// NewConnectionManager creates a new ConnectionManager with the specified maximum concurrent streams.
//
// Parameters:
//   - maxStreams: the maximum number of concurrent streaming connections allowed
//
// Returns:
//   - *ConnectionManager: a new connection manager instance
//
// Example:
//
//	cm := NewConnectionManager(100)
func NewConnectionManager(maxStreams int64) *ConnectionManager {
	return &ConnectionManager{
		maxStreams: maxStreams,
	}
}

// TryAcquire attempts to acquire a streaming connection slot.
//
// Returns:
//   - bool: true if a slot was acquired, false if at maximum capacity
//
// Example:
//
//	if cm.TryAcquire() {
//	    defer cm.Release()
//	    // Stream content
//	}
func (cm *ConnectionManager) TryAcquire() bool {
	current := atomic.LoadInt64(&cm.activeStreams)
	if current >= cm.maxStreams {
		return false
	}
	atomic.AddInt64(&cm.activeStreams, 1)
	return true
}

// Release releases a streaming connection slot.
// Should be called when a streaming connection ends.
//
// Example:
//
//	cm.Release()
func (cm *ConnectionManager) Release() {
	atomic.AddInt64(&cm.activeStreams, -1)
}

// GetActiveCount returns the current number of active streaming connections.
//
// Returns:
//   - int64: the number of active connections
//
// Example:
//
//	count := cm.GetActiveCount()
//	fmt.Printf("Active streams: %d\n", count)
func (cm *ConnectionManager) GetActiveCount() int64 {
	return atomic.LoadInt64(&cm.activeStreams)
}

// Config represents the complete application configuration.
// All settings are grouped into logical sections for better organization.
//
// Usage example:
//
//	cfg := Config{
//	    Server: ServerConfig{PublicPort: 8080},
//	    Fonts: FontConfig{DefaultFont: "standard"},
//	}
type Config struct {
	Server    ServerConfig    `yaml:"server"`
	RateLimit RateLimitConfig `yaml:"rateLimit"`
	Fonts     FontConfig      `yaml:"fonts"`
	Streaming StreamingConfig `yaml:"streaming"`
	Text      TextConfig      `yaml:"text"`
	Version   string          `yaml:"version"`
}

// ServerConfig contains HTTP server settings.
type ServerConfig struct {
	PublicPort        int           `yaml:"publicPort"`
	AdminPort         int           `yaml:"adminPort"`
	ReadTimeout       time.Duration `yaml:"readTimeout"`
	IdleTimeout       time.Duration `yaml:"idleTimeout"`
	MaxStreamDuration time.Duration `yaml:"maxStreamDuration"`
}

// RateLimitConfig contains rate limiting settings.
type RateLimitConfig struct {
	RequestsPerMinute int `yaml:"requestsPerMinute"`
	BurstSize         int `yaml:"burstSize"`
}

// FontConfig contains font-related settings.
type FontConfig struct {
	Directory   string   `yaml:"directory"`
	DefaultFont string   `yaml:"defaultFont"`
	Fonts       []string `yaml:"fonts"`
}

// StreamingConfig contains streaming animation settings.
type StreamingConfig struct {
	MaxConcurrentStreams int64         `yaml:"maxConcurrentStreams"`
	DefaultTimeout       time.Duration `yaml:"defaultTimeout"`
	DefaultSpeed         int           `yaml:"defaultSpeed"`
	MinSpeed             int           `yaml:"minSpeed"`
	MaxSpeed             int           `yaml:"maxSpeed"`
}

// TextConfig contains text processing settings.
type TextConfig struct {
	MaxLength     int    `yaml:"maxLength"`
	DefaultAlign  string `yaml:"defaultAlign"`
	DefaultBorder string `yaml:"defaultBorder"`
}

// Metrics tracks application metrics for monitoring.
// All fields should be accessed atomically for thread safety.
//
// Usage example:
//
//	atomic.AddInt64(&metrics.StaticRequests, 1)
//	count := atomic.LoadInt64(&metrics.StaticRequests)
type Metrics struct {
	StaticRequests  int64 `json:"staticRequests"`
	PartyRequests   int64 `json:"partyRequests"`
	FontRequests    int64 `json:"fontRequests"`
	RejectedStreams int64 `json:"rejectedStreams"`
	TotalErrors     int64 `json:"totalErrors"`
}
