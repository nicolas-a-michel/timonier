<?php

// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry starter, `TimonierOtel`.

declare(strict_types=1);

namespace Timonier;

use OpenTelemetry\API\Globals;
use PHPUnit\Framework\TestCase;

require_once __DIR__ . '/TimonierOtel.php';

final class TimonierSmokeTest extends TestCase
{
    private function smoke(): void
    {
        Globals::tracerProvider()->getTracer('timonier-smoke')
            ->spanBuilder('smoke')
            ->setAttribute('code.function.name', 'smoke')
            ->setAttribute('code.file.path', 'tests/Timonier/TimonierSmokeTest.php')
            // Where `smoke` starts, as the conventions ask.
            ->setAttribute('code.line.number', 18)
            ->setAttribute('timonier.test.name', 'testSmokeTraceReachesTimonier')
            ->startSpan()
            ->end();
    }

    public function testSmokeTraceReachesTimonier(): void
    {
        if (getenv('OTEL_EXPORTER_OTLP_ENDPOINT') === false) {
            $this->markTestSkipped('no run of timonier test');
        }
        TimonierOtel::start();
        $this->smoke();
        TimonierOtel::stop();
        $this->addToAssertionCount(1);
    }
}
