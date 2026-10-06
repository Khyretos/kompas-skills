#!/usr/bin/env python3
"""
test_privacy_check.py

Unit tests for tools/skills/privacy_check.py.
"""

import os
import subprocess
import sys
import tempfile
import unittest

# Ensure we can import privacy_check from the local directory
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from privacy_check import load_deny_list, findings as find


class TestFindFindings(unittest.TestCase):
    """Tests for the findings function."""

    def test_planted_ssh_ip(self):
        """A planted 'ssh to 192.168.1.20' gives kind 'ip'."""
        text = "ssh to 192.168.1.20"
        findings = find(text, set())
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0][1], 'ip')
        self.assertEqual(findings[0][2], '192.168.1.20')

    def test_loopback_ip_ignored(self):
        """'127.0.0.1' gives nothing."""
        text = "connect to 127.0.0.1"
        findings = find(text, set())
        self.assertEqual(len(findings), 0)

    def test_home_path_slash(self):
        """'/home/kees/x' gives 'home path'."""
        text = "/home/kees/x"
        findings = find(text, set())
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0][1], 'home path')

    def test_home_path_tilde(self):
        """'~/Docker' gives one finding of kind 'home path'."""
        text = "~/Docker"
        results = find(text, set())
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], 'home path')

    def test_clean_text(self):
        """A clean text 'Use git rebase, never force-push.' gives []."""
        text = "Use git rebase, never force-push."
        results = find(text, set())
        self.assertEqual(results, [])

    def test_email_address(self):
        """'mail me@example.com' gives 'email'."""
        text = "mail me@example.com"
        findings = find(text, set())
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0][1], 'email')
        self.assertEqual(findings[0][2], 'me@example.com')

    def test_deny_host_soucouyant(self):
        """With deny ['soucouyant'], 'run on Soucouyant now' gives 'host'."""
        deny_hosts = {'soucouyant'}
        text = "run on Soucouyant now"
        findings = find(text, deny_hosts)
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0][1], 'host')
        self.assertEqual(findings[0][2], 'soucouyant')

    def test_deny_host_not_found(self):
        """'soucouyants' (plural) gives nothing with deny ['soucouyant']."""
        deny_hosts = {'soucouyant'}
        text = "soucouyants are here"
        findings = find(text, deny_hosts)
        self.assertEqual(len(findings), 0)

    def test_missing_path_fails(self):
        """Running on a missing path with --deny pointing to an empty temp file exits with code 2."""
        with tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.txt') as f:
            pass  # Create empty deny file
        try:
            script_path = os.path.join(os.path.dirname(__file__), "privacy_check.py")
            env = os.environ.copy()
            env["KOMPANION_SKILLS_LOCAL"] = os.path.dirname(f.name)

            result = subprocess.run(
                [sys.executable, script_path, "/nonexistent/path", "--deny", f.name],
                capture_output=True,
                text=True,
                env=env
            )
            self.assertEqual(result.returncode, 2)
        finally:
            os.unlink(f.name)

    def test_clean_text(self):
        """A clean text 'Use git rebase, never force-push.' gives []."""
        text = "Use git rebase, never force-push."
        results = find(text, set())
        self.assertEqual(results, [])


class TestLoadDenyList(unittest.TestCase):
    """Tests for the load_deny_list function."""

    def test_empty_file(self):
        """An empty file returns an empty set."""
        with tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.txt') as f:
            pass  # Create empty file
        try:
            result = load_deny_list(f.name)
            self.assertEqual(result, set())
        finally:
            os.unlink(f.name)

    def test_comment_lines(self):
        """Comment lines and blank lines are ignored."""
        with tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.txt') as f:
            f.write("# This is a comment\n")
            f.write("\n")
            f.write("host1\n")
            f.write("# Another comment\n")
            f.write("host2\n")
        try:
            result = load_deny_list(f.name)
            self.assertEqual(result, {'host1', 'host2'})
        finally:
            os.unlink(f.name)

    def test_case_insensitive(self):
        """Hostnames are stored in lowercase."""
        with tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.txt') as f:
            f.write("HOST1\n")
            f.write("Host2\n")
        try:
            result = load_deny_list(f.name)
            self.assertEqual(result, {'host1', 'host2'})
        finally:
            os.unlink(f.name)


class TestMainSubprocess(unittest.TestCase):
    """Tests for the main() function via subprocess."""

    def test_temp_folder_with_ip(self):
        """On a temp folder holding a.md with an IP: exit code 1 and the file name in stdout."""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create a.md with an IP address
            a_md_path = os.path.join(tmpdir, "a.md")
            with open(a_md_path, 'w', encoding='utf-8') as f:
                f.write("Connect to 192.168.1.50\n")

            # Create an empty deny list file
            deny_file = os.path.join(tmpdir, "deny.txt")
            with open(deny_file, 'w', encoding='utf-8') as f:
                pass

            # Run the script
            script_path = os.path.join(os.path.dirname(__file__), "privacy_check.py")
            env = os.environ.copy()
            # Set KOMPANION_SKILLS_LOCAL to tmpdir so it uses our deny file
            env["KOMPANION_SKILLS_LOCAL"] = tmpdir

            result = subprocess.run(
                [sys.executable, script_path, tmpdir, "--deny", deny_file],
                capture_output=True,
                text=True,
                env=env
            )

            self.assertEqual(result.returncode, 1)
            self.assertIn("a.md", result.stdout)

    def test_temp_folder_clean(self):
        """On a temp folder holding only a clean b.md: exit code 0."""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create a clean b.md
            b_md_path = os.path.join(tmpdir, "b.md")
            with open(b_md_path, 'w', encoding='utf-8') as f:
                f.write("Use git rebase, never force-push.\n")

            # Create an empty deny list file
            deny_file = os.path.join(tmpdir, "deny.txt")
            with open(deny_file, 'w', encoding='utf-8') as f:
                pass

            # Run the script
            script_path = os.path.join(os.path.dirname(__file__), "privacy_check.py")
            env = os.environ.copy()
            env["KOMPANION_SKILLS_LOCAL"] = tmpdir

            result = subprocess.run(
                [sys.executable, script_path, tmpdir, "--deny", deny_file],
                capture_output=True,
                text=True,
                env=env
            )

            self.assertEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
