<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->


Use 'bd' for task tracking


## Beads Issue Tracking


**BEFORE ANY WORK**
: Run `bd onboard` if you haven't already this session.


### When to Use Beads vs OpenSpec


| Situation | Tool | Action |
|-----------|------|--------|
| New feature/capability | OpenSpec | `/openspec:proposal` first |
| Approved spec ready for implementation | Both | Import tasks to Beads, then implement |
| Bug fix, small task, tech debt | Beads | `bd create` directly |
| Discovered issue during work | Beads | `bd create --discovered-from <parent>` |
| Tracking what's ready to work on | Beads | `bd ready` |
| Feature complete | OpenSpec | `/openspec:archive` |


### Daily Workflow


1. 
**Orient**
: Run `bd ready --json` to see unblocked work
2. 
**Pick work**
: Select highest priority ready issue OR continue in-progress work
3. 
**Update status**
: `bd update <id> --status in_progress`
4. 
**Implement**
: Do the work
5. 
**Discover**
: File any new issues found: `bd create "Found: <issue>" -t bug --discovered-from <current-id>`
6. 
**Complete**
: `bd close <id> --reason "Implemented"`


### Converting OpenSpec Tasks to Beads


When an OpenSpec change is approved and ready for implementation:
```bash
# Create epic for the change
bd create "<change-name>" -t epic -p 1 -l "openspec:<change-name>"


# For each task in tasks.md, create a child issue
bd create "<task description>" -t task -l "openspec:<change-name>"
```


Keep OpenSpec `tasks.md` and Beads in sync:
- When completing a Beads issue, also mark `[x]` in tasks.md
- When all Beads issues for a change are closed, run `/openspec:archive`


### Importing OpenSpec Tasks to Beads


When converting OpenSpec tasks to Beads issues, ALWAYS include full context. Issues must be 
**self-contained**
 — an agent must understand the task without re-reading OpenSpec files.


**REQUIRED in every issue description:**
1. Spec file reference path
2. Relevant requirements (copy key points)
3. Acceptance criteria from the spec
4. Any technical context needed


**BAD — Never do this:**
```bash
bd create "Update stripe-price.entity.ts" -t task
```


**GOOD — Always do this:**
```bash
bd create "Add description and features fields to stripe-price.entity.ts" -t task -p 2 \
  -l "openspec:billing-improvements" \
  -d "## Spec Reference
openspec/changes/billing-improvements/specs/billing/spec.md


## Requirements
- Add 'description: string' field (nullable)
- Add 'features: string[]' field for feature list display  
- Sync fields from Stripe Price metadata on webhook


## Acceptance Criteria
- Fields populated from Stripe dashboard metadata
- Features displayed as bullet list on pricing page


## Files to modify
- apps/api/src/billing/entities/stripe-price.entity.ts
- apps/api/src/billing/stripe-webhook.service.ts"
```


**The test:**
 Could someone implement this issue correctly with ONLY the bd description and access to the codebase? If not, add more context.


### Label Conventions


- `openspec:<change-name>` - Links issue to OpenSpec change proposal
- `spec:<spec-name>` - Links to specific spec file
- `discovered` - Issue found during other work
- `tech-debt` - Technical debt items
- `blocked-external` - Blocked by external dependency


---


## Landing the Plane (Session Completion)


**When ending a work session**
, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.


### MANDATORY WORKFLOW


#### 1. File Issues for Remaining Work
Create Beads issues for anything that needs follow-up:
```bash
bd create "TODO: <description>" -t task -p 2
bd create "Bug: <description>" -t bug -p 1
```


#### 2. Run Quality Gates (if code changed)
- Tests, linters, builds
- File P0 issues if builds are broken


#### 3. Update All Tracking
**Beads issues:**
```bash
bd close <id> --reason "Completed"           # Finished work
bd update <id> --status in_progress          # Partially done
bd update <id> --add-note "Session end: <context>"  # Add context
```


**OpenSpec tasks.md:**
- Mark completed tasks: `- [x] Task description`
- Add notes for partial progress


#### 4. Sync and Push (MANDATORY)
```bash
# Sync Beads database
bd sync


# Pull, rebase, push
git pull --rebase
git add -A
git commit -m "chore: session end - <summary>"
git push


# VERIFY - must show "up to date with origin"
git status
```


#### 5. Clean Up
- Clear stashes: `git stash clear` (if appropriate)
- Prune remote branches if needed


#### 6. Verify Final State
```bash
bd list --status open    # Review open issues
bd ready                 # Show what's ready for next session
git status               # Must be clean and pushed
```


#### 7. Hand Off
Provide context for next session:
```
## Next Session Context
- Current epic: <id and name>
- Ready work: `bd ready` shows N issues
- Blocked items: <any blockers>
- Notes: <important context>
```


### CRITICAL RULES
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
- ALWAYS run `bd sync` before committing to capture issue changes
