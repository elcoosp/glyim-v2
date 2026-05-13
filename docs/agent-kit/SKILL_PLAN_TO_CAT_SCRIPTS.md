# Skill: plan-to-cat-scripts (File-Watcher Friendly, Branch-Aware, Worktree-Aware)

### Purpose
Given an implementation plan, the LLM outputs a sequence of self-contained bash scripts, one per message, that write or surgically patch every file to its exact state, never truncate or use placeholders, and automatically test and commit.

When users paste error logs, the LLM switches to surgical fix mode, producing a single-message script that corrects only the problematic lines. Every operation is logged with echo, so the error log pinpoints exactly what failed.

---

### Critical Rules
- No comment lines starting with hash -- every action is prefixed by an echo.
- No backup files -- if a tool fails, the original file is never overwritten.
- sed is only for trivial single-line substitutions. All other changes MUST use Python with the actual strings stored in temp files, never embedded in Python source code.
- Heredoc delimiters must be unique and must not appear as a complete line in the content being written. For each heredoc, the LLM generates a random delimiter, checks that it does not match any line of the content, and uses it.
- Failures from sed, Python, or other commands are logged but do not halt the script. A compile check runs after all patches; if it fails, the failure is logged but the script continues to the end, skipping only the commit, and exits with non-zero to signal the file-watcher.
- Never use head or tail to arbitrarily split file content across scripts. Splitting must be done only by writing complete, syntactically coherent chunks (e.g., entire functions, modules, or at logical blank lines) -- never by line count.
- Every file written or patched must be syntactically valid (as a whole) whenever the script marks it as complete. The compile check (cargo check) serves as the final safety net, but the LLM must strive to avoid introducing bracket/delimiter mismatches in the first place.

---

### Output Format
- Each LLM message is exactly one fenced markdown code block with triple-backtick-bash opening and triple-backtick closing. No other text -- no explanations, no filenames, only the fenced block.
- The script inside the fence is immediately executable once the fences are stripped.
- The LLM emits multiple consecutive messages until the plan is finished.

---

### Script Structure (Per Script)
Every script starts with:

    STREAM_ID="S01"
    WORKTREE_DIR="../glyim-worktrees/stream-${STREAM_ID}"
    COMPILE_OK=true
    INCOMPLETE=false

**IMPORTANT:** All scripts MUST change directory into the worktree before doing anything else. The first script creates the worktree; subsequent scripts assume it exists.

---

0. Worktree & Branch Setup (First Script Only)

    echo "Setting up worktree for stream ${STREAM_ID}"
    if [ ! -d "$WORKTREE_DIR" ]; then
      git worktree add "$WORKTREE_DIR" main
    fi
    cd "$WORKTREE_DIR" || { echo "ERROR: cannot cd to $WORKTREE_DIR"; exit 1; }

    BRANCH_NAME="stream-${STREAM_ID}/v0.1.0"
    if git rev-parse --verify "$BRANCH_NAME" >/dev/null 2>&1; then
      echo "Branch $BRANCH_NAME already exists, checking it out"
      git checkout "$BRANCH_NAME"
    else
      echo "Creating branch $BRANCH_NAME from main"
      git checkout main
      git pull origin main 2>/dev/null || true
      git checkout -b "$BRANCH_NAME"
    fi

This ensures:
- No work happens on main directly.
- The branch follows the convention stream-SXX/v0.1.0.
- The worktree is created once and reused.
- Subsequent scripts in the same stream do NOT repeat worktree creation.

1. Write a complete file -- use a heredoc with a safe delimiter:

    echo "Writing /absolute/path/to/file"
    cat > /absolute/path/to/file << 'SAFE_UNIQUE_DELIM'
    entire content here
    SAFE_UNIQUE_DELIM

Rule: The delimiter must be chosen by the LLM so that it does not appear as an entire line in the file content.

2. Start a huge file (if needed) -- with a meaningful chunk, never an arbitrary line cut.

    echo "Starting /path (partial -- up to end of fn foo())"
    cat > /path << 'CHUNK_X9yZ0'
    first part, complete up to a logical break
    CHUNK_X9yZ0
    INCOMPLETE=true
    echo "File /path is incomplete, continuation required"

The chunk must be syntactically balanced. Later scripts will cat >> the remainder.

3. Patch an existing file:

A. Trivial single-line substitution (sed only if extremely simple)
Choose a delimiter (pipe, at, etc.) that does not appear in the old or new text.

    echo "Patching /path: replace 'old' with 'new'"
    if sed 's|old|new|g' /path > /path.tmp; then
      mv /path.tmp /path
    else
      echo "ERROR: sed failed for /path"
      rm -f /path.tmp
    fi

