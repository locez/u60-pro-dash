#!/usr/bin/env python3
"""Build the supplemental OFL Hangul font; see assets/FONT.md for the source."""
import argparse
import hashlib
from pathlib import Path
from fontTools import subset
from fontTools.ttLib import TTFont

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
args = parser.parse_args()
if hashlib.sha256(args.source.read_bytes()).hexdigest() != '194018e6b2b293a7964f037b25c0249ce1418bc9ab3c971060a03aa57861e252':
    parser.error('source font does not match assets/FONT.md')
font = TTFont(args.source, recalcTimestamp=False)
options = subset.Options()
options.name_IDs = ['*']
options.name_legacy = True
options.name_languages = ['*']
options.recalc_timestamp = False
subsetter = subset.Subsetter(options=options)
# All modern Hangul syllables plus Jamo, not only currently translated copy.
ranges = [(0x20, 0x7f), (0x1100, 0x1200), (0x3130, 0x3190), (0xa960, 0xa980), (0xac00, 0xd800), (0x2000, 0x2070), (0x3000, 0x3040)]
subsetter.populate(unicodes=[point for start, end in ranges for point in range(start, end)])
subsetter.subset(font)
for record in font['name'].names:
    if record.nameID in (1, 2, 3, 4, 6, 16, 17) or record.nameID > 255:
        name = record.toUnicode().replace('Noto Sans KR', 'U60 Hangul').replace('NotoSansKR', 'U60Hangul')
        record.string = name.encode(record.getEncoding())
font.save(Path(__file__).resolve().parents[1] / 'assets/U60Hangul.ttf')
print(f'Supplemental font: {len(font.getBestCmap())} characters')
