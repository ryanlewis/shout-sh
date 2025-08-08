# shout.sh Implementation Guide

## Project Overview

**shout.sh** - A curl-able ASCII art text generator with animated rainbow effects. Transform any text into bold, attention-grabbing terminal output.

```bash
# Static banner
curl shout.sh/HELLO+WORLD

# Animated party mode
curl shout.sh/p/DEPLOY+SUCCESS?t=5
```

## Core Architecture

### Technology Stack
- **Language**: Go 1.21+
- **HTTP Framework**: Fiber v2 (gofiber.io)
- **ASCII Generation**: github.com/mbndr/figlet4go
- **Deployment**: Docker container
- **Hosting**: DigitalOcean/Linode/Hetzner VPS

### Port Configuration
- **Public Port**: 8080 (main service)
- **Admin Port**: 9090 (health, metrics, internal endpoints)

### Project Structure
```
shout-sh/
├── main.go
├── handlers/
│   ├── static.go      # Static banner generation
│   ├── party.go       # Animated party mode
│   ├── help.go        # Help endpoint
│   └── fonts.go       # Font listing/preview
├── render/
│   ├── figlet.go      # FIGlet wrapper
│   ├── colors.go      # Color schemes
│   └── animation.go   # Animation logic
├── fonts/
│   ├── doom.flf
│   ├── 3d.flf
│   ├── big.flf
│   └── ...
├── middleware/
│   ├── ratelimit.go
│   └── logging.go
├── Dockerfile
├── config/
│   ├── config.go      # Config struct and loader
│   ├── defaults.yaml  # Default configuration
│   └── config.yaml    # User overrides (gitignored)
├── docker-compose.yml
└── README.md
```

## Configuration Management

### Configuration Structure

```yaml
# config/defaults.yaml
server:
  public_port: 8080
  admin_port: 9090
  max_connections: 100
  max_stream_duration: 5m
  read_timeout: 5s
  idle_timeout: 120s

rate_limit:
  requests_per_minute: 100
  burst: 20

fonts:
  directory: ./fonts
  cache_size: 100
  fonts:
    - doom
    - 3d
    - big
    - bloody
    - standard
    - slant
    - small
    - shadow

logging:
  level: info
  format: json
```

### Configuration Loading

```go
package config

import (
    "fmt"
    "os"
    "time"
    
    "gopkg.in/yaml.v3"
)

// LoadConfig loads configuration from files and environment
func LoadConfig() (*Config, error) {
    config := &Config{}
    
    // Load defaults
    defaultData, err := os.ReadFile("config/defaults.yaml")
    if err != nil {
        return nil, fmt.Errorf("failed to read defaults: %w", err)
    }
    
    if err := yaml.Unmarshal(defaultData, config); err != nil {
        return nil, fmt.Errorf("failed to parse defaults: %w", err)
    }
    
    // Load user overrides if exists
    if userData, err := os.ReadFile("config/config.yaml"); err == nil {
        if err := yaml.Unmarshal(userData, config); err != nil {
            return nil, fmt.Errorf("failed to parse config: %w", err)
        }
    }
    
    // Apply environment variables
    applyEnvOverrides(config)
    
    // Validate configuration
    if err := validateConfig(config); err != nil {
        return nil, fmt.Errorf("invalid configuration: %w", err)
    }
    
    return config, nil
}

func applyEnvOverrides(config *Config) {
    if port := os.Getenv("SHOUT_PUBLIC_PORT"); port != "" {
        fmt.Sscanf(port, "%d", &config.Server.PublicPort)
    }
    
    if port := os.Getenv("SHOUT_ADMIN_PORT"); port != "" {
        fmt.Sscanf(port, "%d", &config.Server.AdminPort)
    }
    
    if maxConn := os.Getenv("SHOUT_MAX_CONNECTIONS"); maxConn != "" {
        fmt.Sscanf(maxConn, "%d", &config.Server.MaxConnections)
    }
    
    if rateLimit := os.Getenv("SHOUT_RATE_LIMIT"); rateLimit != "" {
        fmt.Sscanf(rateLimit, "%d", &config.RateLimit.RequestsPerMinute)
    }
    
    if logLevel := os.Getenv("SHOUT_LOG_LEVEL"); logLevel != "" {
        config.Logging.Level = logLevel
    }
}

func validateConfig(config *Config) error {
    if config.Server.PublicPort < 1 || config.Server.PublicPort > 65535 {
        return fmt.Errorf("invalid public port: %d", config.Server.PublicPort)
    }
    
    if config.Server.AdminPort < 1 || config.Server.AdminPort > 65535 {
        return fmt.Errorf("invalid admin port: %d", config.Server.AdminPort)
    }
    
    if config.Server.PublicPort == config.Server.AdminPort {
        return fmt.Errorf("public and admin ports cannot be the same")
    }
    
    if config.Server.MaxConnections < 1 {
        return fmt.Errorf("max_connections must be at least 1")
    }
    
    if config.RateLimit.RequestsPerMinute < 1 {
        return fmt.Errorf("rate limit must be at least 1 request per minute")
    }
    
    validLevels := map[string]bool{"debug": true, "info": true, "warn": true, "error": true}
    if !validLevels[config.Logging.Level] {
        return fmt.Errorf("invalid log level: %s", config.Logging.Level)
    }
    
    return nil
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SHOUT_PUBLIC_PORT` | Public API port | 8080 |
| `SHOUT_ADMIN_PORT` | Admin API port | 9090 |
| `SHOUT_MAX_CONNECTIONS` | Max concurrent streams | 100 |
| `SHOUT_RATE_LIMIT` | Requests per minute | 100 |
| `SHOUT_LOG_LEVEL` | Logging level (debug/info/warn/error) | info |
| `SHOUT_FONTS_DIR` | Directory containing font files | ./fonts |

## API Specification

### Endpoints

#### Static Banner
```
GET /{text}
GET /{text}?f={font}&c={color}&mw={width}
```

#### Party Mode (Animated)
```
GET /p/{text}
GET /party/{text}
```

#### Special Endpoints
```
GET /              # Shows help (CLI) or landing page (browser)
GET /help          # Detailed help/usage page
GET /fonts         # List available fonts
```

#### Admin Endpoints (Port 9090)
```
GET /health        # Health check
GET /metrics       # Prometheus metrics
GET /debug/pprof   # Go profiling (dev only)
```

