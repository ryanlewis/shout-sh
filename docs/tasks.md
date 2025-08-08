# shout.sh Implementation Tasks

## Overview
This document breaks down the shout.sh PRD into actionable implementation tasks. Tasks are identified with unique codes (SHO-XXX) and organized by implementation phases.

---

## PHASE-01: Foundation

### SHO-001: Project Setup and Initialize Go Module

**Description:**  
Set up the initial Go project structure, initialize the Go module, ensure the development environment is ready with the correct Go version, and install required development tools.

**Sample Code:**
```bash
# Initialize Go module
go mod init github.com/ryanlewis/shout-sh

# Install development tools
go install golang.org/x/tools/cmd/goimports@latest
go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
```

**Dependencies:** None

**Acceptance Criteria:**
- [x] .tool-versions file exists with `golang 1.24.6`
- [x] go.mod file created with module name
- [x] Go 1.24 is installed via asdf or system package manager
- [x] goimports installed for code formatting and import management
- [x] golangci-lint installed for comprehensive linting
- [x] Project compiles with a basic main.go file

**Implementation Notes:**
- Used TDD approach: wrote tests first in `main_test.go`
- Initialized Go module with `go mod init github.com/ryanlewis/shout-sh`
- Created minimal `main.go` that compiles and runs
- Installed dev tools with `go install` to GOBIN
- Required `asdf reshim golang` after tool installation to make them available in PATH
- All tests pass with 100% success rate
- Code formatted with goimports and passes golangci-lint checks

---

### SHO-002: Create Project Directory Structure

**Description:**  
Create all necessary directories for the project following the structure defined in the PRD.

**Sample Code:**
```
shout-sh/
├── handlers/
├── render/
├── fonts/
├── middleware/
├── config/
├── types/
├── constants/
└── cmd/
```

**Dependencies:** SHO-001

**Acceptance Criteria:**
- [x] All directories created as per PRD structure
- [x] README.md created with basic project information
- [x] .gitignore file configured for Go projects
- [x] License file added (MIT)

**Implementation Notes:**
- Created all required directories: handlers/, render/, fonts/, middleware/, config/, types/, constants/, cmd/
- README.md includes project overview and getting started instructions
- .gitignore configured with standard Go patterns
- LICENSE file added with MIT license

---

### SHO-003: Add Project Dependencies

**Description:**  
Update go.mod with all required dependencies including godotenv and caarlos0/env for configuration management.

**Dependencies:** SHO-001

**Acceptance Criteria:**
- [x] Fiber v2 added to go.mod
- [x] go-figure library added (alternative to figlet4go)
- [x] godotenv library added for .env file loading
- [x] caarlos0/env library added for env var parsing
- [x] All dependencies downloaded with `go mod download`
- [x] go.sum file generated

**Implementation Notes:**
- Used go-figure instead of figlet4go for ASCII art generation
- All dependencies successfully installed and verified
- go.sum file generated with dependency checksums

---

### SHO-004: Define Type Structures

**Description:**  
Create the type definitions file with all core structures including RenderOptions, ConnectionManager, Config, and Metrics.

**Sample Code:**
```go
type RenderOptions struct {
    Font     string `json:"font" query:"f,font"`
    Color    string `json:"color" query:"c,color"`
    MaxWidth int    `json:"maxwidth" query:"mw,maxwidth"`
    Timeout  int    `json:"timeout" query:"t,timeout"`
    Speed    int    `json:"speed" query:"s,speed"`
    Align    string `json:"align" query:"a,align"`
    Border   string `json:"border" query:"b,border"`
}
```

**Dependencies:** SHO-002, SHO-003

**Acceptance Criteria:**
- [x] types/types.go file created with all type definitions
- [x] ConnectionManager struct with methods implemented
- [x] Config struct with nested configuration types
- [x] All types have proper JSON and YAML tags

**Implementation Notes:**
- Used TDD approach: wrote comprehensive tests first in `types/types_test.go`
- Created all required type structures with proper documentation
- ConnectionManager implemented with thread-safe atomic operations
- Config struct includes nested types for logical grouping (ServerConfig, RateLimitConfig, FontConfig, StreamingConfig, TextConfig)
- All types include GoDoc comments with usage examples
- Achieved 100% test coverage for the types package
- All code formatted with goimports and passes golangci-lint checks

---

### SHO-005: Configuration Structure Definition ✅

**Description:**  
Define configuration struct with all application settings using caarlos0/env tags for default values. This approach keeps defaults in code as struct tags rather than separate files.

**Dependencies:** SHO-003, SHO-004

**Acceptance Criteria:**
- [x] Config struct defined with all settings
- [x] Default values specified as env tags
- [x] Nested structs for logical grouping (Server, RateLimit, Fonts, etc.)
- [x] All constants defined as struct fields with defaults
- [x] Version field included in config

