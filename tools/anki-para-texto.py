#!/usr/bin/env python3
"""Converte um pacote do Anki em linhas frente<TAB>verso para o Verso Ink.

Uso: python3 tools/anki-para-texto.py baralho.apkg [baralho.txt]
Copie o arquivo produzido para data/flashcards/ no Kindle. Os dois primeiros
campos de cada nota são usados como frente e verso; imagens e áudio não entram.
"""

import argparse
import html
from html.parser import HTMLParser
from pathlib import Path
import re
import shutil
import sqlite3
import sys
import tempfile
import zipfile


class TextoHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.partes = []
        self.oculto = 0

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.oculto += 1
        elif not self.oculto and tag in ("br", "p", "div", "li", "tr"):
            self.partes.append(" ")

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self.oculto:
            self.oculto -= 1
        elif not self.oculto and tag in ("p", "div", "li", "tr"):
            self.partes.append(" ")

    def handle_data(self, data):
        if not self.oculto:
            self.partes.append(data)


def texto_campo(campo):
    """Remove HTML e áudio e mantém cada campo em uma única linha TSV."""
    campo = re.sub(r"\[sound:[^\]]*\]", " ", campo, flags=re.IGNORECASE)
    campo = re.sub(r"\{\{c\d+::(.*?)(?:::[^{}]*?)?\}\}", r"\1", campo, flags=re.IGNORECASE)
    parser = TextoHTML()
    parser.feed(campo)
    parser.close()
    # O parser já decodifica entidades; html.unescape também resolve entidades
    # duplamente codificadas, comuns em campos importados de outros programas.
    return " ".join(html.unescape("".join(parser.partes)).split())


def ler_notas(pacote):
    """Lê notas de collection.anki2 ou collection.anki21, sem extrair caminhos."""
    with zipfile.ZipFile(pacote) as arquivo:
        nomes = arquivo.namelist()
        membro = next((nome for nome in ("collection.anki21", "collection.anki2") if nome in nomes), None)
        if membro is None:
            raise ValueError("O pacote não contém collection.anki2 nem collection.anki21.")
        with tempfile.TemporaryDirectory(prefix="verso-anki-") as pasta:
            banco = Path(pasta) / "colecao.sqlite"
            with arquivo.open(membro) as origem, banco.open("wb") as destino:
                shutil.copyfileobj(origem, destino)
            with sqlite3.connect(banco.as_uri() + "?mode=ro", uri=True) as conexao:
                campos = conexao.execute("SELECT flds FROM notes ORDER BY id").fetchall()
    return [linha[0] for linha in campos]


def converter(pacote, destino):
    linhas = []
    ignoradas = 0
    for nota in ler_notas(pacote):
        campos = nota.split("\x1f")
        if len(campos) < 2:
            ignoradas += 1
            continue
        frente, verso = (texto_campo(campo) for campo in campos[:2])
        if not frente or not verso:
            ignoradas += 1
            continue
        linhas.append(frente + "\t" + verso)
    if not linhas:
        raise ValueError("Nenhuma nota tem frente e verso em texto.")
    destino.write_text("\n".join(linhas) + "\n", encoding="utf-8")
    return len(linhas), ignoradas


def main():
    cli = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    cli.add_argument("pacote", type=Path, help="arquivo .apkg exportado do Anki")
    cli.add_argument("destino", type=Path, nargs="?", help="arquivo .txt de saída (padrão: mesmo nome do pacote)")
    args = cli.parse_args()
    destino = args.destino or args.pacote.with_suffix(".txt")
    if args.pacote.resolve() == destino.resolve():
        cli.error("o arquivo de saída precisa ser diferente do pacote")
    try:
        count, ignoradas = converter(args.pacote, destino)
    except (OSError, ValueError, sqlite3.Error, zipfile.BadZipFile) as erro:
        print(f"Não foi possível converter: {erro}", file=sys.stderr)
        return 1
    print(f"{count} nota(s) em {destino}; {ignoradas} ignorada(s) sem frente ou verso.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
