// Timonier's smoke test, written by `timonier init`: `timonier doctor` runs it under
// `timonier test`, and its span must come back attached to the code graph. It goes through this
// project's OpenTelemetry starter, `TimonierOtel`.
using System.Diagnostics;
using Xunit;

namespace Timonier;

public class TimonierSmokeTest
{
    private static readonly ActivitySource Source = new("timonier-smoke");

    private static void Smoke()
    {
        using var activity = Source.StartActivity("smoke");
        activity?.SetTag("code.function.name", "Smoke");
        activity?.SetTag("code.file.path", "Timonier/TimonierSmokeTest.cs");
        // Where `Smoke` starts, as the conventions ask.
        activity?.SetTag("code.line.number", 13);
        activity?.SetTag("timonier.test.name", "SmokeTraceReachesTimonier");
    }

    [Fact]
    public void SmokeTraceReachesTimonier()
    {
        if (Environment.GetEnvironmentVariable("OTEL_EXPORTER_OTLP_ENDPOINT") is null)
        {
            return; // no run of timonier test
        }
        TimonierOtel.Start();
        Smoke();
        TimonierOtel.Stop();
    }
}
