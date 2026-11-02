#!/usr/bin/env bash
# Keep the fork's published history: fast-forward origin, then merge upstream.
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: bash scripts/sync-upstream.sh [--check | --push]
  --check  Fetch and show incoming commits; leave HEAD and the worktree alone.
  default  Merge upstream/main into a clean local main; do not push.
  --push   Merge, then push only main to origin (no force and no tags).
USAGE
}
die() { printf 'Error: %s\n' "$*" >&2; exit 1; }
mode=merge
case "${1:-}" in
    '') ;;
    --check) mode=check ;;
    --push) mode=push ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { usage >&2; exit 2; }

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$script_dir" rev-parse --show-toplevel)
cd "$repo"
[ "$(git branch --show-current)" = main ] || die 'Switch to main before syncing.'
[ "$(git rev-parse --is-shallow-repository)" = false ] || die 'Fetch full history before merging: git fetch --unshallow origin'
for state in MERGE_HEAD CHERRY_PICK_HEAD REVERT_HEAD rebase-merge rebase-apply; do
    [ ! -e "$(git rev-parse --git-path "$state")" ] || die 'Finish or abort the current Git operation first.'
done
if [ "$mode" != check ] && [ -n "$(git status --porcelain)" ]; then
    die 'Commit or stash your changes (including untracked files) before syncing.'
fi

origin_push=$(git remote get-url --push origin) || die 'Configure origin as your fork.'
upstream_fetch=$(git remote get-url upstream) || die 'Add upstream first; see FORK.md.'
origin_push_count=$(git remote get-url --push --all origin | wc -l)
[ "$origin_push_count" -eq 1 ] || die 'origin must have exactly one push URL.'
[ "$origin_push" != "$upstream_fetch" ] || die 'origin points at upstream; configure your fork before syncing.'
case "$origin_push" in
    *jesther-ai/open-u60-pro*) die 'origin must push to your fork, not jesther-ai/open-u60-pro.' ;;
esac

git fetch --prune --no-tags origin
git fetch --prune --no-tags upstream
git rev-parse --verify refs/remotes/origin/main >/dev/null
git rev-parse --verify refs/remotes/upstream/main >/dev/null

printf '\nLocal / origin-only commits: '
git rev-list --left-right --count HEAD...origin/main
printf 'Local / upstream-only commits: '
git rev-list --left-right --count HEAD...upstream/main
git log --oneline --max-count=12 HEAD..upstream/main
[ "$mode" != check ] || exit 0

# Never reconcile divergent fork branches silently: that deserves a review.
if ! git merge-base --is-ancestor origin/main HEAD; then
    git merge --ff-only origin/main || die 'Local main and origin/main diverged. Review them before syncing; no force push was attempted.'
fi
if ! git merge --no-edit upstream/main; then
    printf '\nResolve conflicts, git add the resolved files, then git merge --continue.\nTo cancel this merge: git merge --abort\nNothing was pushed.\n' >&2
    exit 1
fi
git diff --check

if [ "$mode" = push ]; then
    git -c push.followTags=false push origin HEAD:refs/heads/main
else
    printf '\nMerged locally. Review the changes and run the relevant checks in FORK.md.\nThen publish: git -c push.followTags=false push origin HEAD:refs/heads/main\n'
fi
