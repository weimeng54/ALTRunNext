# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

"""Keep application popup menus inside the shared Rust UI component."""
from pathlib import Path
import re
import sys


def main():
    src = Path(__file__).resolve().parents[1] / "src"
    popup = src / "ui" / "menu_style"
    api = re.compile(r"\b(?:CreatePopupMenu|TrackPopupMenu(?:Ex)?)\s*\(")
    problems = []
    for path in src.rglob("*.rs"):
        if path == popup.with_suffix(".rs") or popup in path.parents:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if api.search(line.partition("//")[0]):
                problems.append(f"{path.relative_to(src)}:{number}: use ui::menu_style")
    if problems:
        print("\n".join(problems), file=sys.stderr)
    return bool(problems)


if __name__ == "__main__":
    sys.exit(main())
