/** Errores públicos del servicio. Nunca incluyen detalles de Google ni de la clave. */
const ERRORS = {
  not_found: [404, "Esa operación no existe."],
  method_not_allowed: [405, "Esa operación solo admite POST."],
  invalid_request: [400, "La solicitud no tiene un formato válido."],
  invalid_install: [400, "No se pudo identificar la instalación de Nexium."],
  payload_too_large: [413, "La solicitud o la imagen es demasiado grande."],
  region_unavailable: [451, "El servicio de IA de Nexium no está disponible en tu región: no se ofrece en el Espacio Económico Europeo, Reino Unido ni Suiza."],
  country_unknown: [451, "No se pudo determinar tu país, y el servicio de IA de Nexium no se ofrece sin conocerlo. Si usas Tor o una conexión que oculte tu ubicación, prueba con otra conexión."],
  rate_limited: [429, "Demasiadas solicitudes en poco tiempo. Espera un momento y vuelve a intentarlo."],
  daily_limit: [429, "Se alcanzó el límite diario de uso de IA de este equipo o esta red. Vuelve a intentarlo mañana."],
  service_quota: [429, "Se agotó la cuota diaria gratuita del servicio de IA de Nexium. Vuelve a intentarlo mañana."],
  no_output: [502, "El modelo no devolvió ninguna respuesta."],
  unavailable: [503, "El servicio de IA no está disponible en este momento."],
  timeout: [504, "El servicio de IA tardó demasiado en responder."],
} as const satisfies Record<string, readonly [number, string]>;

export type ErrorCode = keyof typeof ERRORS;

export class ApiError extends Error {
  constructor(
    readonly code: ErrorCode,
    readonly retryAfterSeconds?: number,
  ) {
    super(code);
  }
}

export function errorStatus(code: ErrorCode): number {
  return ERRORS[code][0];
}

export function errorResponse(error: ApiError): Response {
  const [status, message] = ERRORS[error.code];
  const headers = new Headers({ "Content-Type": "application/json; charset=utf-8" });
  if (error.retryAfterSeconds !== undefined) headers.set("Retry-After", String(error.retryAfterSeconds));
  return new Response(JSON.stringify({ error: { code: error.code, message } }), { status, headers });
}
