import { ApiError } from "./errors";

/**
 * Los términos de Google solo permiten servicios de pago en aplicaciones ofrecidas a estas regiones,
 * y Nexium usa la capa gratuita: Unión Europea con sus regiones ultraperiféricas, resto del EEE, Reino Unido y Suiza.
 */
const BLOCKED_COUNTRIES = new Set([
  "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IE", "IT", "LV", "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK", "SI", "ES", "SE",
  "AX", "GF", "GP", "MF", "MQ", "RE", "YT",
  "IS", "LI", "NO",
  "GB",
  "CH",
]);

/** Cloudflare indica XX si desconoce el país y T1 para la red Tor: sin saber dónde está, no se puede ofrecer. */
const UNKNOWN_COUNTRIES = new Set(["XX", "T1"]);

/** `request.cf` lo rellena Cloudflare: el cliente no puede falsificarlo, a diferencia de una cabecera. */
export function assertRegionAllowed(country: unknown): void {
  if (typeof country !== "string" || UNKNOWN_COUNTRIES.has(country)) throw new ApiError("country_unknown");
  if (BLOCKED_COUNTRIES.has(country)) throw new ApiError("region_unavailable");
}