B. Non-trivial changes (multi-line, special chars, blocks) -- Python with temp files
The old and new strings are written to temporary files using safe heredocs. Python reads those files and performs a literal replacement (str.replace). No Python string literals contain the actual content, eliminating all quoting errors.

    echo "Patching /target/file"
    OLD_TMP=$(mktemp) || { echo "ERROR: cannot create temp file"; exit 1; }
    NEW_TMP=$(mktemp)
    cat > "$OLD_TMP" << 'OLD_DELIM_UNIQUE'
    old multiline content exactly as it appears
    OLD_DELIM_UNIQUE
    cat > "$NEW_TMP" << 'NEW_DELIM_UNIQUE'
    new multiline content exactly
    NEW_DELIM_UNIQUE
    if python3 - "$OLD_TMP" "$NEW_TMP" /target/file << 'PYEOF'
    import sys
    with open(sys.argv[1], 'r') as f: old = f.read()
    with open(sys.argv[2], 'r') as f: new = f.read()
    with open(sys.argv[3], 'r') as f: content = f.read()
    content = content.replace(old, new)
    with open(sys.argv[3], 'w') as f: f.write(content)
    PYEOF
    then
      echo "Python patch succeeded for /target/file"
      rm "$OLD_TMP" "$NEW_TMP"
    else
      echo "ERROR: Python patch failed for /target/file"
      rm -f "$OLD_TMP" "$NEW_TMP"
    fi

Why this is safe: The old/new strings live in temp files written via heredocs -- no Python quoting is involved.

4. Compile check (after all file operations):

    echo "Checking compilation"
    if ! cargo check --workspace 2>&1; then
      echo "Compilation failed -- will skip commit"
      COMPILE_OK=false
    fi

5. Commit decision (with stream-prefixed commit messages):

    if [ "$INCOMPLETE" = true ] || [ "$COMPILE_OK" = false ]; then
      echo "Skipping tests and commit due to incomplete files or compilation errors"
      exit 1
    fi

    echo "Running tests"
    cargo nextest run --workspace
    if [ $? -eq 0 ]; then
      echo "All tests passed. Committing."
      git add -A
      git commit -m "stream-${STREAM_ID}: feat(scope): description"
    else
      echo "Tests failed. Fix errors then run the next script."
      exit 1
    fi

---

### Surgical Fixes for User-Reported Errors
When the user pastes a terminal log that ends with a non-zero exit, the LLM responds with a single-message fenced script that fixes only the operation that failed. Surgical fix scripts **must**:

- Set `STREAM_ID` and `WORKTREE_DIR` as usual.
- `cd "$WORKTREE_DIR"` (the worktree already exists).
- Do NOT create a new branch or worktree.
- Perform the minimal patch to correct the error.

---

### Handling Truncation -- The Golden Rule
The LLM never outputs a truncated file or placeholder. When the message is nearly full, stop after the last complete file or patch and set INCOMPLETE=true. The next script continues the work.

Splitting huge files: If a file must be split across scripts, each chunk must be a syntactically valid fragment, not an arbitrary byte or line cut.

---

### Commit Messages
All commit messages are prefixed with the stream ID:
stream-S01: chore: set up workspace skeleton
stream-S09: feat(parse): add Pratt expression parser
stream-S01: fix(lex): correct keyword recognition

---

### Execution Contract
1. Strip the leading triple-backtick-bash line and trailing triple-backtick line from each LLM message, save the result as a .sh file.
2. Execute them **one at a time**, in the order they were emitted.
3. If a script exits non-zero (commit not possible), **stop** and paste the entire terminal output into the chat. The LLM will respond with a surgical fix.
4. If a script exits zero, proceed to the next script.
5. The first script in a stream creates the worktree and branch. Subsequent scripts and fix scripts assume the worktree exists and cd into it.

---

### Worktree Workflow
- First script: Creates `../glyim-worktrees/stream-SXX` (relative to the main repo root), adds it as a worktree pointing to `main`, then checks out/creates the stream branch.
- Subsequent scripts: Assume the worktree exists, `cd` into it, and proceed.
- Fix scripts: Same as subsequent scripts.
- Never commit to main directly. All work happens in the worktree on the stream branch.
- When the stream is complete, push the branch from the worktree and create a PR against main. The worktree can be deleted after merging.

---

### Final Guarantees
Every file is written or patched with exact intended content -- or the patch is logged as failed and the original file is untouched.
No hash comment lines, no .bak files, no base64 blobs -- all content is plainly visible.
No arbitrary head/tail splitting -- multi-script files are divided at logical syntactic boundaries.
sed is only for trivial changes; all complex patches use Python with temp files.
Compile check failure never interrupts the script -- it only blocks the commit.
Errors are logged but never abort the script early; every operation is attempted.
Messages are fenced bash code blocks -- easy to copy, fences stripped automatically.
Repository always passes compilation and tests after a successful (exit 0) script.
Error logs enable precise, restart-free surgical fixes.
All work happens in a dedicated worktree on a stream-specific branch -- main is never modified directly.
