#!/usr/bin/env python3
"""Contratos da ponte do Verso sem chamar Claude, Codex ou a API de imagens."""

import base64
import importlib.util
import json
import os
import shutil
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
BOOT = tempfile.TemporaryDirectory(prefix="verso-bridge-import-")
TOKEN = Path(BOOT.name) / "token"
TOKEN.write_text("teste-token")
os.environ["VERSO_INK_TOKEN_FILE"] = str(TOKEN)
os.environ["VERSO_INK_DATA"] = str(Path(BOOT.name) / "chats")
os.environ["VERSO_INK_PROVIDER_ORDER"] = "claude,codex"
sys.path.insert(0, str(ROOT))
SPEC = importlib.util.spec_from_file_location("verso_bridge_test", ROOT / "claude-bridge.py")
bridge = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(bridge)


class BridgeTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="verso-bridge-test-"))
        bridge.DATA = self.root / "chats"
        bridge.DATA.mkdir()
        bridge.ATTACH = bridge.DATA / "_anexos"
        bridge.CHAT_LOCKS.clear()
        os.environ["VERSO_INK_PROVIDER_ORDER"] = "claude,codex"

    def tearDown(self):
        shutil.rmtree(self.root)

    def test_transient_claude_failure_falls_back_to_codex(self):
        unavailable = {"ok": False, "provider": "claude", "error": "limite", "retryable": True}
        answer = {"ok": True, "provider": "codex", "text": "Continuo daqui.", "session": "thread-codex"}
        with patch.object(bridge, "run_claude_provider", return_value=unavailable) as claude, patch.object(bridge, "run_codex_provider", return_value=answer) as codex:
            result = bridge.run_verso("pedido", cwd=self.root, preferred="claude")

        self.assertTrue(result["ok"])
        self.assertEqual(result["provider"], "codex")
        self.assertEqual(result["fallback_from"], "claude")
        claude.assert_called_once()
        codex.assert_called_once()

    def test_permanent_failure_does_not_try_the_other_provider(self):
        refused = {"ok": False, "provider": "claude", "error": "pedido recusado", "retryable": False}
        with patch.object(bridge, "run_claude_provider", return_value=refused), patch.object(bridge, "run_codex_provider") as codex:
            result = bridge.run_verso("pedido", cwd=self.root, preferred="claude")

        self.assertFalse(result["ok"])
        codex.assert_not_called()

    def test_transient_codex_failure_falls_back_to_claude(self):
        unavailable = {"ok": False, "provider": "codex", "error": "timeout", "retryable": True}
        answer = {"ok": True, "provider": "claude", "text": "O Claude continuou.", "session": "session-claude"}
        with patch.object(bridge, "run_codex_provider", return_value=unavailable) as codex, patch.object(bridge, "run_claude_provider", return_value=answer) as claude:
            result = bridge.run_verso("pedido", cwd=self.root, preferred="codex")

        self.assertTrue(result["ok"])
        self.assertEqual(result["provider"], "claude")
        self.assertEqual(result["fallback_from"], "codex")
        codex.assert_called_once()
        claude.assert_called_once()

    def test_shared_history_and_brain_are_sent_to_the_next_provider(self):
        state = bridge.DATA.parent / "verso-ink-estado"
        state.mkdir()
        (state / "brain.json").write_text(json.dumps({"schema": "aggregate-v1", "text": "Resumo seguro: priorize leitura."}))
        answers = [
            {"ok": True, "provider": "claude", "text": "Primeira resposta.", "fallback_from": None},
            {"ok": True, "provider": "codex", "text": "Segunda resposta.", "fallback_from": "claude"},
        ]
        with patch.object(bridge, "run_verso", side_effect=answers) as run:
            status, first = bridge.handle_chat([], json.dumps({"text": "Primeira pergunta", "brain": True}))
            self.assertEqual(status, 200)
            status, second = bridge.handle_chat(["chats", first["id"]], json.dumps({"text": "Continue daqui", "brain": True}))

        self.assertEqual(status, 200)
        self.assertEqual(second["fallback_from"], "claude")
        prompt = run.call_args_list[1].args[0]
        self.assertIn("Pedro: Primeira pergunta", prompt)
        self.assertIn("Verso (claude): Primeira resposta.", prompt)
        self.assertIn("Resumo seguro: priorize leitura.", prompt)
        chat = bridge.load(first["id"])
        self.assertTrue(chat["messages"][0]["brain"])
        self.assertEqual(chat["messages"][1]["brain_sources"], ["Resumo seguro do Brain sincronizado do PC"])
        self.assertEqual(chat["messages"][-1]["fallback_from"], "claude")

    def test_image_mode_persists_a_typed_file_without_calling_a_provider(self):
        state = bridge.DATA.parent / "verso-ink-estado"
        state.mkdir()
        (state / "brain.json").write_text(json.dumps({"schema": "aggregate-v1", "text": "Referência visual segura."}))

        def create(prompt, cid):
            self.assertIn("Referência visual segura.", prompt)
            self.assertIn("Uma estante do capítulo atual.", prompt)
            name = "imagem-teste.png"
            (bridge.workdir(cid) / name).write_bytes(base64.b64decode("iVBORw0KGgo="))
            return name, None

        with patch.object(bridge, "generate_image", side_effect=create), patch.object(bridge, "run_verso") as run:
            status, response = bridge.handle_chat([], json.dumps({"text": "Uma estante de livros", "context": "Uma estante do capítulo atual.", "brain": True, "mode": "image"}))

        self.assertEqual(status, 200)
        self.assertEqual(response["provider"], "imagem")
        self.assertEqual(response["files"][0]["kind"], "image")
        run.assert_not_called()
        chat = bridge.load(response["id"])
        self.assertEqual(chat["messages"][-1]["media"][0]["mime"], "image/png")

    def test_image_mode_rejects_attachments_before_copying_them(self):
        attachment = "0123456789ab__foto.png"
        with patch.object(bridge, "safe_attachment_names") as copied:
            status, response = bridge.handle_chat([], json.dumps({"text": "Uma foto", "mode": "image", "anexos": [attachment]}))

        self.assertEqual(status, 400)
        self.assertIn("anexos", response["error"])
        copied.assert_not_called()

    def test_image_api_result_is_checked_before_it_is_saved(self):
        png = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLkGQAAAABJRU5ErkJggg==")
        seen = {}

        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read(self, _):
                return json.dumps({"data": [{"b64_json": base64.b64encode(png).decode()}]}).encode()

        def urlopen(request, timeout):
            seen.update(json.loads(request.data))
            self.assertEqual(timeout, 180)
            return Response()

        with patch.object(bridge, "image_key", return_value="secret-kept-out-of-chat"), patch.object(bridge.urllib.request, "urlopen", side_effect=urlopen):
            name, error = bridge.generate_image("desenhe uma capa", "0123456789abcdef")

        self.assertIsNone(error)
        self.assertEqual(seen["size"], "1024x1536")
        self.assertEqual((bridge.workdir("0123456789abcdef") / name).read_bytes(), png)

    def test_image_api_result_rejects_excessive_dimensions(self):
        png_header = b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", 9000, 9000)

        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read(self, _):
                return json.dumps({"data": [{"b64_json": base64.b64encode(png_header).decode()}]}).encode()

        with patch.object(bridge, "image_key", return_value="secret-kept-out-of-chat"), patch.object(bridge.urllib.request, "urlopen", return_value=Response()):
            name, error = bridge.generate_image("desenhe", "0123456789abcdef")

        self.assertIsNone(name)
        self.assertIn("grande demais", error)

    def test_brain_summary_accepts_only_bounded_counts(self):
        summary = bridge.brain_summary("2", 3)
        self.assertIn("2 retomada(s)", summary)
        self.assertIn("3 adequações", summary)
        self.assertEqual(bridge.brain_summary("segredo", -4), "- Nenhuma pendência agregada do Brain.")
        self.assertIn("999", bridge.brain_summary(10_000, 0))

    def test_legacy_brain_text_is_not_exposed_to_a_chat(self):
        state = bridge.DATA.parent / "verso-ink-estado"
        state.mkdir()
        (state / "brain.json").write_text(json.dumps({"text": "conteúdo antigo e privado"}))
        self.assertEqual(bridge.brain_snapshot(), ("", []))

    def test_shared_history_keeps_file_references_for_the_other_provider(self):
        history = bridge.history_text(
            {"messages": [{"role": "user", "text": "Veja este arquivo", "anexos": [{"name": "notas.txt"}]}]}
        )
        self.assertIn("arquivos desta conversa: notas.txt", history)

    def test_codex_invocation_is_ephemeral_and_scoped_to_the_chat_folder(self):
        cwd = self.root / "conversation"
        seen = []

        def execute(command, **kwargs):
            seen.append(command)
            output = Path(command[command.index("--output-last-message") + 1])
            output.write_text("Resposta do Codex.")
            return SimpleNamespace(returncode=0, stdout="", stderr="")

        with patch.object(bridge, "command_ready", return_value=True), patch.object(bridge.subprocess, "run", side_effect=execute):
            result = bridge.run_codex_provider("pedido", cwd=cwd)

        self.assertTrue(result["ok"])
        self.assertEqual(result["text"], "Resposta do Codex.")
        # Extras pode iniciar uma atualização assíncrona ao importar a ponte.
        # Confirme especificamente a chamada do Codex, não a última chamada de
        # subprocess observada pela suíte.
        command = next(command for command in seen if command[:2] == [bridge.CODEX, "exec"])
        self.assertIn("--ephemeral", command)
        self.assertIn("--ignore-user-config", command)
        self.assertIn("--ignore-rules", command)
        self.assertEqual(command[command.index("--cd") + 1], str(cwd))