**Implementation Notes:**
- Used TDD approach with comprehensive test coverage (98.3%)
- Implemented singleton pattern with thread-safe loading
- Created nested config structs: ServerConfig, RateLimitConfig, FontConfig, StreamingConfig, TextConfig
- Added validation for all configuration values
- Included helper functions: Get(), MustLoad(), LoadFromEnv(), Reset()
- Properly handles .env file loading with godotenv
- All struct fields use env tags with SHOUT_ prefix
- Test suite includes validation tests, env override tests, and .env file loading tests
- Fixed singleton reset issues in tests by properly unsetting environment variables
- Improved test coverage with additional tests for Get(), Validate(), and error handling

---

### SHO-006: Configuration Loading with godotenv and caarlos0/env

**Description:**  
Implement configuration loading using godotenv to load .env file (if exists) and caarlos0/env to parse environment variables into the config struct. Defaults are defined as struct tags, making the configuration self-documenting and eliminating the need for separate config files.

**Dependencies:** SHO-005

**Acceptance Criteria:**
- [ ] config/config.go with Load function
- [ ] Use godotenv to load .env file if present
- [ ] Use caarlos0/env to parse env vars into struct
- [ ] All env vars prefixed with SHOUT_
- [ ] Configuration validation after loading
- [ ] Panic/exit if critical config values are invalid
- [ ] Helper functions for accessing config singleton
- [ ] Unit tests for configuration loading

---

## PHASE-02: Core Functionality

### SHO-007: Font File Acquisition

**Description:**  
Download required FIGlet font files from official sources and validate them.

**Sample Code:**
```bash
wget https://raw.githubusercontent.com/cmatsuoka/figlet/master/fonts/doom.flf -O fonts/doom.flf
wget https://raw.githubusercontent.com/cmatsuoka/figlet/master/fonts/3d.flf -O fonts/3d.flf
```

**Dependencies:** SHO-002

**Acceptance Criteria:**
- [ ] All 8 required fonts downloaded (doom, 3d, big, bloody, standard, slant, small, shadow)
- [ ] Font files validated with proper FIGlet headers
- [ ] Font licenses documented
- [ ] Script created for font downloading

---

### SHO-008: Font Loading and Caching System

**Description:**  
Implement the font loading system with caching for efficient ASCII art generation.

**Sample Code:**
```go
func loadFonts() error {
    fontCache = &FontCache{
        fonts: make(map[string]*figlet4go.AsciiRender),
    }
    
    fonts := config.Fonts.Fonts
    for _, name := range fonts {
        renderer := figlet4go.NewAsciiRender()
        fontPath := fmt.Sprintf("%s/%s.flf", config.Fonts.Directory, name)
        
        if err := renderer.LoadFont(fontPath); err != nil {
            log.Printf("Warning: Could not load font %s: %v", name, err)
            continue
        }
        
        fontCache.fonts[name] = renderer
    }
    
    return nil
}
```

**Dependencies:** SHO-007, SHO-004

**Acceptance Criteria:**
- [ ] render/fonts.go with font loading logic
- [ ] FontCache struct with thread-safe access
- [ ] Font validation function implemented
- [ ] Error handling for missing fonts
- [ ] At least one font successfully loads

---

### SHO-009: ASCII Art Generation Core

**Description:**  
Implement the core ASCII art generation function using the figlet4go library.

**Sample Code:**
```go
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
    
    return ascii, nil
}
```

**Dependencies:** SHO-008

**Acceptance Criteria:**
- [ ] render/figlet.go with generateASCII function
- [ ] Font fallback to default if requested font not found
- [ ] Error handling for rendering failures
- [ ] Support for text alignment
- [ ] Support for border styles

---

### SHO-010: Color Schemes Implementation

**Description:**  
Implement all color schemes for both static and animated output.

**Sample Code:**
```go
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
    default:
        return 7 // Default white
    }
}
```

**Dependencies:** SHO-004

**Acceptance Criteria:**
- [ ] render/colors.go with color scheme logic
- [ ] Rainbow, fire, ocean, and matrix schemes implemented
- [ ] Static color application function
- [ ] ANSI color code generation
- [ ] Unit tests for color calculations

---

### SHO-011: Input Sanitization

**Description:**  
Implement input cleaning functions to ensure safe text processing.

**Sample Code:**
```go
func cleanForFiglet(text string) string {
    text = strings.Map(func(r rune) rune {
        if r >= 32 && r <= 126 {
            return r
        }
        if r == '\n' || r == '\t' {
            return r
        }
        return -1
    }, text)
    
    if len(text) > MaxTextLength {
        text = text[:MaxTextLength]
    }
    
    return text
}
```

