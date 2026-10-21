#!/usr/bin/env python3
"""Build the redistributable OFL preview font; no system fonts are used."""
import hashlib
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
from urllib.request import urlopen

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "47d54ca1ebb14fd864695f1d39dc734f39017c33"
URL = f"https://raw.githubusercontent.com/google/fonts/{COMMIT}/ofl/notosanssc/NotoSansSC%5Bwght%5D.ttf"
SHA256 = "a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da"


def build(output: Path) -> None:
    characters = {chr(code) for code in range(32, 127)}
    for lead in range(0xA1, 0xF8):
        for trail in range(0xA1, 0xFF):
            characters.update(bytes((lead, trail)).decode("gb2312", errors="ignore"))
    for source in ("src/i18n.rs", "preview/src/desktop.rs"):
        characters.update((ROOT / source).read_text(encoding="utf-8"))
    with TemporaryDirectory(prefix="snip2pin-font-") as temporary:
        with urlopen(URL, timeout=120) as response:
            data = response.read()
        if hashlib.sha256(data).hexdigest() != SHA256:
            raise RuntimeError("upstream preview font SHA-256 mismatch")
        source = Path(temporary) / "upstream.ttf"
        source.write_bytes(data)
        font = TTFont(source, recalcTimestamp=False)
        # Subset before instancing to avoid processing unused CJK outlines.
        options = subset.Options()
        options.name_IDs = ["*"]
        options.name_languages = ["*"]
        options.name_legacy = True
        subsetter = subset.Subsetter(options=options)
        subsetter.populate(unicodes=sorted(map(ord, characters)))
        subsetter.subset(font)
        instantiateVariableFont(font, {"wght": 400}, inplace=True)
        family = "Snip2Pin Preview Sans"
        names = {
            1: family, 2: "Regular", 3: "Snip2PinPreviewSans-Regular",
            4: f"{family} Regular", 6: "Snip2PinPreviewSans-Regular",
            16: family, 17: "Regular", 21: family, 22: "Regular",
            25: "Snip2PinPreviewSans",
        }
        # Rename every language/platform record; retain copyright and OFL names.
        for record in font["name"].names:
            if record.nameID in names:
                record.string = names[record.nameID].encode(record.getEncoding())
        # Fixed OpenType timestamps correspond to the POSIX epoch.
        font["head"].created = font["head"].modified = 2082844800
        font.recalcTimestamp = False
        output.parent.mkdir(parents=True, exist_ok=True)
        font.save(output, reorderTables=True)
        print(f"{output}: {output.stat().st_size} bytes; {len(font.getGlyphOrder())} glyphs")
        font.close()


if __name__ == "__main__":
    if len(sys.argv) > 2:
        raise SystemExit("usage: build_font.py [output.ttf]")
    build(Path(sys.argv[1]) if len(sys.argv) == 2 else ROOT / "preview/pkg/font.ttf")
