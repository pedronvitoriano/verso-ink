#!/usr/bin/env python3
"""Converte os dicionários livres para o formato do Verso Ink: uma linha por verbete,
"chave<TAB>palavra<TAB>definição", ordenada pela chave (minúsculas, sem acento), para busca binária.

Uso: tools/dicionarios.py PASTA_XMLS_DICIONARIO_ABERTO ARQ_ENG_POR.tei SAIDA
Fontes: Dicionário Aberto (domínio público, github.com/ambs/Dicionario-Aberto, Resources/xmls.zip)
e FreeDict eng-por 0.3 (GPL, download.freedict.org).
"""
import os, re, sys, unicodedata
import xml.etree.ElementTree as ET

def key(w):
    w = unicodedata.normalize("NFD", w.lower())
    return "".join(c for c in w if unicodedata.category(c) != "Mn").strip()

def clean(t):
    t = re.sub(r"_([^_]+)_", r"\1", t or "")
    return re.sub(r"\s+", " ", t).strip()

def pt(folder):
    out = {}
    for f in sorted(os.listdir(folder)):
        if not f.endswith(".xml"):
            continue
        root = ET.fromstring(open(os.path.join(folder, f), encoding="utf-8").read())
        for e in root.iter("entry"):
            orth = clean(e.findtext("form/orth") or e.get("id", ""))
            senses = []
            for s in e.iter("sense"):
                g = clean(s.findtext("gramGrp"))
                d = clean(s.findtext("def"))
                if d:
                    senses.append((f"({g}) " if g else "") + d)
            if orth and senses:
                k = key(re.sub(r":\d+$", "", orth))
                out.setdefault(k, (orth, []))[1].extend(senses[:4])
    return {k: (w, " · ".join(f"{i+1}. {s}" for i, s in enumerate(v[:4]))) for k, (w, v) in out.items()}

def en(tei):
    ns = {"t": "http://www.tei-c.org/ns/1.0"}
    root = ET.parse(tei).getroot()
    out = {}
    for e in root.iter("{http://www.tei-c.org/ns/1.0}entry"):
        orth = clean(e.findtext("t:form/t:orth", namespaces=ns))
        pos = clean(e.findtext(".//t:gramGrp/t:pos", namespaces=ns))
        quotes = [clean(q.text) for q in e.iterfind(".//t:cit[@type='trans']/t:quote", ns) if q.text]
        if orth and quotes:
            k = key(orth)
            w, prev = out.get(k, (orth, []))
            prev.append((f"({pos}) " if pos else "") + ", ".join(dict.fromkeys(quotes)))
            out[k] = (w, prev)
    return {k: (w, " · ".join(v[:4])) for k, (w, v) in out.items()}

def write(d, path):
    with open(path, "w", encoding="utf-8") as f:
        for k in sorted(d):
            w, txt = d[k]
            f.write(f"{k}\t{w}\t{txt.replace(chr(9), ' ')}\n")

if __name__ == "__main__":
    xmls, tei, outdir = sys.argv[1:4]
    os.makedirs(outdir, exist_ok=True)
    a = pt(xmls); write(a, os.path.join(outdir, "pt.tsv"))
    b = en(tei); write(b, os.path.join(outdir, "en-pt.tsv"))
    print(len(a), "verbetes pt;", len(b), "verbetes en→pt")
