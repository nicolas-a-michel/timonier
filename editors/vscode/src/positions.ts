/**
 * The model counts columns in UTF-8 bytes (ADR 0008), VSCode in UTF-16 code units: the relay of
 * the IDE's language servers converts on each side (ADR 0014). Pure, for the unit tests.
 */
import path from "node:path";

/** Bytes of one code point in UTF-8. */
function utf8Length(codePoint: number): number {
  if (codePoint < 0x80) return 1;
  if (codePoint < 0x800) return 2;
  if (codePoint < 0x10000) return 3;
  return 4;
}

/** The UTF-16 column of the UTF-8 byte `column` of `line`. Past the end: the end. */
export function utf8ToUtf16(line: string, column: number): number {
  let bytes = 0;
  let units = 0;
  for (const char of line) {
    if (bytes >= column) break;
    const codePoint = char.codePointAt(0) ?? 0;
    bytes += utf8Length(codePoint);
    units += char.length;
  }
  return units;
}

/** The UTF-8 byte column of the UTF-16 `column` of `line`. Past the end: the end. */
export function utf16ToUtf8(line: string, column: number): number {
  let bytes = 0;
  let units = 0;
  for (const char of line) {
    if (units >= column) break;
    bytes += utf8Length(char.codePointAt(0) ?? 0);
    units += char.length;
  }
  return bytes;
}

/** `file` relative to `root`, with `/` separators; `null` outside it. */
export function relativeInside(root: string, file: string): string | null {
  const relative = path.relative(root, file);
  if (relative === "" || relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    return null;
  }
  return relative.split(path.sep).join("/");
}

/** `file`, relative to `root` with `/` separators, as an absolute path; `null` if it leaves the root. */
export function resolveInside(root: string, file: string): string | null {
  const full = path.resolve(root, file);
  return relativeInside(root, full) === null ? null : full;
}
