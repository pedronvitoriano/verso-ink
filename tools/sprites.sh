#!/usr/bin/env bash
# Trata os sprites gerados (Masmorra e Cripta) para o Kindle: corpo inteiro centrado no quadrado,
# a mesma paleta de 4 tons dos treinadores, 128×128 em assets/sprites.
# Uso: tools/sprites.sh pasta-com-os-png
set -euo pipefail
SRC="${1:?uso: tools/sprites.sh pasta}"
cd "$(dirname "$0")/.."
mkdir -p assets/sprites
python3 -I - "$SRC" assets/sprites <<'PY'
import sys, pathlib
from PIL import Image, ImageOps, ImageFilter
src, out = map(pathlib.Path, sys.argv[1:3])
PALETTE = [0x14, 0x5C, 0xB4, 0xFF]
lut = [min(PALETTE, key=lambda p: abs(p - v)) for v in range(256)]
for p in sorted(src.glob("*.png")):
    if p.name.startswith("ref-"):
        continue
    im = ImageOps.autocontrast(Image.open(p).convert("L"), cutoff=1)
    box = im.point(lambda v: 255 if v < 235 else 0).getbbox() or (0, 0, im.width, im.height)
    x0, y0, x1, y1 = box
    side = int(max(x1 - x0, y1 - y0) * 1.08)
    cx, cy = (x0 + x1) // 2, (y0 + y1) // 2
    sq = Image.new("L", (side, side), 255)
    sq.paste(im.crop((cx - side // 2, cy - side // 2, cx - side // 2 + side, cy - side // 2 + side)), (0, 0))
    small = sq.resize((256, 256), Image.LANCZOS).filter(ImageFilter.ModeFilter(3)).point(lut)
    small.resize((128, 128), Image.LANCZOS).point(lut).save(out / p.name, optimize=True)
    print(out / p.name)
PY
