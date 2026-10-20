import { describe, expect, it } from "vitest";

import { applyDensity } from "./density";
import { isHostMessage } from "./protocol";

describe("the density the host gives (Q32, review n° 2 of Phase 5, m-6)", () => {
  it("is a message of the host, comfortable or compact", () => {
    expect(isHostMessage({ type: "density", density: "compact" })).toBe(true);
    expect(isHostMessage({ type: "density", density: "comfortable" })).toBe(true);
    expect(isHostMessage({ type: "density", density: "dense" })).toBe(false);
  });

  it("goes on <html>, where the tokens read it, and back to the map", () => {
    const root = { dataset: {} as DOMStringMap };
    expect(applyDensity("compact", root)).toBe("compact");
    expect(root.dataset.density).toBe("compact");
    expect(applyDensity("comfortable", root)).toBe("comfortable");
    expect(root.dataset.density).toBe("comfortable");
  });
});