**Dependencies:** SHO-005

**Acceptance Criteria:**
- [ ] render/sanitize.go with cleaning functions
- [ ] Non-ASCII character removal
- [ ] Text length limiting
- [ ] URL decoding support
- [ ] Unit tests for edge cases

---

### SHO-012: Helper Functions

**Description:**  
Implement all utility helper functions used throughout the application.

**Sample Code:**
```go
func firstOf(values ...string) string {
    for _, v := range values {
        if v != "" {
            return v
        }
    }
    return ""
}

func calculateFrameDelay(speed int) time.Duration {
    if speed < MinSpeed {
        speed = MinSpeed
    }
    if speed > MaxSpeed {
        speed = MaxSpeed
    }
    delay := 220 - (speed * 20)
    return time.Duration(delay) * time.Millisecond
}
```

**Dependencies:** SHO-005

**Acceptance Criteria:**
- [ ] utils/helpers.go with all helper functions
- [ ] firstOf function for parameter precedence
- [ ] calculateFrameDelay for animation timing
- [ ] validateOptions for input validation
- [ ] Unit tests for all helpers

---

### SHO-013: Text Formatting Features

**Description:**  
Implement text alignment and border decoration features.

**Sample Code:**
```go
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
```

**Dependencies:** SHO-009

**Acceptance Criteria:**
- [ ] Text alignment (left, center, right) working
- [ ] Border styles (box, double) implemented
- [ ] Width control with proper wrapping
- [ ] Padding calculation correct
- [ ] Unit tests for formatting

---

## PHASE-03: HTTP Handlers

### SHO-014: Main Server Setup

**Description:**  
Set up the main Fiber application with basic configuration and middleware.

**Sample Code:**
```go
func main() {
    config, err := LoadConfig()
    if err != nil {
        log.Fatalf("Failed to load config: %v", err)
    }
    
    app := fiber.New(fiber.Config{
        ServerHeader:      "shout.sh",
        AppName:          "shout.sh",
        StreamRequestBody: true,
        DisableHTTP2:     true,
        ReadTimeout:      config.Server.ReadTimeout,
        IdleTimeout:      config.Server.IdleTimeout,
        ErrorHandler:     customErrorHandler,
    })
    
    // Middleware
    app.Use(recover.New())
    app.Use(cors.New())
    app.Use(logger.New())
    
    addr := fmt.Sprintf(":%d", config.Server.PublicPort)
    log.Fatal(app.Listen(addr))
}
```

**Dependencies:** SHO-006

**Acceptance Criteria:**
- [ ] main.go with server initialization
- [ ] Fiber app configured with proper settings
- [ ] HTTP/2 disabled for CLI compatibility
- [ ] Server starts on configured port
- [ ] Basic health check endpoint working

---

### SHO-015: Static Banner Handler

**Description:**  
Implement the handler for generating static ASCII art banners.

**Sample Code:**
```go
func staticHandler(c *fiber.Ctx) error {
    atomic.AddInt64(&metrics.StaticRequests, 1)
    
    text := c.Params("*")
    if text == "" {
        return c.Status(400).SendString("Error: No text provided")
    }
    
    opts := parseOptions(c)
    
    text = strings.ReplaceAll(text, "+", " ")
    text = cleanForFiglet(text)
    
    output, err := generateASCII(text, opts)
    if err != nil {
        return c.Status(500).SendString("Error generating ASCII art")
    }
    
    if opts.Color != "" && opts.Color != "none" {
        output, _ = applyStaticColor(output, opts.Color)
    }
    
    c.Set("Content-Type", "text/plain; charset=utf-8")
    return c.SendString(output)
}
```

**Dependencies:** SHO-009, SHO-010, SHO-011

**Acceptance Criteria:**
- [ ] handlers/static.go with staticHandler function
- [ ] Query parameter parsing working
- [ ] Text cleaning and validation
- [ ] Color application for static output
- [ ] Error responses with appropriate status codes

---

### SHO-016: Party Mode Streaming Handler

**Description:**  
Implement the animated streaming handler for party mode with rainbow effects.

**Sample Code:**
```go
func partyHandler(c *fiber.Ctx) error {
    atomic.AddInt64(&metrics.PartyRequests, 1)
    
    if !connManager.TryAcquire() {
        return c.Status(503).SendString("Too many active streams")
    }
    defer connManager.Release()
    
    text := c.Params("*")
    text = strings.ReplaceAll(text, "+", " ")
    text = cleanForFiglet(text)
    
    opts := parseOptions(c)
    
    if opts.Timeout == 0 || opts.Timeout > int(config.Server.MaxStreamDuration.Seconds()) {
        opts.Timeout = int(DefaultStreamTimeout.Seconds())
    }
    
    return streamAnimated(c, text, opts)
}
```