### Query Parameters

| Parameter | Alias | Type | Default | Description |
|-----------|-------|------|---------|-------------|
| `font` | `f` | string | `doom` | Font style |
| `color` | `c` | string | none/`rainbow` | Color scheme |
| `maxwidth` | `mw` | int | unlimited | Max character width |
| `timeout` | `t` | int | 0 (infinite) | Animation timeout (seconds) |
| `speed` | `s` | int | 5 | Animation speed (1-10) |
| `align` | `a` | string | `left` | Text alignment |
| `border` | `b` | string | none | Border style |

## Type Definitions

### Core Types

```go
package types

import (
    "sync"
    "sync/atomic"
    "time"
    "github.com/mbndr/figlet4go"
)

// RenderOptions contains all options for rendering ASCII art
type RenderOptions struct {
    Font     string `json:"font" query:"f,font"`
    Color    string `json:"color" query:"c,color"`
    MaxWidth int    `json:"maxwidth" query:"mw,maxwidth"`
    Timeout  int    `json:"timeout" query:"t,timeout"`
    Speed    int    `json:"speed" query:"s,speed"`
    Align    string `json:"align" query:"a,align"`
    Border   string `json:"border" query:"b,border"`
}

// ConnectionManager manages active streaming connections
type ConnectionManager struct {
    activeStreams int64
    maxStreams    int64
    mu            sync.RWMutex
}

// NewConnectionManager creates a new connection manager
func NewConnectionManager(maxStreams int64) *ConnectionManager {
    return &ConnectionManager{
        maxStreams: maxStreams,
    }
}

// TryAcquire attempts to acquire a connection slot
func (cm *ConnectionManager) TryAcquire() bool {
    if atomic.LoadInt64(&cm.activeStreams) >= cm.maxStreams {
        return false
    }
    atomic.AddInt64(&cm.activeStreams, 1)
    return true
}

// Release releases a connection slot
func (cm *ConnectionManager) Release() {
    atomic.AddInt64(&cm.activeStreams, -1)
}

// GetActiveCount returns the current number of active streams
func (cm *ConnectionManager) GetActiveCount() int64 {
    return atomic.LoadInt64(&cm.activeStreams)
}

// FontCache manages loaded FIGlet fonts
type FontCache struct {
    fonts map[string]*figlet4go.AsciiRender
    mu    sync.RWMutex
}

// Metrics tracks application metrics
type Metrics struct {
    StaticRequests  int64
    PartyRequests   int64
    HelpRequests    int64
    FontRequests    int64
    ActiveStreams   int64
    RejectedStreams int64
    TotalErrors     int64
}

// Config holds application configuration
type Config struct {
    Server    ServerConfig    `yaml:"server"`
    RateLimit RateLimitConfig `yaml:"rate_limit"`
    Fonts     FontsConfig     `yaml:"fonts"`
    Logging   LoggingConfig   `yaml:"logging"`
}

type ServerConfig struct {
    PublicPort        int           `yaml:"public_port" env:"SHOUT_PUBLIC_PORT"`
    AdminPort         int           `yaml:"admin_port" env:"SHOUT_ADMIN_PORT"`
    MaxConnections    int           `yaml:"max_connections" env:"SHOUT_MAX_CONNECTIONS"`
    MaxStreamDuration time.Duration `yaml:"max_stream_duration"`
    ReadTimeout       time.Duration `yaml:"read_timeout"`
    IdleTimeout       time.Duration `yaml:"idle_timeout"`
}

type RateLimitConfig struct {
    RequestsPerMinute int `yaml:"requests_per_minute" env:"SHOUT_RATE_LIMIT"`
    Burst            int `yaml:"burst"`
}

type FontsConfig struct {
    Directory string   `yaml:"directory"`
    CacheSize int      `yaml:"cache_size"`
    Fonts     []string `yaml:"fonts"`
}

type LoggingConfig struct {
    Level  string `yaml:"level" env:"SHOUT_LOG_LEVEL"`
    Format string `yaml:"format"`
}
```

### Constants

```go
package constants

import "time"

const (
    // Default values
    DefaultFont          = "doom"
    DefaultSpeed         = 5
    DefaultAlignment     = "left"
    DefaultMaxWidth      = 0 // unlimited
    
    // Limits
    MaxTextLength        = 100
    MaxStreamDuration    = 5 * time.Minute
    DefaultStreamTimeout = 5 * time.Minute
    MaxConcurrentStreams = 100
    
    // Animation
    MinSpeed = 1
    MaxSpeed = 10
    
    // HTTP
    DefaultPublicPort = 8080
    DefaultAdminPort  = 9090
    
    // Rate limiting
    DefaultRateLimit = 100
    DefaultBurst     = 20
    
    // Version
    Version = "1.0.0"
)

// ANSI color codes for terminal output
const (
    AnsiReset      = "\033[0m"
    AnsiClearScreen = "\033[2J"
    AnsiHome       = "\033[H"
)
```

## Implementation Details

### Main Server Setup

