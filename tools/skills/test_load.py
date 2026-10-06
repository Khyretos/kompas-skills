import os, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import load

def card(root, rel, header, body):
    path = os.path.join(root, rel + ".md")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w").write(("---\n" + header + "\n---\n" if header else "") + body + "\n")

class SelectTest(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp()
        card(self.root, "work-habits", "", "HABITS")
        card(self.root, "shared/SKILL", "name: shared", "SHARED")
        card(self.root, "worker/web/SKILL", "name: worker/web", "WEB CORE")
        card(self.root, "worker/rust/SKILL", "name: worker/rust", "RUST CORE")
        card(self.root, "shared/colour-themes", 'roles: [worker, reviewer]\ntags: [theme, css]\npaths: ["**/*.css"]', "COLOURS")
        card(self.root, "worker/python", 'roles: [worker]\ntags: [python]\npaths: ["**/*.py"]', "PYTHON")
        card(self.root, "orchestrator/prompting", "roles: [orchestrator]\ntags: [prompt]", "PROMPTING")
        card(self.root, "shared/big", "roles: [worker]\ntags: [theme]", "X" * 40000)
        card(self.root, "_model-notes/qwen3/SKILL", "models: [qwen3]", "QWEN NOTES")
        card(self.root, "_model-notes/gemma4/SKILL", "models: [gemma4]", "GEMMA NOTES")

    def test_css_job_gets_the_theme_card_not_rust(self):
        names, text = load.select("worker/web", "restyle the buttons", ["web/src/styles.css"], notes="qwen3", root=self.root)
        self.assertEqual(names[:3], ["work-habits", "shared/SKILL", "worker/web/SKILL"])
        self.assertIn("shared/colour-themes", names)
        self.assertNotIn("worker/rust/SKILL", names)
        self.assertIn("QWEN NOTES", text)
        self.assertNotIn("GEMMA NOTES", text)

    def test_budget_leaves_out_what_does_not_fit(self):
        names, _ = load.select("worker/web", "a theme change", ["web/a.css"], budget_tokens=1500, root=self.root)
        self.assertIn("shared/colour-themes", names)
        self.assertNotIn("shared/big", names)

    def test_cards_of_other_roles_are_not_picked(self):
        names, _ = load.select("worker/web", "build a prompt", [], root=self.root)
        self.assertNotIn("orchestrator/prompting", names)

    def test_area_cards_stay_in_their_area(self):
        card(self.root, "worker/rust/api", 'roles: [worker, reviewer]\ntags: [header]', "RUST API")
        rust_names, _ = load.select("worker/rust", "fix the header", [], root=self.root)
        web_names, _ = load.select("worker/web", "fix the header", [], root=self.root)
        self.assertIn("worker/rust/api", rust_names)
        self.assertNotIn("worker/rust/api", web_names)

    def test_named_cards_are_always_included_and_unknown_ones_refused(self):
        names, _ = load.select("worker/web", "", [], extra=["worker/python"], root=self.root)
        self.assertIn("worker/python", names)
        with self.assertRaises(ValueError):
            load.select("worker/web", "", [], extra=["nope"], root=self.root)

    def test_headers_are_not_in_the_text(self):
        _, text = load.select("worker/web", "css", ["x.css"], root=self.root)
        self.assertNotIn("roles:", text)

if __name__ == "__main__":
    unittest.main()
