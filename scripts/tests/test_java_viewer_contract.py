"""Verify the offline Java viewer's executable asset and CSP contract."""
import base64
import hashlib
from pathlib import Path
import re
import unittest


class JavaViewerContract(unittest.TestCase):
    def test_fr_ext_026_script_and_style_have_exact_csp_hashes(self):
        root = Path(__file__).resolve().parents[2]
        html = (root / "crates/noesis/assets/s8/java/index.html").read_text()
        for tag in ("script", "style"):
            text = html.split(f"<{tag}>", 1)[1].split(f"</{tag}>", 1)[0]
            digest = base64.b64encode(hashlib.sha256(text.encode()).digest()).decode()
            self.assertIn(f"{tag}-src 'sha256-{digest}'", html)
        self.assertNotRegex(html, r"\b(?:eval|fetch|XMLHttpRequest|WebSocket)\s*\(")
        self.assertNotIn("innerHTML", html)
        self.assertEqual(html.count("__JAVA_PAYLOAD__"), 1)
        self.assertEqual(re.findall(r'<script[^>]*\bsrc=', html), [])
        self.assertIn("connect-src 'none'", html)


if __name__ == "__main__":
    unittest.main()
