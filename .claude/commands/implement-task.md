---
description: Implement a task from docs/tasks.md following TDD practices with comprehensive testing and documentation
---

# Implementing Task: $ARGUMENTS

I'll implement task **$ARGUMENTS** from the shout.sh project following Test-Driven Development (TDD) practices. Let me start by analyzing the task and creating a comprehensive implementation plan.

## Step 1: Task Analysis

First, let me load and analyze the task details from the docs/tasks.md file.

I'll extract:
- Task description and requirements
- Dependencies that must be completed first
- Acceptance criteria that must be met
- Any sample code or patterns to follow

## Step 2: Verify Prerequisites

Before starting, I'll verify:
1. All dependency tasks are implemented
2. Working directory is clean (no uncommitted changes)
3. We're on the main branch and up to date
4. Required development tools are installed (goimports, golangci-lint)

I'll run:
- `git status` - to check for uncommitted changes
- `git branch --show-current` - to verify current branch

Note: If this is task SHO-001, I'll ensure the following tools are installed:
- `go install golang.org/x/tools/cmd/goimports@latest` - for formatting and import management
- `go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest` - for comprehensive linting

## Step 3: Create Feature Branch

I'll create a descriptive feature branch for this task based on what it implements:
- Example: `git checkout -b feat/$ARGUMENTS-config-loading` (for configuration task)
- Example: `git checkout -b feat/$ARGUMENTS-ascii-generation` (for ASCII art task)
- The branch name will follow the pattern: `feat/TASK-ID-brief-description`

## Step 4: Implementation Planning

Based on the task analysis, I'll:
1. Identify all components that need to be created/modified
2. List the test cases needed to verify functionality
3. Note any unclear requirements that need clarification
4. Refer to the PRD in docs/prd.md if any architectural or design clarification is needed

**Questions for you before proceeding:**
- Are there any specific implementation preferences or patterns you want me to follow?
- Any existing code or libraries I should reuse?
- Specific error handling approaches you prefer?
- Performance requirements or constraints I should consider?
- Should I check the PRD for any additional context about this feature?

## Step 5: Test-First Development

### 5.1 Write Test Cases
I'll create comprehensive test files (*_test.go) that:
- Cover all acceptance criteria
- Include unit tests for individual functions
- Add integration tests where appropriate
- Test error conditions and edge cases
- Aim for >80% code coverage

### 5.2 Run Tests (Expect Failures)
I'll run the tests initially (expecting them to fail since code isn't implemented yet):
- `go test -v ./...` - verbose output showing all test results

### 5.3 Implement Minimal Code
Write just enough code to make tests pass:
- Follow Go best practices and idioms
- Use appropriate error handling
- Add comprehensive GoDoc comments
- Include usage examples in comments

### 5.4 Refactor
Once tests pass, refactor for:
- Better readability
- Performance optimization
- Code reuse
- Proper separation of concerns

## Step 6: Code Documentation

All exported types and functions will include:
```go
// TypeName represents [clear description].
// 
// Usage example:
//   instance := NewTypeName(...)
//   result := instance.Method()
//
// The type is safe for concurrent use.
type TypeName struct {
    // Document all exported fields
}

// MethodName performs [specific action] and returns [what].
//
// Parameters:
//   - param1: description of what param1 represents
//   - param2: description of what param2 represents
//
// Returns:
//   - result: what the result represents
//   - error: possible error conditions
//
// Example:
//   result, err := MethodName(value1, value2)
//   if err != nil {
//       // handle error
//   }
func MethodName(param1 Type1, param2 Type2) (result ResultType, err error) {
    // implementation
}
```

## Step 7: Quality Assurance

### 7.1 Format and Organize Imports
- `goimports -w .` - format code and organize imports (adds missing imports, removes unused ones)

### 7.2 Run Comprehensive Linting
- `golangci-lint run ./...` - run multiple linters including staticcheck, gosec, ineffassign, and more

### 7.3 Run Tests with Coverage
- `go test -v -race -cover ./...` - run all tests with race detection and coverage report

### 7.4 Check for Module Issues
- `go mod tidy` - clean up module dependencies
- `go mod verify` - verify dependencies are correct

## Step 8: Update Documentation

### 8.1 Update Task in docs/tasks.md
I'll add an "Implementation Notes" section to the task with:
- Actual implementation approach taken
- Any additional dependencies discovered
- Gotchas or challenges encountered
- Patterns that worked well
- Performance considerations
- Integration points with other components

### 8.2 Update README if needed
If the task introduces new:
- Setup requirements
- Configuration options
- Usage patterns
- API endpoints

### 8.3 Update API Documentation
If the task adds new public interfaces

## Step 9: Commit Changes

### 9.1 Stage Changes
I'll stage all changes:
- `git add -A` - add all new and modified files

### 9.2 Create Descriptive Commit
I'll commit following the project's **strict** conventional commit guidelines from CLAUDE.md:

**IMPORTANT Commit Rules:**
- **Very short messages**: First line ~60 chars max
- **Conventional format**: `type(scope): message` or `type: message`
- **No watermarks**: Never add "Generated with Claude" or emoji signatures
- **Minimal body**: Additional lines only for breaking changes
- **Always confirm**: Get user approval before committing

**Good Examples:**
```
feat: initialize go module and project setup
fix: handle empty text input correctly
test: add party mode streaming tests
```

**Bad Examples (TOO LONG):**
```
feat($ARGUMENTS): implement comprehensive task with multiple features
```

I'll propose a short, clear commit message and confirm with you before committing.

## Step 10: Final Validation

### 10.1 Verify Acceptance Criteria
I'll go through each acceptance criterion and confirm it's met:
- [ ] Criterion 1: [verify how it's satisfied]
- [ ] Criterion 2: [verify how it's satisfied]
- [ ] ... (all criteria from the task)

### 10.2 Run Final Test Suite
- `go test -v ./...` - ensure all tests pass

### 10.3 Check No Formatting or Linting Issues  
- `goimports -l .` - list any files that need formatting (should return nothing)
- `golangci-lint run ./...` - ensure no linting issues remain

## Step 11: Task Completion

Once everything is verified:
1. Update task status in docs/tasks.md (mark criteria as completed)
2. Note any follow-up tasks or improvements identified
3. Prepare for PR if needed

## Progress Tracking

Throughout this process, I'll use the TodoWrite tool to track implementation progress and ensure nothing is missed.

---

**Ready to start implementation of $ARGUMENTS!** 

Please confirm you want me to proceed, and answer any questions I've raised in Step 4.