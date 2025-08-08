# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

shout.sh is a curl-able ASCII art text generator with animated rainbow effects. It's a Go-based HTTP service that transforms text into bold, attention-grabbing terminal output with both static and animated streaming modes.

## Project Status

Currently in planning phase with comprehensive documentation but no implementation yet. The project has:
- Complete PRD in `docs/prd.md` 
- 40 structured tasks in `docs/tasks.md` organized into 6 phases
- Custom `/implement-task` command for TDD implementation

## Essential Commands

### Development Setup
```bash
# Install Go dependencies
go mod init github.com/ryanlewis/shout-sh
go get github.com/gofiber/fiber/v2
go get github.com/mbndr/figlet4go
go get github.com/joho/godotenv
go get github.com/caarlos0/env/v11

# Install development tools
go install golang.org/x/tools/cmd/goimports@latest
go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
```

### Testing and Quality
```bash
# Run tests with coverage
go test -v -race -cover ./...

# Format code
goimports -w .

# Lint code
golangci-lint run ./...

# Build and run
go build -o shout .
./shout
```

### Task Implementation
Use the custom Claude command to implement tasks following TDD:
```
/implement-task SHO-001
```

## Architecture and Key Design Decisions

### Project Structure
- `main.go` - Server entry point with Fiber app setup
- `handlers/` - HTTP request handlers (static, party, help, fonts, admin)
- `render/` - ASCII art rendering logic (figlet wrapper, colors, animation)
- `config/` - Configuration using env tags (no config files)
- `middleware/` - Rate limiting and logging
- `fonts/` - FIGlet font files (.flf)

### Core Technologies
- **Framework**: Fiber v2 for HTTP handling
- **ASCII Generation**: figlet4go library
- **Configuration**: caarlos0/env with struct tags for defaults
- **Streaming**: HTTP/1.1 chunked transfer encoding for animations

### API Design
```bash
# Static banner
GET /{text}

# Animated party mode  
GET /p/{text} or /party/{text}

# Query parameters
?font=doom          # Font style
?color=rainbow      # Color scheme (rainbow, fire, matrix, ocean, neon)
?timeout=5          # Animation timeout in seconds
?speed=5           # Animation speed (1-10)
?align=center      # Text alignment
?border=double     # Border style
```

### Configuration Philosophy
- **No config files** - all defaults in Go struct tags
- Environment variables with `SHOUT_` prefix override defaults
- Example: `SHOUT_PUBLIC_PORT=8080`, `SHOUT_ADMIN_PORT=9090`

## Implementation Guidelines

### Task System
Tasks are identified with SHO-XXX format across 6 phases:
1. **Foundation** (SHO-001 to SHO-008): Core structure and config
2. **ASCII Rendering** (SHO-009 to SHO-016): Text generation
3. **HTTP Service** (SHO-017 to SHO-024): API endpoints  
4. **Party Mode** (SHO-025 to SHO-029): Animation streaming
5. **Production** (SHO-030 to SHO-036): Deployment readiness
6. **Polish** (SHO-037 to SHO-040): Documentation and extras

### Development Patterns
- **TDD Required**: Write tests first for all new functionality
- **Error Handling**: Return meaningful HTTP status codes
- **Logging**: Structured logging with request IDs
- **Rate Limiting**: 100 requests/minute per IP
- **Graceful Degradation**: Continue if some fonts fail to load

### Performance Targets
- Static requests: 1000+ req/sec
- Concurrent streams: 100+ connections
- Memory usage: ~50MB base
- P50 latency: <50ms for static requests

## Important Notes

### Streaming Implementation
- Use HTTP/1.1 (not HTTP/2) for better CLI compatibility
- Implement chunked transfer encoding for animations
- Flush after each frame for real-time output
- Handle client disconnects gracefully

### Font Management
- Cache loaded fonts in memory
- Support standard FIGlet fonts (doom, standard, banner, etc.)
- Validate font names to prevent path traversal
- Provide font preview endpoint

### Client Detection
- Detect CLI tools (curl, wget, httpie) via User-Agent
- Return plain text for CLI, HTML for browsers
- Show help text when accessing root without text

### Testing Requirements
- Minimum 80% test coverage
- Test with race detector enabled
- Include integration tests for HTTP endpoints
- Test streaming behavior and timeouts

## Commit Guidelines

### Conventional Commits Required
- **Strict conventional commit format**: `type(scope): message`
- **Very short messages**: First line ~60 chars max
- **Additional lines only for breaking changes**
- **NEVER add watermarks** (no "Generated with Claude", no emoji signatures)
- **Always confirm commit message with user before committing**

### Common Types
- `feat`: New feature
- `fix`: Bug fix
- `test`: Adding tests
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `docs`: Documentation only changes
- `chore`: Changes to build process or auxiliary tools

### Examples
```bash
# Good - short and clear
git commit -m "feat: add rainbow color scheme"
git commit -m "fix: handle empty text input"
git commit -m "test: add party mode streaming tests"

# Bad - too long, has watermark
git commit -m "feat: implement comprehensive rainbow color scheme with multiple gradient options 🤖 Generated with Claude"
```