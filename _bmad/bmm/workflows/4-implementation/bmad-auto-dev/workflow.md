# Auto-Dev Workflow

**Goal:** Autonomous board-driven development loop. Pick tasks from the Task Board, implement them, validate with tests, and update the board.

**Your Role:** You are the auto-dev agent. You pick up tasks, write code, run tests, and update the board. Execute autonomously until the task is complete or a blocker is hit.

---

## INITIALIZATION

### Configuration Loading

Load config from `{project-root}/_bmad/bmm/config.yaml` and resolve:

- `project_name`, `user_name`
- `communication_language`, `document_output_language`
- `user_skill_level`
- `date` as system-generated current datetime

### Path Discovery

All paths are resolved relative to `{project-root}`:

- `BOARD_STORE` = `{project-root}/_bmad-output/board-store.json`
- `PLUGIN_BINARY` = `{project-root}/config/plugins/plugin-coding-pack`
- `PLUGIN_RPC` = the plugin binary, called via JSON-RPC stdin/stdout

Discover the Pulse installation:
- Check if `PLUGIN_BINARY` exists and is executable
- If not, search for `plugin-coding-pack` in parent directories under `config/plugins/`
- The Pulse workspace root is the grandparent of the `config/plugins/` directory containing the binary

### Pulse Server Detection

- Check if Pulse is running: `curl -s http://localhost:3000 | head -1`
- If not running, inform the user and continue without dashboard sync
- If running, board changes will be visible in the dashboard after deploy

---

## COMMANDS

Parse the user's intent and execute the matching section.

### "status" / "show board" / "board status"

Read `{BOARD_STORE}` directly (or query via plugin RPC) and display a Kanban summary:

```
[backlog]        (N cards)
[ready-for-dev]  (N cards)  ← auto-dev picks from here
[in-progress]    (N cards)
[review]         (N cards)
[done]           (N cards)
```

Show the next task that auto-dev would pick (highest priority ready-for-dev).

### "create task" / "add task [description]"

Create a new assignment via JSON-RPC to the plugin binary:

```bash
echo '{RPC_CREATE_ASSIGNMENT}' | {PLUGIN_RPC}
```

Required fields from user: `title`, `description`
Auto-defaults: `status: "ready-for-dev"`, `priority: "high"`
Ask the user for any missing required fields.

### "next" / "run auto-dev" / "pick up next task"

Execute the full auto-dev cycle:

#### Step 1: Read the board
Read `{BOARD_STORE}`. Find the highest-priority `ready-for-dev` assignment.
If none found, tell the user and stop.

#### Step 2: Claim the task
Update assignment status to `in-progress` via JSON-RPC.
Add comment: `[auto-dev] Starting implementation`.

#### Step 3: Implement
Read the task's title, description, and subtasks. Then:
- Read relevant source files to understand context
- Write code to implement the task
- Follow existing code patterns and conventions in `{project-root}/src/`
- Check off subtasks as you complete them (toggle via JSON-RPC)

#### Step 4: Validate
Run the project's test suite:
```bash
cd {project-root} && cargo test
```
- If all tests pass → proceed to Step 5
- If tests fail → fix the code and re-run, up to 3 attempts
- If still failing after 3 attempts → update board with failure comment, keep status `in-progress`, STOP

#### Step 5: Complete
- Update assignment status to `review` via JSON-RPC
- Add comment with summary: `[auto-dev] Completed. {test_count} tests passing. Changes: {brief_summary}`
- Toggle all remaining subtasks to done

#### Step 6: Deploy (if Pulse is running)
If Pulse server is detected:
```bash
cd {project-root} && cargo build --release
```
Copy the new binary to the Pulse plugins directory and restart Pulse so the dashboard reflects changes.

### "deploy" / "build and deploy"

Build the plugin and deploy to Pulse:
1. `cd {project-root} && cargo build --release`
2. Find and stop the running Pulse server
3. Copy the release binary to Pulse's `config/plugins/`
4. Ensure `{BOARD_STORE}` is symlinked into Pulse's `_bmad-output/`
5. Restart Pulse server
6. Verify with `curl -s http://localhost:3000 | head -1`

### "clear" / "clear board"

Remove `{BOARD_STORE}` to reset the Task Board.

### "watch" / "run all tasks"

Loop the "next" command until no `ready-for-dev` tasks remain.

---

## JSON-RPC PROTOCOL

All board mutations go through the plugin binary via stdin JSON-RPC.

**Base format:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "step-executor.execute",
  "params": {
    "task": {
      "task_id": "ID",
      "description": "DESC",
      "input": {
        "action": "data-mutate",
        "endpoint": "ENDPOINT",
        "payload": { PAYLOAD }
      }
    },
    "config": {
      "step_id": "s1",
      "step_type": "function"
    }
  }
}
```

**Endpoints:**

| Operation | Endpoint | Payload |
|-----------|----------|---------|
| Create assignment | `board/assignments` | `{"title":"...","description":"...","status":"ready-for-dev","priority":"high","labels":[...],"tasks":["subtask1","subtask2"]}` |
| Update assignment | `board/assignments/{id}` | `{"status":"review"}` or any updatable fields |
| Add comment | `board/assignments/{id}/comments` | `{"content":"...","author":"auto-dev"}` |
| Add subtask | `board/assignments/{id}/subtasks` | `{"title":"..."}` |
| Toggle subtask | `board/assignments/{id}/subtasks/{st_id}/toggle` | `{}` |

**Reading data (no mutation):**
```json
{
  "action": "data-query",
  "endpoint": "board/assignments/list"
}
```

---

## RULES

- All paths relative to `{project-root}` — never hardcode absolute paths
- Run `cargo test` after every code change — all tests must pass before marking "review"
- Add meaningful board comments documenting what was implemented
- Follow existing code patterns in `{project-root}/src/`
- If a task is unclear, add a comment asking for clarification and keep status as `ready-for-dev`
- Do not modify files outside `{project-root}` without explicit user permission
- Toggle subtasks as you complete them for real-time progress tracking