```go
package main

import (
    "context"
    "fmt"
    "log"
    "os"
    "os/signal"
    "syscall"
    "time"
    
    "github.com/gofiber/fiber/v2"
    "github.com/gofiber/fiber/v2/middleware/cors"
    "github.com/gofiber/fiber/v2/middleware/logger"
    "github.com/gofiber/fiber/v2/middleware/recover"
    "github.com/gofiber/fiber/v2/middleware/limiter"
)

var (
    connManager *ConnectionManager
    fontCache   *FontCache
    metrics     *Metrics
    config      *Config
    startTime   time.Time
)

func main() {
    var err error
    startTime = time.Now()
    
    // Load configuration
    config, err = LoadConfig()
    if err != nil {
        log.Fatalf("Failed to load config: %v", err)
    }
    
    // Initialize components
    connManager = NewConnectionManager(int64(config.Server.MaxConnections))
    metrics = &Metrics{}
    
    // Load fonts
    if err := loadFonts(); err != nil {
        log.Fatalf("Failed to load fonts: %v", err)
    }
    
    // Public app
    app := fiber.New(fiber.Config{
        ServerHeader:          "shout.sh",
        AppName:              "shout.sh",
        DisableStartupMessage: false,
        // Streaming support
        StreamRequestBody:     true,
        // Disable HTTP/2 for CLI compatibility
        DisableHTTP2:         true,
        // Timeouts
        ReadTimeout:          config.Server.ReadTimeout,
        IdleTimeout:          config.Server.IdleTimeout,
        // Error handler
        ErrorHandler: customErrorHandler,
    })
    
    // Middleware
    app.Use(recover.New(recover.Config{
        EnableStackTrace: config.Logging.Level == "debug",
    }))
    
    // CORS for browser support
    app.Use(cors.New(cors.Config{
        AllowOrigins: "*",
        AllowMethods: "GET",
    }))
    
    app.Use(logger.New(logger.Config{
        Format: "${time} ${ip} ${method} ${path} ${status} ${latency}\n",
        TimeFormat: "2006-01-02 15:04:05",
    }))
    
    // Rate limiting
    app.Use(limiter.New(limiter.Config{
        Max:        config.RateLimit.RequestsPerMinute,
        Expiration: 1 * time.Minute,
        KeyGenerator: func(c *fiber.Ctx) string {
            return c.IP()
        },
        LimitReached: func(c *fiber.Ctx) error {
            atomic.AddInt64(&metrics.RejectedStreams, 1)
            return c.Status(429).SendString("Rate limit exceeded. Try again in a minute.")
        },
        SkipSuccessfulRequests: false,
    }))
    
    // Routes
    app.Get("/", rootHandler)
    app.Get("/help", helpHandler)
    app.Get("/fonts", fontsHandler)
    app.Get("/p/*", partyHandler)
    app.Get("/party/*", partyHandler)
    app.Get("/*", staticHandler) // Catch-all for text input
    
    // Admin app (port 9090)
    admin := fiber.New(fiber.Config{
        ServerHeader: "shout.sh-admin",
    })
    
    admin.Get("/health", healthHandler)
    admin.Get("/metrics", metricsHandler)
    
    // Graceful shutdown
    go gracefulShutdown(app, admin)
    
    // Start servers
    go func() {
        addr := fmt.Sprintf(":%d", config.Server.AdminPort)
        log.Printf("Starting admin server on %s", addr)
        if err := admin.Listen(addr); err != nil {
            log.Fatalf("Admin server failed: %v", err)
        }
    }()
    
    addr := fmt.Sprintf(":%d", config.Server.PublicPort)
    log.Printf("Starting shout.sh on %s (HTTP/1.1 only)", addr)
    if err := app.Listen(addr); err != nil {
        log.Fatalf("Server failed: %v", err)
    }
}
```

### Root Handler with Client Detection

```go
func rootHandler(c *fiber.Ctx) error {
    // If path is not exactly "/", treat as text input
    if c.Path() != "/" {
        return staticHandler(c)
    }
    
    // Detect client type
    if isBrowser(c) {
        return serveLandingPage(c)
    }
    return serveCliHelp(c)
}

func isBrowser(c *fiber.Ctx) bool {
    accept := c.Get("Accept")
    ua := string(c.Context().UserAgent())
    
    // Check for browser indicators
    hasHtmlAccept := strings.Contains(accept, "text/html")
    hasBrowserUA := strings.Contains(strings.ToLower(ua), "mozilla") || 
                    strings.Contains(strings.ToLower(ua), "chrome") || 
                    strings.Contains(strings.ToLower(ua), "safari")
    
    return hasHtmlAccept && hasBrowserUA
}

func serveCliHelp(c *fiber.Ctx) error {
    c.Set("Content-Type", "text/plain; charset=utf-8")
    return c.SendString(cliHelpText)
}

func serveLandingPage(c *fiber.Ctx) error {
    c.Set("Content-Type", "text/html; charset=utf-8")
    return c.SendString(landingPageHTML)
}
```

### Static Handler

```go
func staticHandler(c *fiber.Ctx) error {
    // Track metrics
    atomic.AddInt64(&metrics.StaticRequests, 1)
    
    // Get text from path
    text := c.Params("*")
    if text == "" {
        text = strings.TrimPrefix(c.Path(), "/")
    }
    
    // Validate input
    if text == "" {
        return c.Status(400).SendString("Error: No text provided")
    }
    
    // Parse query parameters
    opts := parseOptions(c)
    
    // Validate options
    if err := validateOptions(opts); err != nil {
        return c.Status(400).SendString(fmt.Sprintf("Error: %v", err))
    }
    
    // Clean and process text
    text = strings.ReplaceAll(text, "+", " ")
    text = cleanForFiglet(text)
    
    if len(text) == 0 {
        return c.Status(400).SendString("Error: Text contains no valid characters")
    }
    
    // Generate ASCII art
    output, err := generateASCII(text, opts)
    if err != nil {
        atomic.AddInt64(&metrics.TotalErrors, 1)
        log.Printf("Error generating ASCII: %v", err)
        return c.Status(500).SendString("Error generating ASCII art")
    }
    
    // Apply color if specified
    if opts.Color != "" && opts.Color != "none" {
        output, err = applyStaticColor(output, opts.Color)
        if err != nil {
            log.Printf("Error applying color: %v", err)
            // Continue without color rather than failing
        }
    }
    
    // Send response
    c.Set("Content-Type", "text/plain; charset=utf-8")
    return c.SendString(output)
}

func parseOptions(c *fiber.Ctx) RenderOptions {
    return RenderOptions{
        Font:     firstOf(c.Query("f"), c.Query("font"), "doom"),
        Color:    firstOf(c.Query("c"), c.Query("color"), ""),
        MaxWidth: c.QueryInt("mw", c.QueryInt("maxwidth", 0)),
        Timeout:  c.QueryInt("t", c.QueryInt("timeout", 0)),
        Speed:    c.QueryInt("s", c.QueryInt("speed", 5)),
        Align:    firstOf(c.Query("a"), c.Query("align"), "left"),
        Border:   firstOf(c.Query("b"), c.Query("border"), ""),
    }
}
```

### Helper Functions

