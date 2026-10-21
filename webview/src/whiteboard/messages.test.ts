/** The whiteboard's words and the host's messages to it (ADR 0037). */
import { describe, expect, it } from "vitest";

import { isWhiteboardMessage } from "../protocol";
import { whiteboardMessages } from "./messages";

describe("the whiteboard's messages", () => {
  it("takes the palette's commands, and nothing else", () => {
    expect(isWhiteboardMessage({ type: "whiteboard", command: { name: "undo" } })).toBe(true);
    expect(isWhiteboardMessage({ type: "whiteboard", command: { name: "tool", tool: "eraser" } })).toBe(true);
    expect(isWhiteboardMessage({ type: "whiteboard", command: { name: "tool", tool: "laser" } })).toBe(false);
    expect(isWhiteboardMessage({ type: "whiteboard", command: { name: "save" } })).toBe(false);
    expect(isWhiteboardMessage({ type: "whiteboard", command: { name: "add", shape: "zone" } })).toBe(true);
    expect(isWhiteboardMessage({ type: "whiteboard", command: { name: "add", shape: "arrow" } })).toBe(false);
    expect(isWhiteboardMessage({ type: "whiteboard", command: null })).toBe(false);
    expect(isWhiteboardMessage({ type: "command", command: { name: "undo" } })).toBe(false);
  });

  it("says every word in French too, and counts strokes in both languages", () => {
    expect(Object.keys(whiteboardMessages.fr).sort()).toEqual(Object.keys(whiteboardMessages.en).sort());
    expect(whiteboardMessages.en.count(1, 0)).toBe("1 stroke, 0 shapes");
    expect(whiteboardMessages.en.count(3, 1)).toBe("3 strokes, 1 shape");
    expect(whiteboardMessages.fr.count(0, 1)).toBe("0 trait, 1 forme");
    expect(whiteboardMessages.fr.count(2, 3)).toBe("2 traits, 3 formes");
    expect(whiteboardMessages.fr.limitLine("p95Ms", 4.5)).toBe("p95 < 4,5 ms");
    expect(whiteboardMessages.en.limitLine("memoryMb", 50)).toBe("memory < 50 MB");
  });
});
