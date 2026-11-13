"""Cotações da AwesomeAPI, com cache em memória e limite de resposta."""

import datetime
import http.client
import json
import math
import threading
import time
import urllib.request


URL = "https://economia.awesomeapi.com.br/json/last/USD-BRL,EUR-BRL,GBP-BRL,ARS-BRL,BTC-BRL"
MOEDAS = ("USD", "EUR", "GBP", "ARS", "BTC")
MAX_RESPONSE = 1024 * 1024
CACHE_SECONDS = 3600


class Cotacoes:
    def __init__(self):
        self._lock = threading.Lock()
        self._cache = None
        self._cached_at = 0

    def obter(self):
        """Devolve (status HTTP, JSON); usa a última resposta válida em falha."""
        with self._lock:
            if self._cache is not None and time.monotonic() - self._cached_at < CACHE_SECONDS:
                return 200, dict(self._cache)
            try:
                with urllib.request.urlopen(URL, timeout=15) as response:
                    raw = response.read(MAX_RESPONSE + 1)
                if len(raw) > MAX_RESPONSE:
                    raise ValueError("resposta muito grande")
                payload = json.loads(raw)
                valores = {}
                for moeda in MOEDAS:
                    value = float(payload[moeda + "BRL"]["bid"])
                    if not math.isfinite(value) or value <= 0:
                        raise ValueError("cotação inválida")
                    valores[moeda] = value
                valores["quando"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
            except (OSError, http.client.HTTPException, ValueError, TypeError, KeyError, OverflowError):
                if self._cache is None:
                    return 502, {"error": "não foi possível obter as cotações"}
                return 200, {**self._cache, "antigo": True}
            self._cache = valores
            self._cached_at = time.monotonic()
            return 200, dict(valores)