```go
// firstOf returns the first non-empty string from the arguments
func firstOf(values ...string) string {
    for _, v := range values {
        if v != "" {
            return v
        }
    }
    return ""
}

// calculateFrameDelay calculates the delay between animation frames
func calculateFrameDelay(speed int) time.Duration {
    if speed < MinSpeed {
        speed = MinSpeed
    }
    if speed > MaxSpeed {
        speed = MaxSpeed
    }
    
    // Inverse relationship: higher speed = shorter delay
    // Speed 1 = 200ms, Speed 10 = 20ms
    delay := 220 - (speed * 20)
    return time.Duration(delay) * time.Millisecond
}

// getAnimatedColor returns the color for a character at the given position
func getAnimatedColor(scheme string, frame, line, col int) int {
    switch scheme {
    case "rainbow":
        colors := []int{196, 202, 208, 214, 220, 226, 190, 154, 118, 82, 46, 47, 48, 49, 50, 51}
        position := (frame + col*2) % len(colors)
        return colors[position]
    
    case "fire":
        colors := []int{52, 88, 124, 160, 196, 202, 208, 214, 220, 226}
        position := (frame + line*3 + col) % len(colors)
        return colors[position]
    
    case "ocean":
        colors := []int{17, 18, 19, 20, 21, 25, 31, 37, 43, 49, 50, 51}
        position := (frame + line + col*2) % len(colors)
        return colors[position]
    
    case "matrix":
        if (frame+line+col)%17 == 0 {
            return 46 // Bright green
        }
        return 28 // Dark green
    
    default:
        return getAnimatedColor("rainbow", frame, line, col)
    }
}

// validateOptions validates render options
func validateOptions(opts RenderOptions) error {
    // Validate font
    validFonts := map[string]bool{
        "doom": true, "3d": true, "big": true, "bloody": true,
        "standard": true, "slant": true, "small": true, "shadow": true,
    }
    if opts.Font != "" && !validFonts[opts.Font] {
        return fmt.Errorf("invalid font: %s", opts.Font)
    }
    
    // Validate speed
    if opts.Speed < 0 || opts.Speed > MaxSpeed {
        return fmt.Errorf("speed must be between 1 and %d", MaxSpeed)
    }
    
    // Validate alignment
    validAligns := map[string]bool{"left": true, "center": true, "right": true}
    if opts.Align != "" && !validAligns[opts.Align] {
        return fmt.Errorf("invalid alignment: %s", opts.Align)
    }
    
    return nil
}

// generateASCII generates ASCII art from text
func generateASCII(text string, opts RenderOptions) (string, error) {
    renderer, exists := fontCache.fonts[opts.Font]
    if !exists {
        renderer = fontCache.fonts[DefaultFont]
        if renderer == nil {
            return "", fmt.Errorf("no fonts loaded")
        }
    }
    
    ascii, err := renderer.Render(text)
    if err != nil {
        return "", fmt.Errorf("failed to render text: %w", err)
    }
    
    // Apply alignment if needed
    if opts.Align == "center" || opts.Align == "right" {
        ascii = alignText(ascii, opts.Align, opts.MaxWidth)
    }
    
    // Apply border if specified
    if opts.Border != "" {
        ascii = addBorder(ascii, opts.Border)
    }
    
    return ascii, nil
}

// applyStaticColor applies color to static ASCII art
func applyStaticColor(text string, color string) (string, error) {
    switch color {
    case "red":
        return fmt.Sprintf("\033[31m%s\033[0m", text), nil
    case "green":
        return fmt.Sprintf("\033[32m%s\033[0m", text), nil
    case "blue":
        return fmt.Sprintf("\033[34m%s\033[0m", text), nil
    case "yellow":
        return fmt.Sprintf("\033[33m%s\033[0m", text), nil
    case "magenta":
        return fmt.Sprintf("\033[35m%s\033[0m", text), nil
    case "cyan":
        return fmt.Sprintf("\033[36m%s\033[0m", text), nil
    default:
        return text, nil
    }
}

// alignText aligns ASCII art text
func alignText(text, align string, width int) string {
    if width <= 0 {
        return text
    }
    
    lines := strings.Split(text, "\n")
    aligned := make([]string, len(lines))
    
    for i, line := range lines {
        lineLen := len(line)
        if lineLen >= width {
            aligned[i] = line
            continue
        }
        
        switch align {
        case "center":
            padding := (width - lineLen) / 2
            aligned[i] = strings.Repeat(" ", padding) + line
        case "right":
            padding := width - lineLen
            aligned[i] = strings.Repeat(" ", padding) + line
        default:
            aligned[i] = line
        }
    }
    
    return strings.Join(aligned, "\n")
}

// addBorder adds a border around ASCII art
func addBorder(text, style string) string {
    lines := strings.Split(text, "\n")
    maxLen := 0
    for _, line := range lines {
        if len(line) > maxLen {
            maxLen = len(line)
        }
    }
    
    var top, bottom, left, right string
    switch style {
    case "box":
        top = "+" + strings.Repeat("-", maxLen+2) + "+"
        bottom = top
        left = "| "
        right = " |"
    case "double":
        top = "╔" + strings.Repeat("═", maxLen+2) + "╗"
        bottom = "╚" + strings.Repeat("═", maxLen+2) + "╝"
        left = "║ "
        right = " ║"
    default:
        return text
    }
    
    bordered := []string{top}
    for _, line := range lines {
        padding := strings.Repeat(" ", maxLen-len(line))
        bordered = append(bordered, left+line+padding+right)
    }
    bordered = append(bordered, bottom)
    
    return strings.Join(bordered, "\n")
}
```

### Party Mode Handler (Streaming)

