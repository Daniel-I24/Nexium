/** Cantidad con el sustantivo concordado: "1 solicitud", "0 solicitudes", "2 tokens". Cifras con separador de miles en español. */
export function count(amount: number, singular: string, plural: string): string {
  return `${amount.toLocaleString("es")} ${amount === 1 ? singular : plural}`;
}