**Dependencies:** SHO-015, SHO-022

**Acceptance Criteria:**
- [ ] handlers/party.go with streaming logic
- [ ] Animation frames rendering correctly
- [ ] Timeout handling working
- [ ] Connection cleanup on disconnect
- [ ] ANSI escape sequences for animation

---

### SHO-017: Root Handler with Client Detection

**Description:**  
Implement the root handler that detects browser vs CLI clients and serves appropriate content.

**Sample Code:**
```go
func rootHandler(c *fiber.Ctx) error {
    if c.Path() != "/" {
        return staticHandler(c)
    }
    
    if isBrowser(c) {
        return serveLandingPage(c)
    }
    return serveCliHelp(c)
}

func isBrowser(c *fiber.Ctx) bool {
    accept := c.Get("Accept")
    ua := string(c.Context().UserAgent())
    
    hasHtmlAccept := strings.Contains(accept, "text/html")
    hasBrowserUA := strings.Contains(strings.ToLower(ua), "mozilla") || 
                    strings.Contains(strings.ToLower(ua), "chrome")
    
    return hasHtmlAccept && hasBrowserUA
}
```

**Dependencies:** Task 15

**Acceptance Criteria:**
- [ ] handlers/root.go with client detection
- [ ] Browser detection based on headers
- [ ] CLI help text defined
- [ ] HTML landing page template
- [ ] Proper content-type headers

---

### SHO-018: Help Endpoints

**Description:**  
Implement help and documentation endpoints for both CLI and browser clients.

**Sample Code:**
```go
func helpHandler(c *fiber.Ctx) error {
    if isBrowser(c) {
        return serveHelpPage(c)
    }
    return serveDetailedCliHelp(c)
}

const cliHelpText = `
shout.sh - ASCII art generator

Usage:
  curl shout.sh/YOUR+TEXT
  curl shout.sh/p/PARTY+MODE

Options:
  f=FONT    Font style (doom, 3d, big, etc.)
  c=COLOR   Color scheme (rainbow, fire, ocean)
  t=TIME    Timeout for animations (seconds)
  s=SPEED   Animation speed (1-10)
`
```

**Dependencies:** SHO-017

**Acceptance Criteria:**
- [ ] handlers/help.go with help endpoints
- [ ] Detailed CLI help text
- [ ] HTML help page for browsers
- [ ] Examples included in help
- [ ] Parameter documentation

---

### SHO-019: Font Listing Endpoint

**Description:**  
Implement endpoint to list available fonts and provide previews.

**Sample Code:**
```go
func fontsHandler(c *fiber.Ctx) error {
    atomic.AddInt64(&metrics.FontRequests, 1)
    
    var fontList []string
    for name := range fontCache.fonts {
        fontList = append(fontList, name)
    }
    sort.Strings(fontList)
    
    if c.Query("preview") != "" {
        return fontPreviewHandler(c, fontList)
    }
    
    c.Set("Content-Type", "text/plain")
    return c.SendString(strings.Join(fontList, "\n"))
}
```

**Dependencies:** SHO-008

**Acceptance Criteria:**
- [ ] handlers/fonts.go with font listing
- [ ] List all available fonts
- [ ] Optional preview parameter
- [ ] Font samples with "SHOUT"
- [ ] Sorted alphabetically

---

### SHO-020: Admin Endpoints

**Description:**  
Implement admin endpoints for health checks and metrics on the admin port.

**Sample Code:**
```go
func healthHandler(c *fiber.Ctx) error {
    status := fiber.Map{
        "status":      "healthy",
        "timestamp":   time.Now(),
        "fonts_loaded": len(fontCache.fonts),
        "uptime":      time.Since(startTime).String(),
        "version":     Version,
    }
    
    return c.JSON(status)
}

func metricsHandler(c *fiber.Ctx) error {
    c.Set("Content-Type", "text/plain")
    
    metrics := fmt.Sprintf(`# HELP shout_requests_total Total requests
# TYPE shout_requests_total counter
shout_requests_total{endpoint="static"} %d
shout_requests_total{endpoint="party"} %d
shout_active_streams %d
`, atomic.LoadInt64(&metrics.StaticRequests), 
   atomic.LoadInt64(&metrics.PartyRequests),
   connManager.GetActiveCount())
    
    return c.SendString(metrics)
}
```

**Dependencies:** SHO-014

**Acceptance Criteria:**
- [ ] handlers/admin.go with admin endpoints
- [ ] Health check returns JSON status
- [ ] Prometheus-compatible metrics
- [ ] Separate admin server on port 9090
- [ ] Uptime and version tracking

---

## PHASE-04: Middleware & Management

### SHO-021: Rate Limiting Middleware

