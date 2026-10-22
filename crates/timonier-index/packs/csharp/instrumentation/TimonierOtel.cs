// OpenTelemetry for this project's tests, written by `timonier init` (Timonier's C#
// conventions, in `.timonier/conventions/csharp/`). The SDK starts when a run of `timonier test`
// asks (`OTEL_EXPORTER_OTLP_ENDPOINT`), reads `OTEL_RESOURCE_ATTRIBUTES` and
// `OTEL_EXPORTER_OTLP_HEADERS`, and stays off with `OTEL_SDK_DISABLED=true`.
using OpenTelemetry;
using OpenTelemetry.Exporter;
using OpenTelemetry.Trace;

namespace Timonier;

public static class TimonierOtel
{
    private static TracerProvider? provider;

    /// <summary>Starts the export once, when a run asks for it; every ActivitySource is listened to.</summary>
    public static void Start()
    {
        var endpoint = Environment.GetEnvironmentVariable("OTEL_EXPORTER_OTLP_ENDPOINT");
        if (string.IsNullOrEmpty(endpoint) || provider is not null)
        {
            return;
        }
        if (string.Equals(Environment.GetEnvironmentVariable("OTEL_SDK_DISABLED"), "true", StringComparison.OrdinalIgnoreCase))
        {
            return;
        }
        provider = Sdk.CreateTracerProviderBuilder()
            .AddSource("*")
            // Timonier receives OTLP over HTTP; the endpoint and the headers come from the run.
            .AddOtlpExporter(options => options.Protocol = OtlpExportProtocol.HttpProtobuf)
            .Build();
    }

    /// <summary>Flushes and stops the export.</summary>
    public static void Stop()
    {
        provider?.ForceFlush();
        provider?.Dispose();
        provider = null;
    }
}
