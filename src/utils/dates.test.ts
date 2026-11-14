import { describe, expect, it } from "vitest";

import { isCompleteLocal } from "./dates";

describe("fecha y hora de un recordatorio", () => {
  it("solo se guarda con fecha y hora completas", () => {
    expect(isCompleteLocal("2026-11-14T09:00")).toBe(true);
    expect(isCompleteLocal("2026-11-14T")).toBe(false);
    expect(isCompleteLocal("T09:00")).toBe(false);
    expect(isCompleteLocal("")).toBe(false);
  });
});
