// Smoke test — verifies the Vitest harness boots end-to-end.
// Real unit tests will be added alongside their respective modules in later milestones.
import { describe, expect, it } from "vitest";

describe("vitest smoke", () => {
  it("runs basic arithmetic", () => {
    expect(1 + 1).toBe(2);
  });
});
