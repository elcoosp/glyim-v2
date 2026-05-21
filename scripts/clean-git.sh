#!/usr/bin/env bash
set -euo pipefail

# Script: clean-git.sh
# Purpose: Delete all worktrees and local branches except main/master.
# Usage: Run from the root of the Git repository.

# Ensure we are in a Git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "Error: Not inside a Git repository."
    exit 1
fi

# Determine the default branch (main or master)
DEFAULT_BRANCH=""
if git show-ref --verify --quiet refs/heads/main; then
    DEFAULT_BRANCH="main"
elif git show-ref --verify --quiet refs/heads/master; then
    DEFAULT_BRANCH="master"
else
    echo "Error: Neither 'main' nor 'master' branch found."
    exit 1
fi

echo "Default branch detected: $DEFAULT_BRANCH"
echo "This script will delete ALL worktrees and local branches except '$DEFAULT_BRANCH'."
read -p "Are you sure? Type 'yes' to continue: " CONFIRM
if [[ "$CONFIRM" != "yes" ]]; then
    echo "Aborted."
    exit 0
fi

# 1. Delete all worktrees except the main one
echo "Deleting worktrees..."
git worktree list --porcelain | grep -E '^worktree ' | cut -d' ' -f2- | while read -r wt_path; do
    # Determine if this is the main worktree (repository root)
    if [[ "$wt_path" == "$(git rev-parse --show-toplevel)" ]]; then
        echo "Skipping main worktree: $wt_path"
        continue
    fi
    echo "Removing worktree: $wt_path"
    git worktree remove --force "$wt_path"
done

# 2. Delete all local branches except DEFAULT_BRANCH
echo "Deleting local branches..."
git branch | grep -v "$DEFAULT_BRANCH" | grep -v '*' | xargs -r git branch -D

echo "Cleanup complete. Remaining branches:"
git branch

echo "Remaining worktrees:"
git worktree list