**Description:**  
Implement rate limiting to prevent abuse and ensure fair usage.

**Sample Code:**
```go
app.Use(limiter.New(limiter.Config{
    Max:        config.RateLimit.RequestsPerMinute,
    Expiration: 1 * time.Minute,
    KeyGenerator: func(c *fiber.Ctx) string {
        return c.IP()
    },
    LimitReached: func(c *fiber.Ctx) error {
        atomic.AddInt64(&metrics.RejectedStreams, 1)
        return c.Status(429).SendString("Rate limit exceeded")
    },
}))
```

**Dependencies:** SHO-014

**Acceptance Criteria:**
- [ ] middleware/ratelimit.go implemented
- [ ] Per-IP rate limiting
- [ ] Configurable limits
- [ ] 429 status on limit exceeded
- [ ] Metrics for rejected requests

---

### SHO-022: Connection Manager

**Description:**  
Implement connection manager to limit concurrent streaming connections.

**Sample Code:**
```go
func NewConnectionManager(maxStreams int64) *ConnectionManager {
    return &ConnectionManager{
        maxStreams: maxStreams,
    }
}

func (cm *ConnectionManager) TryAcquire() bool {
    if atomic.LoadInt64(&cm.activeStreams) >= cm.maxStreams {
        return false
    }
    atomic.AddInt64(&cm.activeStreams, 1)
    return true
}

func (cm *ConnectionManager) Release() {
    atomic.AddInt64(&cm.activeStreams, -1)
}
```

**Dependencies:** SHO-004

**Acceptance Criteria:**
- [ ] Connection manager with atomic operations
- [ ] Thread-safe acquire/release
- [ ] Maximum connection enforcement
- [ ] Active count tracking
- [ ] Integration with party handler

---

### SHO-023: CORS Configuration

**Description:**  
Configure CORS middleware to support browser-based access.

**Sample Code:**
```go
app.Use(cors.New(cors.Config{
    AllowOrigins: "*",
    AllowMethods: "GET",
    AllowHeaders: "Origin, Content-Type, Accept",
}))
```

**Dependencies:** SHO-014

**Acceptance Criteria:**
- [ ] CORS headers properly set
- [ ] All origins allowed for public API
- [ ] Only GET method allowed
- [ ] Browser requests working
- [ ] Preflight requests handled

---

### SHO-024: Logging System

**Description:**  
Implement structured logging for debugging and monitoring.

**Sample Code:**
```go
app.Use(logger.New(logger.Config{
    Format:     "${time} ${ip} ${method} ${path} ${status} ${latency}\n",
    TimeFormat: "2006-01-02 15:04:05",
    Output:     os.Stdout,
}))
```

**Dependencies:** SHO-014

**Acceptance Criteria:**
- [ ] middleware/logging.go with logger setup
- [ ] Request/response logging
- [ ] Error logging with stack traces
- [ ] Configurable log levels
- [ ] Structured log format

---

### SHO-025: Error Handling

**Description:**  
Implement custom error handler for consistent error responses.

**Sample Code:**
```go
func customErrorHandler(c *fiber.Ctx, err error) error {
    code := fiber.StatusInternalServerError
    message := "Internal Server Error"
    
    if e, ok := err.(*fiber.Error); ok {
        code = e.Code
        message = e.Message
    }
    
    if code >= 500 {
        log.Printf("Error %d: %v", code, err)
        atomic.AddInt64(&metrics.TotalErrors, 1)
    }
    
    if strings.Contains(c.Get("Accept"), "application/json") {
        return c.Status(code).JSON(fiber.Map{
            "error": message,
            "code":  code,
        })
    }
    
    return c.Status(code).SendString(fmt.Sprintf("Error %d: %s", code, message))
}
```

**Dependencies:** SHO-014

**Acceptance Criteria:**
- [ ] Custom error handler implemented
- [ ] JSON responses for API clients
- [ ] Plain text for CLI clients
- [ ] Error metrics tracking
- [ ] Proper status codes

---

## PHASE-05: Deployment & Testing

### SHO-026: Dockerfile Creation

**Description:**  
Create multi-stage Dockerfile for efficient container builds.

**Sample Code:**
```dockerfile
FROM golang:1.23 AS builder

WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download

COPY . .

ARG TARGETOS=linux
ARG TARGETARCH
RUN CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
    go build -ldflags="-w -s" -o shout .

FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /app/shout /shout
COPY --from=builder /app/fonts /fonts

USER nonroot

EXPOSE 8080 9090
ENTRYPOINT ["/shout"]
```

**Dependencies:** All code tasks

**Acceptance Criteria:**
- [ ] Multi-stage build for small image
- [ ] Non-root user execution
- [ ] Fonts included in image
- [ ] Both ports exposed
- [ ] Build args for architecture

