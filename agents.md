# Embedded Vector Database (vdb) - Agent Workflow

## Project Overview
- **Name**: Embedded Vector Database (vdb)
- **Language**: Rust
- **Version**: 0.1.0-alpha
- **Type**: Embedded, single-file storage system
- **Primary Use**: Knowledge bases, semantic search, and vector indexing

---

## Agent Workflow and Conventions

### 1. Project Structure
The project follows a modular structure:
- **Source Code**: `src/`
- **Tests**: `tests/`
- **Documentation**: `dev_docs/`
- **Configuration**: `Cargo.toml`

### 2. Git Workflow
- **Branching Strategy**: Feature branches for each epic or task.
- **Commit Conventions**: Follow [Conventional Commits](https://www.conventionalcommits.org/).
  - `feat:` for new features
  - `fix:` for bug fixes
  - `docs:` for documentation changes
  - `refactor:` for code refactoring
  - `test:` for adding or updating tests

### 3. Agent Roles and Responsibilities

#### **Main Agent (You)**
- **Responsibilities**:
  - Orchestrate tasks across sub-agents.
  - Ensure adherence to project conventions.
  - Generate commit messages and plan work.
  - Validate changes before committing.

#### **Sub-Agents**
- **Scout**: Locate code, dependencies, or documentation.
- **Builder**: Implement features, fix bugs, or refactor code.
- **Reviewer**: Review code changes, tests, and documentation.
- **Tester**: Write and run tests to ensure correctness.
- **Documenter**: Update documentation and maintain `dev_docs/`.

### 4. Task Delegation
- **Parallel Work**: Use `task` to delegate independent tasks to sub-agents.
- **Sequential Work**: Execute tasks sequentially when dependencies exist.
- **Context Sharing**: Use `local://` files to share context between agents.

### 5. Commit Message Planning
Before committing, ensure:
1. **Changes are staged** (`git add`).
2. **Commit message follows Conventional Commits** format.
3. **Changes are validated** (tests pass, code is reviewed).
4. **Documentation is updated** if applicable.

---

## Example Workflow

### Step 1: Plan the Work
- **Task**: "Implement HNSW graph indexing for vector search."
- **Sub-Tasks**:
  - Add HNSW graph module (`src/graph.rs`).
  - Update vector indexing logic.
  - Write tests (`tests/vector_database_test.rs`).
  - Update documentation.

### Step 2: Delegate Tasks
```
{
  "i": "Delegate tasks",
  "tasks": [
    {
      "task": "Implement HNSW graph module in `src/graph.rs`.",
      "agent": "builder"
    },
    {
      "task": "Write tests for HNSW graph in `tests/vector_database_test.rs`.",
      "agent": "tester"
    },
    {
      "task": "Update documentation in `dev_docs/`.",
      "agent": "documenter"
    }
  ]
}
```

### Step 3: Review and Validate
- **Review**: Use `xd://mcp__mem_add_observations` to log changes and validate.
- **Test**: Run tests to ensure correctness.

### Step 4: Commit
- **Generate Commit Message**:
  ```
  feat: Add HNSW graph indexing for vector search
  - Implement HNSW graph in `src/graph.rs`
  - Add tests in `tests/vector_database_test.rs`
  - Update documentation
  ```

- **Commit**:
  ```bash
  git commit -m "feat: Add HNSW graph indexing for vector search\n\n- Implement HNSW graph in `src/graph.rs`\n- Add tests in `tests/vector_database_test.rs`\n- Update documentation"
  ```

---

## Knowledge Graph Integration
- Use `xd://mcp__mem_add_observations` to log changes and track progress.
- Example:
  ```
  {
    "i": "Log changes",
    "entity": "VectorIndexing",
    "observations": ["Implemented HNSW graph indexing"]
  }
  ```

---

## Next Steps
1. **Initialize the repository** with the correct structure.
2. **Plan the first set of tasks** (e.g., core database setup).
3. **Delegate tasks** to sub-agents.
4. **Review, validate, and commit** changes.

---