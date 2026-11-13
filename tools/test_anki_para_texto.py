import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest
import zipfile


spec = importlib.util.spec_from_file_location("anki_para_texto", Path(__file__).with_name("anki-para-texto.py"))
anki = importlib.util.module_from_spec(spec)
spec.loader.exec_module(anki)


class ConversaoAnki(unittest.TestCase):
    def test_html_entidades_midias_e_linhas(self):
        self.assertEqual(anki.texto_campo('<div>Olá&nbsp;<b>mundo</b></div><br>linha\nseguinte [sound:a.mp3]<style>x</style>'), "Olá mundo linha seguinte")
        self.assertEqual(anki.texto_campo("{{c1::Brasília::capital}}"), "Brasília")

    def test_pacote_sqlite_para_tsv(self):
        with tempfile.TemporaryDirectory() as pasta:
            pasta = Path(pasta)
            banco = pasta / "colecao"
            with sqlite3.connect(banco) as db:
                db.execute("CREATE TABLE notes (id INTEGER PRIMARY KEY, flds TEXT NOT NULL)")
                db.executemany("INSERT INTO notes VALUES (?, ?)", [(1, "<b>Frente</b>\x1fVerso<br>texto\x1fExtra"), (2, "Sem verso"), (3, "Texto\x1f<img src='a.png'>")])
            for nome in ("collection.anki2", "collection.anki21"):
                pacote = pasta / (nome + ".apkg")
                with zipfile.ZipFile(pacote, "w") as z:
                    z.write(banco, nome)
                destino = pasta / (nome + ".txt")
                self.assertEqual(anki.converter(pacote, destino), (1, 2))
                self.assertEqual(destino.read_text(encoding="utf-8"), "Frente\tVerso texto\n")

    def test_pacote_sem_banco(self):
        with tempfile.TemporaryDirectory() as pasta:
            pacote = Path(pasta) / "vazio.apkg"
            with zipfile.ZipFile(pacote, "w"):
                pass
            with self.assertRaisesRegex(ValueError, "não contém"):
                anki.ler_notas(pacote)


if __name__ == "__main__":
    unittest.main()
