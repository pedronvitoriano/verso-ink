#!/usr/bin/env bash
# Reduz os retratos gerados (PNG grandes) para o tamanho do Kindle: 216×216, cinza, em assets/treinadores.
# Uso: tools/rostos.sh pasta-com-os-png
set -euo pipefail
SRC="${1:?uso: tools/rostos.sh pasta}"
cd "$(dirname "$0")/.."
mkdir -p assets/treinadores
python3 -I - "$SRC" assets/treinadores <<'PY'
import sys, pathlib
from PIL import Image, ImageOps
src, out = map(pathlib.Path, sys.argv[1:3])
for p in sorted(src.glob("*.png")):
    im = ImageOps.exif_transpose(Image.open(p)).convert("L")
    side = min(im.size)
    im = im.crop(((im.width - side) // 2, (im.height - side) // 2, (im.width + side) // 2, (im.height + side) // 2))
    im = ImageOps.autocontrast(im, cutoff=1).resize((216, 216), Image.LANCZOS)
    im.save(out / p.name, optimize=True)
    print(out / p.name)
PY
