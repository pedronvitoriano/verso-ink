#!/usr/bin/env bash
# Prepara arte com transparência para assets/sprites (peças de xadrez, ladrilhos, Sokoban).
# Uso: tools/arte.sh pecas PASTA   (rei-b.png … peao-p.png → xadrez-rei-b.png …)
#      tools/arte.sh ladrilho ARQ.png NOME   (quadrado opaco que preenche a casa)
#      tools/arte.sh recorte ARQ.png NOME [LADO]  (objeto em fundo branco → fundo transparente)
# O fundo é o branco ligado à borda (o corpo branco de uma peça continua opaco).
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p assets/sprites
python3 -I - "$@" <<'PY'
import sys, pathlib
from PIL import Image, ImageOps, ImageFilter
mode = sys.argv[1]
out = pathlib.Path("assets/sprites")
PALETTE = [0x14, 0x4C, 0x88, 0xC4, 0xF4]
lut = [min(PALETTE, key=lambda p: abs(p - v)) for v in range(256)]

def cutout(src, side, box=None):
    im = ImageOps.autocontrast(Image.open(src).convert("L"), cutoff=1)
    w, h = im.size
    # Fundo: branco alcançável a partir da borda (preenchimento a partir dos quatro lados).
    mask = Image.new("L", (w + 2, h + 2), 0)
    pad = Image.new("L", (w + 2, h + 2), 255)
    pad.paste(im, (1, 1))
    from PIL import ImageDraw
    filled = pad.point(lambda v: 255 if v > 232 else 0)
    ImageDraw.floodfill(filled, (0, 0), 128)
    bg = filled.point(lambda v: 255 if v == 128 else 0).crop((1, 1, w + 1, h + 1))
    alpha = ImageOps.invert(bg).filter(ImageFilter.MinFilter(3)).filter(ImageFilter.GaussianBlur(1.2))
    box = box or alpha.point(lambda v: 255 if v > 40 else 0).getbbox() or (0, 0, w, h)
    x0, y0, x1, y1 = box
    s = int(max(x1 - x0, y1 - y0) * 1.04)
    cx, cy = (x0 + x1) // 2, (y0 + y1) // 2
    crop = (cx - s // 2, cy - s // 2, cx - s // 2 + s, cy - s // 2 + s)
    lum = Image.new("L", (s, s), 255); lum.paste(im.crop(crop), (0, 0))
    a = Image.new("L", (s, s), 0); a.paste(alpha.crop(crop), (0, 0))
    lum = lum.resize((side, side), Image.LANCZOS).point(lut)
    a = a.resize((side, side), Image.LANCZOS)
    return Image.merge("LA", (lum, a))

def tile(src, side):
    im = ImageOps.autocontrast(Image.open(src).convert("L"), cutoff=1)
    w, h = im.size
    m = min(w, h)
    im = im.crop(((w - m) // 2, (h - m) // 2, (w + m) // 2, (h + m) // 2)).resize((side, side), Image.LANCZOS).point(lut)
    return Image.merge("LA", (im, Image.new("L", (side, side), 255)))

if mode == "pecas":
    src = pathlib.Path(sys.argv[2])
    # Proporção clássica entre as peças (o rei é a maior), todas com a base na mesma linha.
    scale = {"rei": 1.0, "dama": 0.95, "bispo": 0.9, "cavalo": 0.86, "torre": 0.8, "peao": 0.72}
    for p in sorted(src.glob("*-[bp].png")):
        f = scale[p.stem.rsplit("-", 1)[0]]
        piece = cutout(p, int(160 * f))
        canvas = Image.new("LA", (160, 160), (255, 0))
        canvas.paste(piece, ((160 - piece.width) // 2, 160 - piece.height - 4))
        canvas.save(out / f"xadrez-{p.stem}.png", optimize=True)
        print("ok", p.stem)
elif mode == "ladrilho":
    tile(sys.argv[2], 176).save(out / f"{sys.argv[3]}.png", optimize=True)
elif mode == "recorte":
    cutout(sys.argv[2], int(sys.argv[4]) if len(sys.argv) > 4 else 160).save(out / f"{sys.argv[3]}.png", optimize=True)
else:
    sys.exit("modo: pecas | ladrilho | recorte")
PY
