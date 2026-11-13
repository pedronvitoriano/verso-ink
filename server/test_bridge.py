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


if __name__ == "__main__":
    unittest.main()