```go
func partyHandler(c *fiber.Ctx) error {
    // Track metrics
    atomic.AddInt64(&metrics.PartyRequests, 1)
    
    // Check connection limit
    if !connManager.TryAcquire() {
        atomic.AddInt64(&metrics.RejectedStreams, 1)
        return c.Status(503).SendString("Error: Too many active streams. Please try again later.")
    }
    defer connManager.Release()
    
    // Get text from path
    text := c.Params("*")
    if text == "" {
        return c.Status(400).SendString("Error: No text provided")
    }
    
    text = strings.ReplaceAll(text, "+", " ")
    text = cleanForFiglet(text)
    
    if len(text) == 0 {
        return c.Status(400).SendString("Error: Text contains no valid characters")
    }
    
    opts := parseOptions(c)
    
    // Validate options
    if err := validateOptions(opts); err != nil {
        return c.Status(400).SendString(fmt.Sprintf("Error: %v", err))
    }
    
    // Apply timeout limits
    if opts.Timeout == 0 || opts.Timeout > int(config.Server.MaxStreamDuration.Seconds()) {
        opts.Timeout = int(DefaultStreamTimeout.Seconds())
    }
    
    // Stream animated output
    return streamAnimated(c, text, opts)
}

func streamAnimated(c *fiber.Ctx, text string, opts RenderOptions) error {
    // Update metrics
    atomic.AddInt64(&metrics.ActiveStreams, 1)
    defer atomic.AddInt64(&metrics.ActiveStreams, -1)
    
    // Generate ASCII art
    ascii, err := generateASCII(text, opts)
    if err != nil {
        atomic.AddInt64(&metrics.TotalErrors, 1)
        log.Printf("Error generating ASCII: %v", err)
        return c.Status(500).SendString("Error generating ASCII art")
    }
    
    lines := strings.Split(strings.TrimRight(ascii, "\n"), "\n")
    
    // Setup streaming response
    c.Set("Content-Type", "text/plain; charset=utf-8")
    c.Set("Cache-Control", "no-cache")
    c.Set("Transfer-Encoding", "chunked")
    c.Set("X-Content-Type-Options", "nosniff")
    
    // Setup timeout with maximum limit
    timeoutDuration := DefaultStreamTimeout
    if opts.Timeout > 0 {
        timeoutDuration = time.Duration(opts.Timeout) * time.Second
        if timeoutDuration > config.Server.MaxStreamDuration {
            timeoutDuration = config.Server.MaxStreamDuration
        }
    }
    timeout := time.NewTimer(timeoutDuration)
    defer timeout.Stop()
    
    // Animation settings
    frameDelay := calculateFrameDelay(opts.Speed)
    frame := 0
    
    // Clear screen once
    c.Context().SetBodyStreamWriter(func(w *bufio.Writer) {
        // Initial clear screen
        fmt.Fprintf(w, "\033[2J\033[H")
        w.Flush()
        
        // Animation loop
        ticker := time.NewTicker(frameDelay)
        defer ticker.Stop()
        
        for {
            select {
            case <-timeout.C:
                // Timeout reached
                fmt.Fprintf(w, "\033[0m") // Reset colors
                w.Flush()
                return
                
            case <-ticker.C:
                // Render frame
                fmt.Fprintf(w, "\033[H") // Move cursor home
                renderAnimatedFrame(w, lines, frame, opts)
                
                if err := w.Flush(); err != nil {
                    // Client disconnected
                    return
                }
                
                frame++
            }
        }
    })
    
    return nil
}

func renderAnimatedFrame(w io.Writer, lines []string, frame int, opts RenderOptions) {
    for lineNum, line := range lines {
        for charPos, char := range line {
            if char != ' ' {
                color := getAnimatedColor(opts.Color, frame, lineNum, charPos)
                fmt.Fprintf(w, "\033[38;5;%dm%c\033[0m", color, char)
            } else {
                fmt.Fprint(w, " ")
            }
        }
        fmt.Fprintln(w)
    }
}
```

### Help Handler

```go
func helpHandler(c *fiber.Ctx) error {
    if isBrowser(c) {
        return serveHelpPage(c)
    }
    return serveDetailedCliHelp(c)
}

func serveDetailedCliHelp(c *fiber.Ctx) error {
    c.Set("Content-Type", "text/plain; charset=utf-8")
    return c.SendString(detailedHelpText)
}

func serveHelpPage(c *fiber.Ctx) error {
    c.Set("Content-Type", "text/html; charset=utf-8")
    return c.SendString(helpPageHTML)
}
```

### Health Check (Admin Port)

```go
func healthHandler(c *fiber.Ctx) error {
    status := fiber.Map{
        "status":      "healthy",
        "timestamp":   time.Now(),
        "fonts_loaded": len(fontCache),
        "uptime":      time.Since(startTime).String(),
        "version":     VERSION,
    }
    
    return c.JSON(status)
}

func metricsHandler(c *fiber.Ctx) error {
    c.Set("Content-Type", "text/plain")
    
    metrics := fmt.Sprintf(`# HELP shout_requests_total Total number of requests
# TYPE shout_requests_total counter
shout_requests_total{endpoint="static"} %d
shout_requests_total{endpoint="party"} %d
shout_requests_total{endpoint="help"} %d

# HELP shout_active_streams Current number of active party mode streams
# TYPE shout_active_streams gauge
shout_active_streams %d
`, getMetrics())
    
    return c.SendString(metrics)
}
```

### Dependencies (go.mod)

```go
module github.com/ryanlewis/shout-sh

go 1.21

require (
    github.com/gofiber/fiber/v2 v2.52.0
    github.com/mbndr/figlet4go v0.0.0-20190224160619-d6cef5b186ea
)

require (
    github.com/andybalholm/brotli v1.0.5 // indirect
    github.com/google/uuid v1.5.0 // indirect
    github.com/klauspost/compress v1.17.0 // indirect
    github.com/mattn/go-colorable v0.1.13 // indirect
    github.com/mattn/go-isatty v0.0.20 // indirect
    github.com/mattn/go-runewidth v0.0.15 // indirect
    github.com/philhofer/fwd v1.1.2 // indirect
    github.com/rivo/uniseg v0.4.4 // indirect
    github.com/savsgio/dictpool v0.0.0-20221023140959-7bf2e61cea94 // indirect
    github.com/savsgio/gotils v0.0.0-20230208104028-c358bd845dee // indirect
    github.com/tinylib/msgp v1.1.8 // indirect
    github.com/valyala/bytebufferpool v1.0.0 // indirect
    github.com/valyala/fasthttp v1.50.0 // indirect
    github.com/valyala/tcplisten v1.0.0 // indirect
    golang.org/x/sys v0.14.0 // indirect
)
```

### Color Schemes

```go
type ColorScheme interface {
    GetColor(frame, line, column int) int
}

type RainbowScheme struct{}

func (r RainbowScheme) GetColor(frame, line, col int) int {
    colors := []int{196, 202, 208, 214, 220, 226, 190, 154, 118, 82, 46, 47, 48, 49, 50, 51}
    position := (frame + col*2) % len(colors)
    return colors[position]
}

type FireScheme struct{}

func (f FireScheme) GetColor(frame, line, col int) int {
    colors := []int{52, 88, 124, 160, 196, 202, 208, 214, 220, 226}
    position := (frame + line*3 + col) % len(colors)
    return colors[position]
}

type MatrixScheme struct{}

func (m MatrixScheme) GetColor(frame, line, col int) int {
    if (frame+line+col)%17 == 0 {
        return 46 // Bright green
    }
    return 28 // Dark green
}
```