class BackupEndpointTest(unittest.TestCase):
    def setUp(self):
        import io
        import tarfile
        self.io, self.tarfile = io, tarfile
        self.temp = tempfile.TemporaryDirectory(prefix="verso-backup-")
        self.addCleanup(self.temp.cleanup)
        old_data = bridge.DATA
        self.addCleanup(setattr, bridge, "DATA", old_data)
        bridge.DATA = Path(self.temp.name) / "chats"
        bridge.DATA.mkdir()

    def archive(self, name="data/plano.txt", kind=None):
        output = self.io.BytesIO()
        with self.tarfile.open(fileobj=output, mode="w:gz") as tar:
            info = self.tarfile.TarInfo(name)
            if kind is not None:
                info.type = kind
                info.linkname = "../../fora"
                tar.addfile(info)
            else:
                body = b"conteudo"
                info.size = len(body)
                tar.addfile(info, self.io.BytesIO(body))
        return output.getvalue()

    def request(self, method, path, body=b"", headers=None):
        handler = bridge.Handler.__new__(bridge.Handler)
        lines = [f"{method} {path} HTTP/1.1", "Host: localhost"]
        for key, value in (headers or {}).items():
            lines.append(f"{key}: {value}")
        request = ("\r\n".join(lines) + "\r\n\r\n").encode() + body
        handler.rfile = self.io.BytesIO(request)
        handler.wfile = self.io.BytesIO()
        handler.client_address = ("127.0.0.1", 1234)
        handler.close_connection = True
        handler.handle_one_request()
        raw = handler.wfile.getvalue()
        head, content = raw.split(b"\r\n\r\n", 1)
        return int(head.split(b" ", 2)[1]), content

    def post(self, body, length=None, token="teste-token"):
        return self.request("POST", "/backup", body, {
            "Authorization": "Bearer " + token,
            "Content-Length": str(len(body) if length is None else length),
        })

    def test_backup_upload_list_and_download_are_authenticated(self):
        data = self.archive()
        self.assertEqual(self.post(data, token="errado")[0], 401)
        self.assertEqual(self.request("GET", "/backup")[0], 401)
        status, body = self.post(data)
        self.assertEqual(status, 200)
        name = json.loads(body)["nome"]
        headers = {"Authorization": "Bearer teste-token"}
        status, body = self.request("GET", "/backup", headers=headers)
        listing = json.loads(body)
        self.assertEqual(status, 200)
        self.assertEqual(listing[0]["nome"], name)
        self.assertEqual(listing[0]["tamanho"], len(data))
        self.assertRegex(listing[0]["data"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d$")
        self.assertEqual(self.request("GET", "/backup/" + name, headers=headers), (200, data))
        # A cópia baixada pode ser aberta e lida, sem extrair para o disco.
        with self.tarfile.open(fileobj=self.io.BytesIO(data), mode="r:gz") as archive:
            self.assertEqual(archive.extractfile("data/plano.txt").read(), b"conteudo")
        self.assertEqual(self.request("GET", "/backup/%2e%2e%2fsegredo", headers=headers)[0], 404)
        self.assertEqual(self.request("GET", "/backup/20200101-000000.tar.gz", headers=headers)[0], 404)
        self.assertEqual(self.request("GET", "/backup/" + name)[0], 401)

    def test_rejects_oversized_invalid_and_incomplete_bodies(self):
        data = self.archive()
        self.assertEqual(self.post(data, bridge.BACKUP_LIMIT + 1)[0], 413)
        self.assertEqual(self.post(data, -1)[0], 400)
        self.assertEqual(self.post(data, "abc")[0], 400)
        self.assertEqual(self.post(data, len(data) + 10)[0], 400)
        self.assertEqual(self.post(b"arquivo invalido")[0], 400)
        self.assertEqual(self.post(data[:-6])[0], 400)
        self.assertEqual(bridge.backup_list(), [])

    def test_rejects_absolute_traversal_and_links(self):
        for name in ("../segredo", "data/../../segredo", "/etc/token", "C:/token", "data\\..\\token"):
            with self.subTest(name=name):
                self.assertEqual(self.post(self.archive(name))[0], 400)
        for kind in (self.tarfile.SYMTYPE, self.tarfile.LNKTYPE):
            with self.subTest(kind=kind):
                self.assertEqual(self.post(self.archive("data/link", kind))[0], 400)

    def test_keeps_fourteen_newest_and_rejects_same_second_collision(self):
        directory = bridge.backup_dir()
        directory.mkdir()
        for day in range(1, 16):
            (directory / f"202001{day:02d}-000000.tar.gz").write_bytes(b"old")
        data = self.archive()
        from datetime import datetime

        class FixedDatetime(datetime):
            @classmethod
            def now(cls, tz=None):
                return cls(2030, 1, 1, tzinfo=tz)

        with patch.object(bridge.datetime, "datetime", FixedDatetime):
            self.assertEqual(self.post(data)[0], 200)
            names = [item["nome"] for item in bridge.backup_list()]
            self.assertEqual(len(names), 14)
            self.assertNotIn("20200101-000000.tar.gz", names)
            self.assertNotIn("20200102-000000.tar.gz", names)
            self.assertEqual(self.post(data)[0], 409)

    def test_download_does_not_follow_symlinks(self):
        directory = bridge.backup_dir()
        directory.mkdir()
        (directory / "20200101-000000.tar.gz").symlink_to(TOKEN)
        headers = {"Authorization": "Bearer teste-token"}
        self.assertEqual(self.request("GET", "/backup/20200101-000000.tar.gz", headers=headers)[0], 404)
        self.assertEqual(json.loads(self.request("GET", "/backup", headers=headers)[1]), [])



class CotacoesTest(unittest.TestCase):
    def setUp(self):
        from verso_cotacoes import Cotacoes
        self.cotacoes = Cotacoes()
        self.payload = {moeda + "BRL": {"bid": str(valor)} for moeda, valor in (
            ("USD", 5.43), ("EUR", 6.12), ("GBP", 7.01), ("ARS", 0.005), ("BTC", 324000)
        )}

    def response(self, payload):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read(self, limit):
                return json.dumps(payload).encode()[:limit]

        return Response()

    def test_cotacoes_fetch_and_one_hour_cache(self):
        with patch("verso_cotacoes.time.monotonic", side_effect=[100, 101]), patch(
            "verso_cotacoes.urllib.request.urlopen", return_value=self.response(self.payload)
        ) as fetch:
            status, first = self.cotacoes.obter()
            status2, second = self.cotacoes.obter()
        self.assertEqual((status, status2), (200, 200))
        self.assertEqual(first, second)
        self.assertEqual(first["USD"], 5.43)
        self.assertEqual(first["BTC"], 324000)
        self.assertIn("quando", first)
        self.assertNotIn("antigo", first)
        self.assertEqual(fetch.call_count, 1)
        self.assertEqual(fetch.call_args.kwargs["timeout"], 15)

    def test_cotacoes_returns_stale_cache_after_failure(self):
        with patch("verso_cotacoes.time.monotonic", side_effect=[100, 3701]), patch(
            "verso_cotacoes.urllib.request.urlopen", side_effect=[self.response(self.payload), OSError("fora")]
        ):
            _, first = self.cotacoes.obter()
            status, stale = self.cotacoes.obter()
        self.assertEqual(status, 200)
        self.assertTrue(stale.pop("antigo"))
        self.assertEqual(stale, first)

    def test_cotacoes_failure_without_cache_and_invalid_values(self):
        self.payload["USDBRL"]["bid"] = "nan"
        with patch("verso_cotacoes.urllib.request.urlopen", return_value=self.response(self.payload)):
            status, body = self.cotacoes.obter()
        self.assertEqual(status, 502)
        self.assertEqual(body, {"error": "não foi possível obter as cotações"})

    def test_cotacoes_route_is_authenticated(self):
        fake = SimpleNamespace(path_parts=lambda: ["cotacoes"], authorized=lambda: True,
                               reply=lambda status, body: (status, body))
        with patch.object(bridge.COTACOES, "obter", return_value=(200, {"USD": 5.43})) as obter:
            self.assertEqual(bridge.Handler.do_GET(fake), (200, {"USD": 5.43}))
            obter.assert_called_once()
        fake.authorized = lambda: False
        with patch.object(bridge.COTACOES, "obter") as obter:
            self.assertIsNone(bridge.Handler.do_GET(fake))
            obter.assert_not_called()

    def test_cotacoes_limits_remote_response(self):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read(self, limit):
                return b"x" * limit

        with patch("verso_cotacoes.urllib.request.urlopen", return_value=Response()):
            status, body = self.cotacoes.obter()
        self.assertEqual(status, 502)
        self.assertIn("error", body)

    def test_cotacoes_http_contract(self):
        import io

        def request(token):
            handler = bridge.Handler.__new__(bridge.Handler)
            handler.rfile = io.BytesIO(
                f"GET /cotacoes HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n".encode()
            )
            handler.wfile = io.BytesIO()
            handler.client_address = ("127.0.0.1", 12345)
            handler.close_connection = True
            handler.handle_one_request()
            headers, body = handler.wfile.getvalue().split(b"\r\n\r\n", 1)
            return headers, json.loads(body)

        with patch.object(bridge.COTACOES, "obter", return_value=(200, {"USD": 5.43})) as obter:
            headers, body = request("teste-token")
            self.assertIn(b"200 OK", headers)
            self.assertEqual(body, {"USD": 5.43})
            obter.assert_called_once()
            headers, body = request("token-errado")
            self.assertIn(b"401 Unauthorized", headers)
            self.assertIn("error", body)
            obter.assert_called_once()


class AgendaTest(unittest.TestCase):
    def setUp(self):
        import threading
        import verso_extras
        self.mod = verso_extras
        self.directory = tempfile.TemporaryDirectory()
        self.agenda_file = Path(self.directory.name) / "ical-url"
        self.env = patch.dict(os.environ, {"VERSO_INK_AGENDA_FILE": str(self.agenda_file)})
        self.env.start()
        self.extras = verso_extras.Extras.__new__(verso_extras.Extras)
        self.extras.agenda_lock = threading.Lock()
        self.extras.agenda_cache = None

    def tearDown(self):
        self.env.stop()
        self.directory.cleanup()

    def test_agenda_without_configuration(self):
        self.assertEqual(self.extras.agenda(), ({"configured": False}, None))

    def test_agenda_http_authentication_and_error_do_not_expose_url(self):
        import io

        def request(token):
            handler = bridge.Handler.__new__(bridge.Handler)
            handler.rfile = io.BytesIO(
                f"GET /agenda HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n".encode()
            )
            handler.wfile = io.BytesIO()
            handler.client_address = ("127.0.0.1", 1234)
            handler.close_connection = True
            handler.handle_one_request()
            headers, body = handler.wfile.getvalue().split(b"\r\n\r\n", 1)
            return int(headers.split(b" ", 2)[1]), json.loads(body)

        with patch.object(bridge.EXTRAS, "agenda", return_value=({"configured": False}, None)) as agenda:
            self.assertEqual(request("errado")[0], 401)
            agenda.assert_not_called()
            self.assertEqual(request("teste-token"), (200, {"configured": False}))
        with patch.object(bridge.EXTRAS, "agenda", return_value=(None, "https://secret.test/private")):
            status, body = request("teste-token")
            self.assertEqual(status, 502)
            self.assertNotIn("secret", json.dumps(body))

    def test_ical_timezone_all_day_folding_and_crossing_midnight(self):
        from datetime import date
        ical = """BEGIN:VCALENDAR
BEGIN:VEVENT
DTSTART:20261113T020000Z
DTEND:20261113T033000Z
SUMMARY:Viagem\\, à noite
LOCATION:São
  Paulo
END:VEVENT
BEGIN:VEVENT
DTSTART;VALUE=DATE:20261113
DTEND;VALUE=DATE:20261115
SUMMARY:Dia inteiro
END:VEVENT
END:VCALENDAR"""
        events = self.mod._ical_events(ical, date(2026, 11, 12))
        self.assertEqual(events, [
            {"inicio": "23:00", "fim": "00:30", "titulo": "Viagem, à noite", "local": "São Paulo", "dia": "hoje"},
            {"inicio": "dia todo", "fim": "dia todo", "titulo": "Dia inteiro", "local": "", "dia": "amanha"},
            {"inicio": "23:00", "fim": "00:30", "titulo": "Viagem, à noite", "local": "São Paulo", "dia": "amanha"},
        ])

    def test_weekly_byday_count_and_until_and_daily_interval(self):
        from datetime import date
        ical = """BEGIN:VCALENDAR
BEGIN:VEVENT
DTSTART;TZID=America/Sao_Paulo:20261109T090000
DTEND;TZID=America/Sao_Paulo:20261109T100000
RRULE:FREQ=WEEKLY;BYDAY=MO,FR;COUNT=2
SUMMARY:Semanal
END:VEVENT
BEGIN:VEVENT
DTSTART;TZID=America/Sao_Paulo:20261109T110000
DTEND;TZID=America/Sao_Paulo:20261109T120000
RRULE:FREQ=DAILY;INTERVAL=2;UNTIL=20261113T140000Z
SUMMARY:Diário
END:VEVENT
END:VCALENDAR"""
        events = self.mod._ical_events(ical, date(2026, 11, 13))
        self.assertEqual([(e["titulo"], e["dia"], e["inicio"]) for e in events], [("Semanal", "hoje", "09:00"), ("Diário", "hoje", "11:00")])
        self.assertEqual(self.mod._ical_events(ical, date(2026, 11, 16)), [])

    def test_cache_and_secret_not_returned_on_network_failure(self):
        import io
        from datetime import datetime
        secret = "https://example.test/private-secret"
        self.agenda_file.write_text(secret)
        today = datetime.now(self.mod.SAO_PAULO).strftime("%Y%m%d")
        ical = f"BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;VALUE=DATE:{today}\nSUMMARY:Reunião\nEND:VEVENT\nEND:VCALENDAR".encode()
        with patch.object(self.mod.urllib.request, "urlopen", return_value=io.BytesIO(ical)) as request:
            first, error = self.extras.agenda()
            second, error2 = self.extras.agenda()
        self.assertIsNone(error)
        self.assertIsNone(error2)
        self.assertEqual(first, second)
        self.assertEqual(first[0]["titulo"], "Reunião")
        self.assertEqual(request.call_count, 1)
        self.assertEqual(request.call_args.kwargs["timeout"], 15)
        self.extras.agenda_cache = None
        with patch.object(self.mod.urllib.request, "urlopen", side_effect=OSError(secret)):
            out, error = self.extras.agenda()
        self.assertIsNone(out)
        self.assertNotIn(secret, error)

    def test_oversized_calendar_and_configuration_are_rejected(self):
        import io
        self.agenda_file.write_text("https://example.test/private")
        with patch.object(self.mod.urllib.request, "urlopen", return_value=io.BytesIO(b"x" * (self.mod.ICAL_MAX + 1))):
            result, error = self.extras.agenda()
        self.assertIsNone(result)
        self.assertIsNotNone(error)
        self.agenda_file.write_bytes(b"x" * 8193)
        with patch.object(self.mod.urllib.request, "urlopen") as request:
            self.assertIsNotNone(self.extras.agenda()[1])
            request.assert_not_called()


if __name__ == "__main__":
    unittest.main()
