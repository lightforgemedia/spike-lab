# spike-lab

Research spike management with [Jules](https://jules.google.com) integration.

## Setup

```bash
# Add to PATH
export PATH="$PATH:$HOME/PROJECTS/spike-lab/bin"
```

## Usage

```bash
# Create a new spike
spike new graphql-perf "Investigate GraphQL query performance"

# Send task to Jules
spike jules graphql-perf "Profile and optimize N+1 queries"

# List all spikes
spike list

# Check spike status
spike status graphql-perf
```

## Structure

```
spike-lab/
├── bin/spike          # CLI tool
├── spikes/            # All spike directories
│   └── <spike-name>/
│       ├── README.md        # Spike overview & findings
│       ├── notes/           # Detailed notes
│       ├── artifacts/       # Code, diagrams, outputs
│       └── jules-tasks.log  # History of Jules tasks
```

## Workflow

1. **Create spike**: `spike new <name> "description"`
2. **Research**: Add notes to `spikes/<name>/notes/`
3. **Delegate to Jules**: `spike jules <name> "task description"`
4. **Capture artifacts**: Save outputs to `spikes/<name>/artifacts/`
5. **Document findings**: Update `spikes/<name>/README.md`