### Resource Management

```go
// Connection management with limits
func (cm *ConnectionManager) Middleware() fiber.Handler {
    return func(c *fiber.Ctx) error {
        // Only apply to streaming endpoints
        if !strings.HasPrefix(c.Path(), "/p/") && !strings.HasPrefix(c.Path(), "/party/") {
            return c.Next()
        }
        
        if !cm.TryAcquire() {
            atomic.AddInt64(&metrics.RejectedStreams, 1)
            return c.Status(503).SendString("Service temporarily unavailable. Too many active connections.")
        }
        
        defer cm.Release()
        return c.Next()
    }
}

// Graceful shutdown implementation
func gracefulShutdown(app *fiber.App, admin *fiber.App) {
    quit := make(chan os.Signal, 1)
    signal.Notify(quit, os.Interrupt, syscall.SIGTERM)
    
    <-quit
    log.Println("Shutting down servers...")
    
    // Create shutdown context with timeout
    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()
    
    // Shutdown both servers
    var shutdownErr error
    done := make(chan struct{}, 2)
    
    go func() {
        if err := app.ShutdownWithContext(ctx); err != nil {
            shutdownErr = err
        }
        done <- struct{}{}
    }()
    
    go func() {
        if err := admin.ShutdownWithContext(ctx); err != nil {
            shutdownErr = err
        }
        done <- struct{}{}
    }()
    
    // Wait for both servers
    <-done
    <-done
    
    if shutdownErr != nil {
        log.Printf("Error during shutdown: %v", shutdownErr)
        os.Exit(1)
    }
    
    // Wait for active streams to complete
    maxWait := time.Now().Add(10 * time.Second)
    for connManager.GetActiveCount() > 0 && time.Now().Before(maxWait) {
        log.Printf("Waiting for %d active streams to complete...", connManager.GetActiveCount())
        time.Sleep(1 * time.Second)
    }
    
    if connManager.GetActiveCount() > 0 {
        log.Printf("Warning: Shutting down with %d active streams", connManager.GetActiveCount())
    }
    
    log.Println("Server shutdown complete")
    os.Exit(0)
}

// Custom error handler
func customErrorHandler(c *fiber.Ctx, err error) error {
    code := fiber.StatusInternalServerError
    message := "Internal Server Error"
    
    if e, ok := err.(*fiber.Error); ok {
        code = e.Code
        message = e.Message
    }
    
    // Log error
    if code >= 500 {
        log.Printf("Error %d: %v", code, err)
        atomic.AddInt64(&metrics.TotalErrors, 1)
    }
    
    // Return appropriate response based on Accept header
    if strings.Contains(c.Get("Accept"), "application/json") {
        return c.Status(code).JSON(fiber.Map{
            "error": message,
            "code":  code,
        })
    }
    
    return c.Status(code).SendString(fmt.Sprintf("Error %d: %s", code, message))
}
```

### Input Sanitization

```go
func cleanForFiglet(text string) string {
    // Remove emojis and non-ASCII
    text = strings.Map(func(r rune) rune {
        if r >= 32 && r <= 126 { // Printable ASCII range
            return r
        }
        if r == '\n' || r == '\t' {
            return r
        }
        return -1 // Remove
    }, text)
    
    // Limit length
    if len(text) > 100 {
        text = text[:100]
    }
    
    return text
}
```

### Rate Limiting

```go
type RateLimiter struct {
    visitors map[string]*visitor
    mu       sync.RWMutex
}

type visitor struct {
    lastSeen time.Time
    count    int
}

func (rl *RateLimiter) Allow(ip string) bool {
    rl.mu.Lock()
    defer rl.mu.Unlock()
    
    v, exists := rl.visitors[ip]
    if !exists {
        rl.visitors[ip] = &visitor{
            lastSeen: time.Now(),
            count:    1,
        }
        return true
    }
    
    // Reset count after 1 minute
    if time.Since(v.lastSeen) > time.Minute {
        v.count = 1
        v.lastSeen = time.Now()
        return true
    }
    
    // Check limit (100 req/min)
    if v.count >= 100 {
        return false
    }
    
    v.count++
    return true
}
```

## Fonts Configuration

### Required Fonts
1. **doom** - Heavy metal style (default)
2. **3d** - Three dimensional effect
3. **big** - Extra large characters
4. **bloody** - Horror/dripping effect
5. **standard** - Clean FIGlet default
6. **slant** - Italicized appearance
7. **small** - Compact version
8. **shadow** - With shadow effects

### Font Loading

```go
var fontCache map[string]*figlet4go.AsciiRender

func loadFonts() {
    fontCache = make(map[string]*figlet4go.AsciiRender)
    
    fonts := []string{"doom", "3d", "big", "bloody", "standard", "slant", "small", "shadow"}
    
    for _, name := range fonts {
        renderer := figlet4go.NewAsciiRender()
        fontPath := fmt.Sprintf("./fonts/%s.flf", name)
        
        if err := renderer.LoadFont(fontPath); err != nil {
            log.Printf("Warning: Could not load font %s: %v", name, err)
            continue
        }
        
        fontCache[name] = renderer
    }
}
```

### Font Acquisition

#### Where to Get FIGlet Fonts

FIGlet fonts are text files with the `.flf` (FIGlet Font) extension that define how characters are rendered as ASCII art.

**Official Sources:**
1. **FIGlet Official Repository**: https://github.com/cmatsuoka/figlet
   - Contains the standard font collection
   - Includes contributed fonts
   - Well-documented font format specification

2. **FIGlet Fonts Collection**: http://www.figlet.org/fontdb.cgi
   - Comprehensive database of fonts
   - Preview capability for each font
   - Categorized by style (3D, script, block, etc.)

3. **Toilet Fonts**: https://github.com/cacalabs/toilet
   - Extended font collection
   - Unicode support in some fonts
   - Additional rendering effects

**Font Licensing:**
- Most FIGlet fonts are in the public domain or under permissive licenses
- Always check the header of the .flf file for licensing information
- Commercial use is generally permitted for standard fonts
- Attribution may be required for some contributed fonts

