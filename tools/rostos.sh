#!/usr/bin/env bash
# Uniformiza e reduz os retratos dos treinadores para o Kindle (assets/treinadores, 216×216).
# Todos passam pelo mesmo tratamento, para parecerem do mesmo ilustrador:
#   1. mesmo enquadramento: quadro do mesmo tamanho para todos (72% da imagem), com o topo do
#      cabelo na mesma altura e centralizado no desenho;
#   2. mesma paleta: exatamente 4 tons (preto, cinza escuro, cinza claro, branco), sem degradê;
#   3. limpeza: manchas pequenas soltas somem (filtro de moda).
# Uso: tools/rostos.sh pasta-com-os-png [saida]
set -euo pipefail
SRC="${1:?uso: tools/rostos.sh pasta [saida]}"
cd "$(dirname "$0")/.."
OUT="${2:-assets/treinadores}"
mkdir -p "$OUT"
python3 -I - "$SRC" "$OUT" <<'PY'
import sys, pathlib
from PIL import Image, ImageOps, ImageFilter

src, out = map(pathlib.Path, sys.argv[1:3])
PALETTE = [0x14, 0x5C, 0xB4, 0xFF]   # mesmos 4 tons para todos
SIDE = 216

def posterize(im):
    lut = []
    for v in range(256):
        lut.append(min(PALETTE, key=lambda p: abs(p - v)))
    return im.point(lut)

for p in sorted(src.glob("*.png")):
    if p.name.startswith("ref-"):
        continue
    im = ImageOps.exif_transpose(Image.open(p)).convert("L")
    im = ImageOps.autocontrast(im, cutoff=1)
    # Contorno do desenho (tudo que não é fundo branco).
    mask = im.point(lambda v: 255 if v < 235 else 0)
    box = mask.getbbox() or (0, 0, im.width, im.height)
    x0, y0, x1, y1 = box
    # Quadro fixo (todos iguais), a partir do topo do cabelo, centrado no desenho.
    side = int(min(im.size) * 0.72)
    cx = (x0 + x1) // 2
    top = max(0, y0 - side // 20)
    crop = Image.new("L", (side, side), 255)
    crop.paste(im.crop((cx - side // 2, top, cx - side // 2 + side, top + side)), (0, 0))
    small = crop.resize((SIDE * 2, SIDE * 2), Image.LANCZOS)
    small = small.filter(ImageFilter.ModeFilter(5))
    small = posterize(small).resize((SIDE, SIDE), Image.LANCZOS)
    posterize(small).save(out / p.name, optimize=True)
    print(out / p.name)
PY
