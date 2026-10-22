<?php

// OpenTelemetry for this project's tests, written by `timonier init` (Timonier's PHP
// conventions, in `.timonier/conventions/php/`). The SDK starts when a run of `timonier test`
// asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.

declare(strict_types=1);

namespace Timonier;

use OpenTelemetry\Contrib\Otlp\OtlpHttpTransportFactory;
use OpenTelemetry\Contrib\Otlp\SpanExporter;
use OpenTelemetry\SDK\Resource\ResourceInfoFactory;
use OpenTelemetry\SDK\Sdk;
use OpenTelemetry\SDK\Trace\SpanProcessor\SimpleSpanProcessor;
use OpenTelemetry\SDK\Trace\TracerProvider;

final class TimonierOtel
{
    private static ?TracerProvider $provider = null;

    /** Starts the export once, when a run asks for it. */
    public static function start(): void
    {
        $endpoint = getenv('OTEL_EXPORTER_OTLP_ENDPOINT');
        if ($endpoint === false || $endpoint === '' || self::$provider !== null) {
            return;
        }
        if (strtolower((string) getenv('OTEL_SDK_DISABLED')) === 'true') {
            return;
        }
        // `key=value,key=value`, as the specification writes the headers.
        $headers = [];
        foreach (explode(',', (string) getenv('OTEL_EXPORTER_OTLP_HEADERS')) as $pair) {
            if (str_contains($pair, '=')) {
                [$key, $value] = explode('=', $pair, 2);
                $headers[trim($key)] = rawurldecode(trim($value));
            }
        }
        $transport = (new OtlpHttpTransportFactory())->create(
            rtrim($endpoint, '/') . '/v1/traces',
            'application/x-protobuf',
            $headers,
        );
        self::$provider = new TracerProvider(
            new SimpleSpanProcessor(new SpanExporter($transport)),
            null,
            ResourceInfoFactory::defaultResource(),
        );
        Sdk::builder()->setTracerProvider(self::$provider)->buildAndRegisterGlobal();
    }

    /** Flushes and stops the export. */
    public static function stop(): void
    {
        self::$provider?->shutdown();
        self::$provider = null;
    }
}