---

### SHO-027: Docker Compose Setup

**Description:**  
Create docker-compose.yml for local development and testing.

**Sample Code:**
```yaml
version: '3.8'

services:
  shout:
    build: .
    ports:
      - "80:8080"
      - "9090:9090"
    restart: unless-stopped
    environment:
      - SHOUT_LOG_LEVEL=debug
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:9090/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

**Dependencies:** SHO-026

**Acceptance Criteria:**
- [ ] docker-compose.yml created
- [ ] Service configuration complete
- [ ] Health check configured
- [ ] Port mapping correct
- [ ] Environment variables set

---

### SHO-028: Unit Tests

**Description:**  
Write unit tests for core functionality.

**Sample Code:**
```go
func TestCleanForFiglet(t *testing.T) {
    tests := []struct {
        input    string
        expected string
    }{
        {"HELLO", "HELLO"},
        {"Hello 🔥 World", "Hello  World"},
        {strings.Repeat("A", 200), strings.Repeat("A", 100)},
    }
    
    for _, tt := range tests {
        result := cleanForFiglet(tt.input)
        if result != tt.expected {
            t.Errorf("cleanForFiglet(%q) = %q, want %q", 
                    tt.input, result, tt.expected)
        }
    }
}
```

**Dependencies:** Core functionality tasks

**Acceptance Criteria:**
- [ ] Test files for each package
- [ ] Core functions have tests
- [ ] Edge cases covered
- [ ] Minimum 70% code coverage
- [ ] Tests pass with `go test ./...`

---

### SHO-029: Integration Tests

**Description:**  
Create integration tests for API endpoints.

**Sample Code:**
```bash
#!/bin/bash

# Test static endpoint
curl -s localhost:8080/TEST | grep -q "TEST" || echo "Static test failed"

# Test party mode with timeout
timeout 2 curl -s localhost:8080/p/PARTY?t=1 || echo "Party timeout failed"

# Test fonts
curl -s localhost:8080/fonts | grep -q "doom" || echo "Fonts test failed"

# Test help
curl -s localhost:8080/help | grep -q "Usage" || echo "Help test failed"
```

**Dependencies:** All handler tasks

**Acceptance Criteria:**
- [ ] Integration test script created
- [ ] All endpoints tested
- [ ] Error cases tested
- [ ] Performance baseline established
- [ ] CI/CD ready

---

### SHO-030: Performance Testing Setup

**Description:**  
Set up performance testing using vegeta or similar tools.

**Sample Code:**
```bash
# Load test script
echo "GET http://localhost:8080/HELLO+WORLD" | \
  vegeta attack -duration=30s -rate=100 | \
  vegeta report

# Concurrent streams test
for i in {1..10}; do
  timeout 5 curl -s "localhost:8080/p/STREAM$i?t=5" &
done
wait
```

**Dependencies:** SHO-029

**Acceptance Criteria:**
- [ ] Load testing scripts created
- [ ] Performance metrics documented
- [ ] 1000+ RPS for static endpoints achieved
- [ ] 100+ concurrent streams supported
- [ ] Memory usage under 512MB

---

## Task Priority and Parallelization

### Sequential Dependencies:
1. Foundation tasks (SHO-001 to SHO-006) must be completed first
2. Core functionality (SHO-007 to SHO-013) depends on foundation
3. Handlers (SHO-014 to SHO-020) depend on core functionality
4. Middleware (SHO-021 to SHO-025) can be done in parallel with handlers
5. Testing and deployment (SHO-026 to SHO-030) done last

### Parallel Work Opportunities:
- Tasks SHO-007 to SHO-013 can be worked on by different developers
- Tasks SHO-015 to SHO-020 can be developed in parallel once SHO-014 is done
- Tasks SHO-021 to SHO-025 can be developed independently
- Tasks SHO-028 to SHO-030 can be done in parallel

## PHASE-06: Release Automation

### SHO-031: Conventional Commits Setup

**Description:**  
Implement conventional commit message format and create contributing guidelines for standardized commit messages that will be used for automated changelog generation.

**Sample Code:**
```markdown
# Commit Message Format
type(scope): description

Types:
- feat: New feature
- fix: Bug fix  
- feat!, fix!: Breaking change
- chore: Maintenance (excluded from changelog)
- docs: Documentation (excluded from changelog)
- build: Build system changes
- ci: CI configuration changes
- test: Adding/fixing tests
- refactor: Code refactoring
```

**Dependencies:** None

**Acceptance Criteria:**
- [ ] CONTRIBUTING.md updated with commit conventions
- [ ] .gitmessage template file created for commit format
- [ ] Team documentation on commit standards
- [ ] Examples of each commit type provided
- [ ] Git hooks configured for commit message validation (optional)

---

### SHO-032: GoReleaser Configuration

**Description:**  
Create and configure GoReleaser for automated binary building, changelog generation, and GitHub release creation.

**Sample Code:**
```yaml
version: 2
project_name: shout-sh

