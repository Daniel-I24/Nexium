import { describe, expect, it } from "vitest";

import { count } from "./plural";

describe("concordancia de cantidades", () => {
  it("usa el singular solo para uno", () => {
    expect(count(1, "solicitud", "solicitudes")).toBe("1 solicitud");
    expect(count(0, "solicitud", "solicitudes")).toBe("0 solicitudes");
    expect(count(2, "token", "tokens")).toBe("2 tokens");
  });
});