**Font File Structure:**
```
flf2a$ 8 6 15 -1 16
FIGlet font file format example
$$@ 
$$@ 
$$@ 
$$@ 
$$@ 
$$@ 
$$@ 
$$@@
```

**Downloading Fonts:**
```bash
# Clone official FIGlet repository
git clone https://github.com/cmatsuoka/figlet.git
cp figlet/fonts/*.flf ./fonts/

# Or download individual fonts
wget https://raw.githubusercontent.com/cmatsuoka/figlet/master/fonts/doom.flf -O fonts/doom.flf
wget https://raw.githubusercontent.com/cmatsuoka/figlet/master/fonts/3d.flf -O fonts/3d.flf
```

**Font Validation:**
```go
func validateFontFile(path string) error {
    file, err := os.Open(path)
    if err != nil {
        return err
    }
    defer file.Close()
    
    scanner := bufio.NewScanner(file)
    if !scanner.Scan() {
        return fmt.Errorf("empty font file")
    }
    
    // Check FIGlet header
    header := scanner.Text()
    if !strings.HasPrefix(header, "flf2a") {
        return fmt.Errorf("invalid FIGlet font header")
    }
    
    return nil
}
```

## Deployment

### Dockerfile

```dockerfile
# Multi-stage build for smaller image
FROM golang:1.21 AS builder

WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download

COPY . .

# Build for multiple architectures
ARG TARGETOS=linux
ARG TARGETARCH
RUN CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
    go build -ldflags="-w -s" -o shout .

# Use scratch or distroless for minimal attack surface
FROM gcr.io/distroless/static-debian12:nonroot

# Copy binary and fonts
COPY --from=builder /app/shout /shout
COPY --from=builder /app/fonts /fonts

# Run as non-root user (65532 is nonroot user in distroless)
USER nonroot

EXPOSE 8080 9090
ENTRYPOINT ["/shout"]
```

### Alternative: Scratch Image (Even Smaller)

```dockerfile
FROM golang:1.21 AS builder

WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download

COPY . .

ARG TARGETOS=linux
ARG TARGETARCH
RUN CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
    go build -ldflags="-w -s" -o shout .

# Scratch - absolutely minimal
FROM scratch

COPY --from=builder /app/shout /shout
COPY --from=builder /app/fonts /fonts

EXPOSE 8080 9090
ENTRYPOINT ["/shout"]
```

### Multi-Architecture Build

```bash
# Build and push multi-arch image
docker buildx create --use
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag shout/shout:latest \
  --push .
```

### Docker Compose

```yaml
version: '3.8'

services:
  shout:
    build: .
    ports:
      - "80:8080"      # Public port
      - "9090:9090"    # Admin port (only expose internally)
    restart: unless-stopped
    environment:
      - PUBLIC_PORT=8080
      - ADMIN_PORT=9090
      - LOG_LEVEL=info
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:9090/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

### Nginx Configuration (Optional)

```nginx
server {
    listen 80;
    server_name shout.sh www.shout.sh;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        
        # Important for streaming
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400;
    }
    
    # Rate limiting at nginx level
    limit_req_zone $binary_remote_addr zone=shout:10m rate=100r/m;
    limit_req zone=shout burst=20 nodelay;
}
```

## Testing

### Unit Tests

```go
func TestCleanForFiglet(t *testing.T) {
    tests := []struct {
        input    string
        expected string
    }{
        {"HELLO", "HELLO"},
        {"Hello 🔥 World", "Hello  World"},
        {"Test@#$%", "Test@#$%"},
        {strings.Repeat("A", 200), strings.Repeat("A", 100)},
    }
    
    for _, tt := range tests {
        result := cleanForFiglet(tt.input)
        if result != tt.expected {
            t.Errorf("cleanForFiglet(%q) = %q, want %q", tt.input, result, tt.expected)
        }
    }
}
```

### Integration Tests

```bash
#!/bin/bash

# Test static endpoint
curl -s localhost:8080/TEST | grep -q "TEST" || echo "Static test failed"

# Test party mode with timeout
timeout 2 curl -s localhost:8080/p/PARTY?t=1 || echo "Party timeout failed"

# Test fonts
curl -s localhost:8080/TEST?f=doom | head -n1
curl -s localhost:8080/TEST?f=3d | head -n1

# Test special characters
curl -s "localhost:8080/HELLO+WORLD"
curl -s "localhost:8080/TEST%20SPACE"

# Test help endpoint
curl -s localhost:8080/help | grep -q "Usage" || echo "Help test failed"
```

## Monitoring

### Metrics to Track
- Request count by endpoint
- Response time percentiles (p50, p95, p99)
- Active streaming connections
- Error rates
- Font usage distribution
- Timeout vs infinite animations
- Geographic distribution (via X-Forwarded-For)

### Health Check Endpoint (Admin Port)

```go
func healthHandler(w http.ResponseWriter, r *http.Request) {
    status := struct {
        Status      string    `json:"status"`
        Timestamp   time.Time `json:"timestamp"`
        FontsLoaded int       `json:"fonts_loaded"`
        Uptime      string    `json:"uptime"`
        Version     string    `json:"version"`
    }{
        Status:      "healthy",
        Timestamp:   time.Now(),
        FontsLoaded: len(fontCache),
        Uptime:      time.Since(startTime).String(),
        Version:     VERSION,
    }
    
    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(status)
}

