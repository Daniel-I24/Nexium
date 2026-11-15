import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { WeatherCityQuestion, weatherRequest } from "./WeatherCityQuestion";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

describe("ciudad para el clima en la conversación", () => {
  beforeEach(() => invoke.mockReset());

  it("guarda la ciudad comprobada y pide el clima con su nombre oficial", async () => {
    invoke.mockResolvedValueOnce("Medellín");
    const onAsk = vi.fn();
    render(<WeatherCityQuestion when="tomorrow" onAsk={onAsk} />);
    await userEvent.type(screen.getByLabelText("Ciudad"), "medellin{enter}");
    expect(invoke).toHaveBeenCalledWith("weather_city_set", { city: "medellin" });
    expect(onAsk).toHaveBeenCalledWith("¿Cómo estará el clima mañana en Medellín?");
  });

  it("sin guardarla, solo se usa para esta solicitud", async () => {
    const onAsk = vi.fn();
    render(<WeatherCityQuestion when="now" onAsk={onAsk} />);
    await userEvent.click(screen.getByRole("checkbox", { name: "Guardarla como ciudad para el clima" }));
    await userEvent.type(screen.getByLabelText("Ciudad"), "Cali");
    await userEvent.click(screen.getByRole("button", { name: "Consultar el clima" }));
    expect(invoke).not.toHaveBeenCalled();
    expect(onAsk).toHaveBeenCalledWith("¿Cómo está el clima en Cali?");
  });

  it("si la ciudad no existe, lo dice y deja corregirla", async () => {
    invoke.mockRejectedValueOnce("No encontré la ciudad «Nolugar». Escríbela de otra forma, por ejemplo con su país.");
    const onAsk = vi.fn();
    render(<WeatherCityQuestion when="now" onAsk={onAsk} />);
    await userEvent.type(screen.getByLabelText("Ciudad"), "Nolugar{enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("No encontré la ciudad «Nolugar»");
    expect(onAsk).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Ciudad")).toBeEnabled();
  });

  it("conserva para cuándo era la solicitud", () => {
    expect(weatherRequest("Bogotá", "today")).toBe("¿Cómo estará el clima hoy en Bogotá?");
  });
});