builds:
  - env:
      - CGO_ENABLED=0
    goos:
      - linux
    goarch:
      - amd64
      - arm64
    main: ./main.go
    binary: shout-sh

archives:
  - format: tar.gz
    name_template: >-
      {{ .ProjectName }}_
      {{- .Version }}_
      {{- .Os }}_
      {{- .Arch }}

changelog:
  sort: asc
  use: github
  filters:
    exclude:
      - '^chore'
      - '^docs'
  groups:
    - title: '🚀 Features'
      regexp: '^feat'
    - title: '🐛 Bug Fixes'
      regexp: '^fix'
    - title: '⚠️ Breaking Changes'
      regexp: '^.*!:'
```

**Dependencies:** SHO-001

**Acceptance Criteria:**
- [ ] .goreleaser.yml file created in project root
- [ ] Binary build configuration for Linux amd64/arm64
- [ ] Archive format configured as tar.gz
- [ ] Changelog generation configured with commit groups
- [ ] GitHub release configuration added
- [ ] Local testing with `goreleaser check` passes

---

### SHO-033: Ko Configuration for Container Images

**Description:**  
Configure ko for building and pushing multi-architecture Docker images to GitHub Container Registry.

**Sample Code:**
```yaml
# .goreleaser.yml addition
kos:
  - repository: ghcr.io/ryanlewis/shout-sh
    tags:
      - '{{.Version}}'
      - latest
    bare: true
    preserve_import_paths: false
    platforms:
      - linux/amd64
      - linux/arm64
    main: ./

# .ko.yaml
defaultBaseImage: cgr.dev/chainguard/static:latest
defaultPlatforms:
  - linux/amd64
  - linux/arm64
```

**Dependencies:** SHO-032

**Acceptance Criteria:**
- [ ] Ko configuration added to .goreleaser.yml
- [ ] .ko.yaml created with base image settings
- [ ] Multi-arch platforms configured (amd64/arm64)
- [ ] Repository URL points to ghcr.io
- [ ] Version and latest tags configured
- [ ] Local ko build test successful

---

### SHO-034: GitHub Actions Release Workflow

**Description:**  
Create GitHub Actions workflow that triggers on version tags and automates the entire release process.

**Sample Code:**
```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write
  packages: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      
      - uses: actions/setup-go@v5
        with:
          go-version: stable
      
      - uses: ko-build/setup-ko@v0.6
      
      - name: Login to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Run GoReleaser
        uses: goreleaser/goreleaser-action@v6
        with:
          version: latest
          args: release --clean
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

**Dependencies:** SHO-032, SHO-033

**Acceptance Criteria:**
- [ ] .github/workflows/release.yml created
- [ ] Workflow triggers on v* tags only
- [ ] Permissions set for contents and packages
- [ ] Go environment setup included
- [ ] Ko tool setup included
- [ ] GHCR login configured
- [ ] GoReleaser action configured with clean flag

---

### SHO-035: Release Testing Scripts

**Description:**  
Create scripts for testing the release process locally before pushing tags.

**Sample Code:**
```bash
#!/bin/bash
# scripts/test-release.sh

echo "Testing GoReleaser configuration..."
goreleaser check || exit 1

echo "Running snapshot release (no upload)..."
goreleaser release --snapshot --clean --skip=publish

echo "Testing ko build locally..."
ko build . --local --platform=linux/amd64

echo "Checking generated artifacts..."
ls -la dist/

echo "Release test completed successfully!"
```

**Dependencies:** SHO-032, SHO-033

**Acceptance Criteria:**
- [ ] scripts/test-release.sh created
- [ ] GoReleaser dry run test included
- [ ] Ko local build test included
- [ ] Artifact verification steps
- [ ] Script is executable
- [ ] Error handling for failed steps

---

### SHO-036: GitHub Container Registry Setup

**Description:**  
Configure GitHub Container Registry settings and document the setup process for the repository.

**Sample Code:**
```bash
# Setup documentation
## Enable GitHub Container Registry
1. Go to Settings → Package settings → Container registry
2. Enable "Improved container support"

## Test authentication locally
echo $GITHUB_TOKEN | docker login ghcr.io -u ryanlewis --password-stdin

## After first release, configure visibility
1. Go to Packages → shout-sh → Package settings
2. Change visibility to public (optional)
3. Link package to repository
```

**Dependencies:** None

**Acceptance Criteria:**
- [ ] GHCR enabled for repository
- [ ] Package visibility settings documented
- [ ] Authentication test documented
- [ ] README updated with registry information
- [ ] Container pull instructions added

