// Package timonier starts OpenTelemetry for this project, written by `timonier init`
// (Timonier's Go conventions, in `.timonier/conventions/go/`). The SDK starts when a run of
// `timonier test` asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.
package timonier

import (
	"context"
	"os"
	"strings"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp"
	"go.opentelemetry.io/otel/sdk/resource"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
)

// StartTelemetry starts the export when a run asks for it; it returns what flushes and stops it.
func StartTelemetry(ctx context.Context) func(context.Context) {
	none := func(context.Context) {}
	if os.Getenv("OTEL_EXPORTER_OTLP_ENDPOINT") == "" {
		return none
	}
	if strings.EqualFold(os.Getenv("OTEL_SDK_DISABLED"), "true") {
		return none
	}
	exporter, err := otlptracehttp.New(ctx)
	if err != nil {
		return none
	}
	provider := sdktrace.NewTracerProvider(
		sdktrace.WithBatcher(exporter),
		sdktrace.WithResource(resource.Default()),
	)
	otel.SetTracerProvider(provider)
	return func(ctx context.Context) { _ = provider.Shutdown(ctx) }
}
