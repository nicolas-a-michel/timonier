/** Entry files of the webview build, found through Vite's manifest (`.vite/manifest.json`). */
export interface WebviewEntry {
  script: string;
  styles: string[];
}

interface ManifestChunk {
  file?: unknown;
  css?: unknown;
  isEntry?: unknown;
}

export function webviewEntry(manifestJson: string, entryKey = "index.html"): WebviewEntry {
  const manifest = JSON.parse(manifestJson) as Record<string, ManifestChunk | undefined>;
  const chunk = manifest[entryKey];
  if (chunk?.isEntry !== true || typeof chunk.file !== "string") {
    throw new Error(`the webview manifest has no entry "${entryKey}"`);
  }
  const styles = Array.isArray(chunk.css) ? chunk.css.filter((f): f is string => typeof f === "string") : [];
  return { script: chunk.file, styles };
}
