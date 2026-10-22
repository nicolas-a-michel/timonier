// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry package, `timonier/otel.go`.
package timonier

import (
	"context"
	"os"
	"testing"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/trace"
)

func smoke(ctx context.Context) {
	_, span := otel.Tracer("timonier-smoke").Start(ctx, "smoke", trace.WithAttributes(
		attribute.String("code.function.name", "smoke"),
		attribute.String("code.file.path", "timonier/timonier_smoke_test.go"),
		// Where `smoke` starts, as the conventions ask.
		attribute.Int("code.line.number", 16),
		attribute.String("timonier.test.name", "TestTimonierSmoke"),
	))
	span.End()
}

func TestTimonierSmoke(t *testing.T) {
	if os.Getenv("OTEL_EXPORTER_OTLP_ENDPOINT") == "" {
		t.Skip("no run of timonier test")
	}
	ctx := context.Background()
	stop := StartTelemetry(ctx)
	smoke(ctx)
	stop(ctx)
}