---

### SHO-037: Release Documentation

**Description:**  
Create comprehensive documentation for the release process including quick reference and troubleshooting.

**Sample Code:**
```markdown
# RELEASING.md

## Quick Release Process
1. Ensure all changes committed with conventional format
2. Update version if needed
3. Tag release: `git tag v1.2.3`
4. Push tag: `git push origin v1.2.3`
5. Monitor GitHub Actions for completion
6. Verify release assets on GitHub

## Commit Format Quick Reference
- feat: New feature → Features section
- fix: Bug fix → Bug Fixes section  
- feat!: Breaking change → Breaking Changes
- chore: Maintenance → Hidden
- docs: Documentation → Hidden

## Troubleshooting
### Issue: Release workflow fails
- Check GitHub Actions logs
- Verify GHCR permissions
- Run local test: `./scripts/test-release.sh`

### Issue: Docker push fails
- Verify packages permission in workflow
- Check GHCR is enabled in repo settings
```

**Dependencies:** SHO-034, SHO-035

**Acceptance Criteria:**
- [ ] RELEASING.md created
- [ ] Step-by-step release process documented
- [ ] Commit format reference included
- [ ] Common issues and solutions documented
- [ ] Pre-release checklist included
- [ ] Post-release verification steps listed

---

### SHO-038: Version Management Strategy

**Description:**  
Define and implement semantic versioning strategy with version constants in code.

**Sample Code:**
```go
// version.go
package main

// Version is set during build time
var (
    Version   = "dev"
    GitCommit = "unknown"
    BuildDate = "unknown"
)

// Build with:
// go build -ldflags="-X main.Version=v1.2.3 -X main.GitCommit=$(git rev-parse HEAD) -X main.BuildDate=$(date -u +%Y%m%d)"
```

**Dependencies:** SHO-001

**Acceptance Criteria:**
- [ ] version.go file created with version variables
- [ ] Build flags configured in .goreleaser.yml
- [ ] Version exposed in /health endpoint
- [ ] Version shown in CLI help text
- [ ] Semantic versioning documented (MAJOR.MINOR.PATCH)
- [ ] Version update process documented

---

### SHO-039: Pre-commit Hooks Configuration

**Description:**  
Set up optional pre-commit hooks for enforcing conventional commits and running basic checks before commits.

**Sample Code:**
```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/compilerla/conventional-pre-commit
    rev: v3.0.0
    hooks:
      - id: conventional-pre-commit
        stages: [commit-msg]
        args: []

  - repo: https://github.com/golangci/golangci-lint
    rev: v1.54.2
    hooks:
      - id: golangci-lint

  - repo: local
    hooks:
      - id: go-test
        name: go test
        entry: go test ./...
        language: system
        pass_filenames: false
```

**Dependencies:** SHO-001

**Acceptance Criteria:**
- [ ] .pre-commit-config.yaml created
- [ ] Conventional commit validation hook
- [ ] Go linting hook configured
- [ ] Go test hook configured
- [ ] Installation instructions in CONTRIBUTING.md
- [ ] Hook bypass instructions documented

---

### SHO-040: Release Verification Script

**Description:**  
Create automated script to verify release artifacts after publication.

**Sample Code:**
```bash
#!/bin/bash
# scripts/verify-release.sh

VERSION=${1:-latest}

echo "Verifying release $VERSION..."

# Check GitHub release
gh release view "v$VERSION" || exit 1

# Check Linux binaries
for arch in amd64 arm64; do
    echo "Checking linux_$arch binary..."
    gh release download "v$VERSION" -p "*linux_${arch}.tar.gz" -D /tmp/
    tar -tzf "/tmp/shout-sh_${VERSION}_linux_${arch}.tar.gz" || exit 1
done

# Check Docker images
echo "Checking Docker images..."
docker pull "ghcr.io/ryanlewis/shout-sh:$VERSION" || exit 1
docker run --rm "ghcr.io/ryanlewis/shout-sh:$VERSION" --version

echo "✅ Release $VERSION verified successfully!"
```

**Dependencies:** SHO-034

**Acceptance Criteria:**
- [ ] scripts/verify-release.sh created
- [ ] GitHub release check implemented
- [ ] Binary download and verification
- [ ] Docker image pull test
- [ ] Checksum verification included
- [ ] Success/failure reporting

---

## Updated Success Metrics

- All 40 tasks completed (30 original + 10 release automation)
- All acceptance criteria met
- Tests passing with >70% coverage
- Performance targets achieved
- Documentation complete
- Docker image < 50MB
- Deployment successful
- Automated releases working end-to-end
- Zero manual intervention for releases
- Multi-arch binaries and containers published