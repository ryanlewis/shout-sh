# Claude Commands for shout.sh

This directory contains custom Claude commands to assist with development of the shout.sh project.

## Available Commands

### `/implement-task [SHO-XXX]`

Implements a task from `docs/tasks.md` following Test-Driven Development (TDD) practices.

**Usage:**
```
/implement-task SHO-001
/implement-task SHO-007
```

**Features:**
- **TDD Workflow**: Writes tests first, then implements code to pass tests
- **Git Integration**: Creates feature branches for each task
- **Quality Assurance**: Runs gofmt, go vet, and all tests
- **Documentation**: Updates docs/tasks.md with implementation notes
- **Code Documentation**: Adds comprehensive GoDoc comments
- **Progress Tracking**: Uses TodoWrite to track implementation steps

**Workflow:**
1. Analyzes task from docs/tasks.md
2. Verifies dependencies are complete
3. Creates feature branch (`feature/SHO-XXX-implementation`)
4. Asks for clarification on requirements
5. Writes comprehensive tests first
6. Implements minimal code to pass tests
7. Refactors and optimizes
8. Adds complete GoDoc documentation
9. Runs quality checks (format, vet, lint, test)
10. Updates docs/tasks.md with implementation notes
11. Commits with descriptive message
12. Validates all acceptance criteria are met

**Requirements:**
- Task must exist in `docs/tasks.md`
- Dependencies must be implemented first
- Git repository must have clean working directory
- Go development environment must be set up

## Creating New Commands

To create a new command:

1. Create a markdown file in `.claude/commands/`
2. Name it after your command (e.g., `my-command.md`)
3. Add frontmatter with description:
   ```markdown
   ---
   description: Brief description of what the command does
   ---
   ```
4. Write the prompt that Claude will execute

## Command Best Practices

- Use `$ARGUMENTS` to receive user input
- Use `@filename` to reference files
- Use `!command` to run shell commands
- Include clear step-by-step instructions
- Ask for user confirmation before making significant changes
- Update relevant documentation after implementation

## Project Structure

```
.claude/
├── commands/
│   └── implement-task.md    # TDD implementation command
├── settings.local.json       # Local Claude settings
└── README.md                 # This file
```