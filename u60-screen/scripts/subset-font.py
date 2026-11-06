#!/usr/bin/env python3
"""Regenerate the screen font: fonttools==4.60.1, source revision in assets/FONT.md."""
import argparse
import hashlib
from pathlib import Path
from fontTools import subset
from fontTools.ttLib import TTFont

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("source", type=Path)
parser.add_argument("--ui-only", action="store_true", help="small preview-only subset; not suitable for device content")
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
text = "".join(chr(n) for n in range(32, 127)) + "−—·↑↓↗‹›…你好"
for folder, extension in [("ui", "*.slint"), ("src", "*.rs"), ("assets", "*.json")]:
    for path in sorted((root / folder).rglob(extension)):
        text += path.read_text()
if hashlib.sha256(args.source.read_bytes()).hexdigest() != "a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da":
    parser.error("source font does not match assets/FONT.md")
font = TTFont(args.source, recalcTimestamp=False)
# Preserve the weight axis so the panel can use real medium/semibold outlines.
# Flattening everything to Regular made 11px labels particularly weak in RGB565.
options = subset.Options()
options.name_IDs = ["*"]
options.name_legacy = True
options.name_languages = ["*"]
options.recalc_timestamp = False
if args.ui_only:
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text=text)
    subsetter.subset(font)
for record in font["name"].names:
    replacement = {1: "U60 Sans", 2: "Regular", 3: "U60 Sans Regular 1.0", 4: "U60 Sans Regular", 6: "U60Sans-Regular", 16: "U60 Sans", 17: "Regular"}.get(record.nameID)
    if replacement:
        record.string = replacement.encode(record.getEncoding())
    elif record.nameID > 255:
        name = record.toUnicode().replace("Noto Sans SC", "U60 Sans").replace("NotoSansSC", "U60Sans")
        record.string = name.encode(record.getEncoding())
font.save(root / "assets/U60Sans.ttf")
print(f"Screen font: {len(font.getBestCmap())} characters; no firmware assets used")
