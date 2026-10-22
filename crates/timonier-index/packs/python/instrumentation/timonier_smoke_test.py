"""Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
`timonier test`, and its span must come back attached to the code graph. It goes through this
project's OpenTelemetry module, `timonier_otel.py`."""

import os

import pytest
from opentelemetry import trace

from timonier_otel import start_telemetry


def smoke():
    with trace.get_tracer("timonier-smoke").start_as_current_span(
        "smoke",
        attributes={
            "code.function.name": "smoke",
            "code.file.path": "timonier_smoke_test.py",
            # Where `smoke` starts, as the conventions ask.
            "code.line.number": 13,
            "timonier.test.name": "test_timonier_smoke",
        },
    ):
        pass


@pytest.mark.skipif(not os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT"), reason="no run of timonier test")
def test_timonier_smoke():
    stop = start_telemetry()
    smoke()
    stop()
