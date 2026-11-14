import "@testing-library/jest-dom/vitest";

Element.prototype.scrollIntoView = () => undefined;
Element.prototype.scrollTo = () => undefined;

// jsdom no implementa el diálogo modal. Basta con abrirlo y cerrarlo: la inercia del fondo la prueba el navegador real.
if (typeof HTMLDialogElement !== "undefined" && !HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
}
