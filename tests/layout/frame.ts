/**
 * Comprobaciones del marco de la app, antes de desplazar nada. Devuelve un problema por cada:
 *
 * - desplazamiento de la ventana o del marco: solo se desplaza la zona de contenido de cada pantalla;
 * - control de la barra de señal, de la navegación o del pie de Ajustes que no se ve entero en la ventana,
 *   queda fuera de su barra o está tapado;
 * - barra de señal que se desplaza o se sale de la ventana;
 * - zona de contenido sin alto suficiente para un control.
 *
 * Debe ser autosuficiente: Playwright envía su código fuente al navegador.
 */
export function auditFrame(): string[] {
  const problems: string[] = [];
  const tolerance = 1;
  const width = document.documentElement.clientWidth;
  const height = document.documentElement.clientHeight;
  const modal = document.querySelector("dialog[open]");

  const describe = (element: Element) => {
    const html = element as HTMLElement;
    const name = element.getAttribute("aria-label") ?? html.innerText?.trim().slice(0, 40) ?? "";
    return `<${element.tagName.toLowerCase()}${name ? ` «${name.replace(/\s+/g, " ")}»` : ""}>`;
  };
  const shown = (element: Element) => {
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.display !== "none" && style.visibility !== "hidden" && rect.width > 1 && rect.height > 1;
  };
  const inWindow = (rect: DOMRect) => rect.left >= -tolerance && rect.top >= -tolerance && rect.right <= width + tolerance && rect.bottom <= height + tolerance;
  const inside = (rect: DOMRect, box: DOMRect) =>
    rect.left >= box.left - tolerance && rect.top >= box.top - tolerance && rect.right <= box.right + tolerance && rect.bottom <= box.bottom + tolerance;

  /** Visible entero, sin desplazar nada, dentro de su contenedor y sin nada encima. */
  const checkFixed = (control: Element, container: Element, where: string) => {
    const rect = control.getBoundingClientRect();
    if (!inWindow(rect)) {
      problems.push(`${describe(control)} (${where}) no se ve entero en la ventana (${Math.round(rect.top)} a ${Math.round(rect.bottom)} de ${height} px de alto).`);
      return;
    }
    if (!inside(rect, container.getBoundingClientRect())) problems.push(`${describe(control)} se sale de ${where}.`);
    if (modal) return;
    const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
    const label = control.closest("label");
    if (hit && !control.contains(hit) && !hit.contains(control) && !(label && label.contains(hit))) {
      problems.push(`${describe(control)} (${where}) está tapado por ${describe(hit)}.`);
    }
  };

  const scroller = document.scrollingElement ?? document.documentElement;
  if (scroller.scrollHeight > scroller.clientHeight + tolerance || scroller.scrollTop > 0) {
    problems.push(`La ventana se desplaza: ${scroller.scrollHeight} px de contenido en ${scroller.clientHeight} px.`);
  }
  const shell = document.querySelector("[data-shell]");
  if (!shell) problems.push("No se encontró el marco de la app.");
  else if (shell.scrollHeight > shell.clientHeight + tolerance) {
    problems.push(`El marco se desplaza: ${shell.scrollHeight} px de contenido en ${shell.clientHeight} px.`);
  }

  const bar = document.querySelector('section[aria-label="Barra de señal"]');
  if (!bar) problems.push("No se encontró la barra de señal.");
  else {
    const rect = bar.getBoundingClientRect();
    if (!inWindow(rect)) problems.push(`La barra de señal se sale de la ventana (${Math.round(rect.top)} a ${Math.round(rect.bottom)} de ${height} px).`);
    if (Math.abs(rect.bottom - height) > tolerance) problems.push(`La barra de señal no está al pie de la ventana (termina en ${Math.round(rect.bottom)} de ${height} px).`);
    if (bar.scrollHeight > bar.clientHeight + tolerance) problems.push("La barra de señal se desplaza o recorta su contenido.");
    for (const control of bar.querySelectorAll("button, input, select, textarea")) {
      // Un enlace dentro del mensaje se desplaza con su texto (lo revisa auditLayout); los controles van en su fila.
      if (shown(control) && !control.closest("[data-bar-message]")) checkFixed(control, bar, "la barra de señal");
    }
  }

  const nav = document.querySelector('nav[aria-label="Secciones"]');
  if (nav) for (const control of nav.querySelectorAll("button")) if (shown(control)) checkFixed(control, nav, "la navegación");

  for (const button of document.querySelectorAll("button")) {
    if (button.textContent?.trim() === "Guardar ajustes" && shown(button)) {
      if (button.closest("[data-scroll-area]")) problems.push("«Guardar ajustes» está dentro de la zona que se desplaza.");
      checkFixed(button, button.parentElement ?? document.body, "el pie de Ajustes");
    }
  }

  const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
  for (const area of document.querySelectorAll("[data-scroll-area]")) {
    if (area.clientHeight < 2.75 * rem) problems.push(`La zona de contenido solo tiene ${area.clientHeight} px de alto: no cabe ni un control.`);
  }

  return problems;
}