func metricsHandler(w http.ResponseWriter, r *http.Request) {
    // Prometheus-compatible metrics
    w.Header().Set("Content-Type", "text/plain")
    fmt.Fprintf(w, `# HELP shout_requests_total Total number of requests
# TYPE shout_requests_total counter
shout_requests_total{endpoint="static"} %d
shout_requests_total{endpoint="party"} %d
shout_requests_total{endpoint="help"} %d

# HELP shout_active_streams Current number of active party mode streams
# TYPE shout_active_streams gauge
shout_active_streams %d

# HELP shout_response_time_seconds Response time in seconds
# TYPE shout_response_time_seconds histogram
shout_response_time_seconds_bucket{le="0.1"} %d
shout_response_time_seconds_bucket{le="0.5"} %d
shout_response_time_seconds_bucket{le="1.0"} %d
`, metrics.StaticRequests, metrics.PartyRequests, metrics.HelpRequests,
   metrics.ActiveStreams, metrics.Fast, metrics.Medium, metrics.Slow)
}
```

## Performance Optimization

### HTTP Protocol Considerations
- **HTTP/1.1 only** - Better compatibility with CLI tools
- Streaming works more reliably with HTTP/1.1
- curl, wget, and other tools have varying HTTP/2 support
- Chunked transfer encoding is well-supported in HTTP/1.1

### Caching Strategy
- Cache generated ASCII art for common requests
- Use sync.Map for thread-safe caching
- TTL: 1 hour for static content
- Key: hash of (text + font + width)

### Connection Management
- Use connection pooling
- Implement graceful shutdown
- Set appropriate timeouts
- Handle slow clients

### Resource Limits
- Max text length: 100 characters
- Max connection time: 5 minutes
- Max concurrent connections: 100 (configurable)
- Memory limit: 512MB

## Performance Benchmarks

### Expected Performance Metrics

| Metric | Target | Notes |
|--------|--------|-------|
| Static requests/sec | 1000+ | Single instance, cached responses |
| Concurrent streams | 100+ | Limited by connection manager |
| Memory usage (base) | ~50MB | Without active connections |
| Memory per stream | ~1MB | Including buffers |
| CPU usage | <10% | For 100 concurrent streams |
| P50 latency | <50ms | Static requests |
| P95 latency | <200ms | Static requests |
| P99 latency | <500ms | Static requests |

### Load Testing

```bash
# Install vegeta
go install github.com/tsenart/vegeta@latest

# Test static endpoint
echo "GET http://localhost:8080/HELLO+WORLD" | \
  vegeta attack -duration=30s -rate=100 | \
  vegeta report

# Test with different fonts
echo "GET http://localhost:8080/TEST?f=doom
GET http://localhost:8080/TEST?f=3d
GET http://localhost:8080/TEST?f=big" | \
  vegeta attack -duration=30s -rate=50 | \
  vegeta report

# Test party mode (concurrent streams)
for i in {1..10}; do
  timeout 5 curl -s "localhost:8080/p/STREAM$i?t=5" &
done
wait
```

### Optimization Techniques

1. **ASCII Art Caching**
```go
type AsciiCache struct {
    cache *sync.Map
    hits  int64
    miss  int64
}

func (ac *AsciiCache) Get(key string) (string, bool) {
    if val, ok := ac.cache.Load(key); ok {
        atomic.AddInt64(&ac.hits, 1)
        return val.(string), true
    }
    atomic.AddInt64(&ac.miss, 1)
    return "", false
}
```

2. **Connection Pooling**
```go
var bufferPool = sync.Pool{
    New: func() interface{} {
        return bufio.NewWriterSize(nil, 4096)
    },
}
```

3. **Profiling Support**
```go
// Enable profiling in debug mode
if config.Logging.Level == "debug" {
    admin.Get("/debug/pprof/*", adaptor.HTTPHandler(http.DefaultServeMux))
}
```

### Memory Profiling

```bash
# Run with profiling enabled
SHOUT_LOG_LEVEL=debug ./shout

# Capture heap profile
curl http://localhost:9090/debug/pprof/heap > heap.prof

# Analyze
go tool pprof heap.prof
```

## Security Considerations

### Input Validation
- Strip all non-ASCII characters
- Limit text length
- Validate font names against whitelist
- Sanitize query parameters

### Rate Limiting
- 100 requests/minute per IP
- Implement exponential backoff
- Use Redis for distributed rate limiting (if scaled)

### Headers
```go
func securityHeaders(next http.Handler) http.Handler {
    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        w.Header().Set("X-Content-Type-Options", "nosniff")
        w.Header().Set("X-Frame-Options", "DENY")
        w.Header().Set("Content-Security-Policy", "default-src 'none'")
        next.ServeHTTP(w, r)
    })
}
```

## Future Enhancements

### Phase 2 Features
- [ ] Multi-line text support (e.g., `/party/line1/line2`)
- [ ] Enhanced browser help page with live examples
- [ ] Additional color schemes (ocean, sunset, neon)
- [ ] Border decorations (box, stars, lines)
- [ ] Width control with text wrapping
- [ ] Alignment options (center, right)
- [ ] Font preview endpoint
- [ ] Interactive font selector on web interface

### Phase 3 Features
- [ ] Custom color patterns via hex codes
- [ ] ASCII art logos/images
- [ ] Webhook integration for CI/CD
- [ ] Statistics dashboard
- [ ] API key for higher rate limits

### Potential Integrations
- GitHub Actions
- GitLab CI
- Jenkins plugins
- VS Code extension
- Terminal multiplexer themes

## Launch Checklist

- [ ] Domain configured (shout.sh)
- [ ] SSL certificate (Let's Encrypt)
- [ ] Fonts loaded and tested
- [ ] Rate limiting active
- [ ] Monitoring configured
- [ ] Backup strategy
- [ ] Documentation site
- [ ] Example collection
- [ ] Social media prepared
- [ ] Load testing completed

## Example Usage Collection

```bash
# CI/CD Pipeline
curl shout.sh/p/BUILD+PASSING?t=3&c=green

# Error Messages
curl shout.sh/ERROR+404?f=bloody&c=red

# Welcome Banners
curl shout.sh/WELCOME?f=3d > banner.txt

# Terminal MOTD
echo "$(curl -s shout.sh/$(hostname)?f=small)"

# Git Hooks
curl -s shout.sh/COMMIT+SUCCESSFUL?t=2

# Docker Builds
docker build . && curl shout.sh/p/SHIPPED?t=5

# Meeting Reminders
curl shout.sh/p/STANDUP+TIME?t=10&s=slow
```

## Support Documentation

### Common Issues

**Q: No colors showing?**
A: Ensure your terminal supports ANSI colors. Try `echo -e "\033[31mRED\033[0m"`

**Q: Animation not working?**
A: Check if your curl version supports streaming. Update curl if needed.

**Q: Connection closes immediately?**
A: You might be hitting rate limits. Wait 60 seconds and try again.

**Q: Can I use this in production?**
A: Yes! Use timeout parameter and error handling in scripts.

## License & Attribution

- Service: MIT License
- FIGlet fonts: Various open source licenses
- Inspired by: parrot.live, wttr.in, and the FIGlet project

---

*Last updated: 2025*