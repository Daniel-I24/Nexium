/**
 * Auditoría de diseño que se ejecuta dentro de la página. Devuelve un problema por cada:
 *
 * - desplazamiento horizontal en la ventana o en cualquier contenedor;
 * - control (botón, campo, selector, enlace) que, tras desplazarse hasta él, no queda entero en la ventana,
 *   queda cortado por un antepasado que recorta, o está tapado por otro elemento;
 * - texto que se sale de su caja o de la ventana.
 *
 * Debe ser autosuficiente: Playwright envía su código fuente al navegador.
 * `data-clip-ok` marca recortes intencionados (el trazo del micrófono, el progreso indeterminado).
 */
export function auditLayout(): string[] {
  const problems: string[] = [];
  const tolerance = 1;
  const width = document.documentElement.clientWidth;
  const height = document.documentElement.clientHeight;

  const describe = (element: Element) => {
    const html = element as HTMLElement;
    const name = element.getAttribute("aria-label") ?? html.innerText?.trim().slice(0, 40) ?? "";
    return `<${element.tagName.toLowerCase()}${name ? ` «${name.replace(/\s+/g, " ")}»` : ""}>`;
  };
  const visible = (element: Element) => {
    if (element.closest("[data-clip-ok]")) return false;
    const style = getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden") return false;
    const rect = element.getBoundingClientRect();
    // Lo de 1 px o menos es texto solo para lectores de pantalla (sr-only), no para verse.
    return rect.width > 1 && rect.height > 1;
  };
  const clipsX = (style: CSSStyleDeclaration) => style.overflowX !== "visible";
  const clipsY = (style: CSSStyleDeclaration) => style.overflowY !== "visible";

  if (document.documentElement.scrollWidth > width + tolerance) {
    problems.push(`La ventana tiene desplazamiento horizontal: ${document.documentElement.scrollWidth} px de contenido en ${width} px.`);
  }

  for (const element of document.querySelectorAll("body *")) {
    if (!visible(element)) continue;
    const style = getComputedStyle(element);
    // Un campo de texto desplaza su propio contenido al escribir: eso no es un desborde del diseño.
    const scrollsItsText = element instanceof HTMLInputElement && ["text", "search", ""].includes(element.type);
    if (!scrollsItsText && clipsX(style) && element.scrollWidth > element.clientWidth + tolerance && element.clientWidth > 0) {
      problems.push(`${describe(element)} desborda en horizontal: ${element.scrollWidth} px de contenido en ${element.clientWidth} px.`);
    }
  }

  // Con un diálogo modal abierto, el resto de la ventana está inerte: solo cuenta lo que hay en él.
  const scope: Element = document.querySelector("dialog[open]") ?? document.body;
  const controls = scope.querySelectorAll("button, input, select, textarea, a[href], [tabindex]:not([tabindex='-1'])");
  for (const control of controls) {
    if (!visible(control)) continue;
    control.scrollIntoView({ block: "nearest", inline: "nearest" });
    const rect = control.getBoundingClientRect();
    if (rect.left < -tolerance || rect.top < -tolerance || rect.right > width + tolerance || rect.bottom > height + tolerance) {
      problems.push(`${describe(control)} queda fuera de la ventana aun desplazándose hasta él (${Math.round(rect.left)}, ${Math.round(rect.top)}, ${Math.round(rect.right)}, ${Math.round(rect.bottom)}).`);
      continue;
    }
    // Un diálogo modal está en la capa superior: lo que hay fuera de él no lo recorta.
    const top = control.closest("dialog[open]") ?? document.body;
    for (let ancestor = control.parentElement; ancestor && ancestor !== top; ancestor = ancestor.parentElement) {
      const style = getComputedStyle(ancestor);
      if (!clipsX(style) && !clipsY(style)) continue;
      const box = ancestor.getBoundingClientRect();
      const cutX = clipsX(style) && (rect.left < box.left - tolerance || rect.right > box.right + tolerance);
      const cutY = clipsY(style) && (rect.top < box.top - tolerance || rect.bottom > box.bottom + tolerance);
      if (cutX || cutY) {
        problems.push(`${describe(control)} queda cortado por ${describe(ancestor)}.`);
        break;
      }
    }
    const x = Math.min(Math.max(rect.left + rect.width / 2, 0), width - 1);
    const y = Math.min(Math.max(rect.top + rect.height / 2, 0), height - 1);
    const hit = document.elementFromPoint(x, y);
    const label = control.closest("label");
    if (hit && !control.contains(hit) && !hit.contains(control) && !(label && label.contains(hit))) {
      problems.push(`${describe(control)} está tapado por ${describe(hit)}.`);
    }
    if (control instanceof HTMLButtonElement && control.scrollWidth > control.clientWidth + tolerance) {
      problems.push(`El texto de ${describe(control)} se sale del botón.`);
    }
  }

  // Texto que se sale de la ventana o de su caja (palabras o rutas sin cortar).
  const walker = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const parent = node.parentElement;
    if (!parent || !node.textContent?.trim() || !visible(parent)) continue;
    const range = document.createRange();
    range.selectNodeContents(node);
    const text = range.getBoundingClientRect();
    if (text.width === 0) continue;
    const box = parent.getBoundingClientRect();
    if (text.right > box.right + 2 || text.left < box.left - 2) {
      problems.push(`El texto «${node.textContent.trim().slice(0, 40)}» se sale de ${describe(parent)}.`);
    } else if (text.right > width + tolerance) {
      problems.push(`El texto «${node.textContent.trim().slice(0, 40)}» se sale de la ventana.`);
    }
  }

  return [...new Set(problems)];
}
