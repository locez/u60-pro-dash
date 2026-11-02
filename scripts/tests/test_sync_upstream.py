#!/usr/bin/env python3
"""Exercise the fork workflow against disposable local Git remotes."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "sync-upstream.sh"


class SyncUpstreamTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="u60-sync-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
                        GIT_TERMINAL_PROMPT="0", GIT_MERGE_AUTOEDIT="no")
        self.origin, self.upstream = self.root / "origin.git", self.root / "upstream.git"
        self.seed, self.repo = self.root / "seed", self.root / "work"
        for remote in [self.origin, self.upstream]:
            self.git(self.root, "init", "--bare", "-b", "main", str(remote))
        self.git(self.root, "init", "-b", "main", str(self.seed))
        self.identity(self.seed)
        (self.seed / "scripts").mkdir()
        shutil.copyfile(SCRIPT, self.seed / "scripts/sync-upstream.sh")
        self.commit(self.seed, "shared.txt", "base\n")
        for name, remote in [("origin", self.origin), ("upstream", self.upstream)]:
            self.git(self.seed, "remote", "add", name, str(remote))
            self.git(self.seed, "push", name, "main")
        self.git(self.root, "clone", str(self.origin), str(self.repo))
        self.identity(self.repo)
        self.git(self.repo, "remote", "add", "upstream", str(self.upstream))
        self.git(self.repo, "config", "pull.ff", "only")
        self.initial = self.head(self.repo)

    def git(self, cwd, *args, check=True):
        result = subprocess.run(["git", *args], cwd=cwd, env=self.env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        if check and result.returncode:
            self.fail(result.stdout)
        return result.stdout.strip()

    def identity(self, repo):
        self.git(repo, "config", "user.name", "Fork workflow test")
        self.git(repo, "config", "user.email", "test@example.invalid")
        self.git(repo, "config", "commit.gpgsign", "false")

    def commit(self, repo, name, text):
        (repo / name).write_text(text)
        self.git(repo, "add", ".")
        self.git(repo, "commit", "-m", "change " + name)
        return self.head(repo)

    def head(self, repo):
        return self.git(repo, "rev-parse", "main")

    def incoming(self, name="incoming.txt", text="upstream\n"):
        self.commit(self.seed, name, text)
        self.git(self.seed, "push", "upstream", "main")

    def sync(self, *args, success=True):
        result = subprocess.run(["bash", "scripts/sync-upstream.sh", *args], cwd=self.repo,
                                env=self.env, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT)
        self.assertEqual(result.returncode == 0, success, result.stdout)
        return result.stdout

    def test_check_does_not_merge_or_discard_work(self):
        self.incoming()
        (self.repo / "draft.txt").write_text("unsaved")
        self.sync("--check")
        self.assertEqual(self.head(self.repo), self.initial)
        self.assertFalse((self.repo / "incoming.txt").exists())
        self.assertEqual((self.repo / "draft.txt").read_text(), "unsaved")

    def test_merge_preserves_fork_work_without_pushing(self):
        self.commit(self.repo, "custom.txt", "fork feature")
        self.incoming()
        self.sync()
        self.assertEqual((self.repo / "custom.txt").read_text(), "fork feature")
        self.assertTrue((self.repo / "incoming.txt").exists())
        self.assertEqual(len(self.git(self.repo, "show", "-s", "--format=%P").split()), 2)
        self.assertEqual(self.head(self.origin), self.initial)

    def test_push_updates_only_main_even_with_follow_tags_enabled(self):
        self.commit(self.repo, "custom.txt", "fork feature")
        self.git(self.repo, "tag", "-a", "v0.0.1-test", "-m", "local-only tag")
        self.git(self.repo, "config", "push.followTags", "true")
        self.incoming()
        self.sync("--push")
        self.assertEqual(self.head(self.origin), self.head(self.repo))
        self.assertEqual(self.git(self.origin, "tag", "--list"), "")

    def test_dirty_tree_is_rejected(self):
        (self.repo / "shared.txt").write_text("uncommitted")
        self.assertIn("Commit or stash", self.sync(success=False))
        self.assertEqual(self.head(self.repo), self.initial)

    def test_wrong_branch_and_upstream_push_are_rejected(self):
        self.git(self.repo, "switch", "-c", "feat/test")
        self.assertIn("Switch to main", self.sync(success=False))
        self.git(self.repo, "switch", "main")
        self.git(self.repo, "remote", "set-url", "--push", "origin", str(self.upstream))
        self.assertIn("origin points at upstream", self.sync("--push", success=False))

    def test_receives_new_fork_commits_by_fast_forward(self):
        self.commit(self.seed, "other-machine.txt", "fork update")
        self.git(self.seed, "push", "origin", "main")
        self.sync()
        self.assertEqual(self.head(self.repo), self.head(self.origin))
        self.assertTrue((self.repo / "other-machine.txt").exists())

    def test_divergent_origin_is_not_merged_or_overwritten(self):
        before = self.commit(self.repo, "custom.txt", "local fork")
        self.commit(self.seed, "other-machine.txt", "remote fork")
        self.git(self.seed, "push", "origin", "main")
        origin_before = self.head(self.origin)
        self.assertIn("diverged", self.sync("--push", success=False))
        self.assertEqual(self.head(self.repo), before)
        self.assertEqual(self.head(self.origin), origin_before)

    def test_conflict_stays_reviewable_and_never_pushes(self):
        before = self.commit(self.repo, "shared.txt", "fork change\n")
        self.incoming("shared.txt", "upstream change\n")
        self.assertIn("git merge --abort", self.sync("--push", success=False))
        self.assertTrue((self.repo / ".git/MERGE_HEAD").exists())
        self.assertEqual(self.head(self.origin), self.initial)
        self.assertIn("Finish or abort", self.sync(success=False))
        self.git(self.repo, "merge", "--abort")
        self.assertEqual(self.head(self.repo), before)
        self.assertEqual((self.repo / "shared.txt").read_text(), "fork change\n")


if __name__ == "__main__":
    unittest.main()
