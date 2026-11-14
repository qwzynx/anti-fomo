/**
 * Which overlay is on top, so Escape closes one layer and not all of them.
 *
 * The skills form opens over a posting's dialog ("Edit all my skills"), and a
 * confirm can open over either. Each listened for Escape on the window, so one
 * press closed every layer at once; each also locked and unlocked the page's
 * scroll on its own, so closing the top one let the page scroll under the
 * dialog still open beneath it. Layers register here instead: the top one
 * owns Escape, and the page stays locked while any is open.
 */

/**
 * Plain, not `$state`: it is read when a key is pressed or a layer opens,
 * never rendered from, and a reactive stack read *and* written inside the
 * Modal's own open effect made that effect depend on itself — an update loop
 * the moment a dialog opened.
 */
let stack: number[] = [];
let nextId = 1;

export const layers = {
  /** Registers a layer; returns its id and a release function. */
  open(): { id: number; close: () => void } {
    const id = nextId++;
    stack = [...stack, id];
    document.body.style.overflow = "hidden";
    return {
      id,
      close: () => {
        stack = stack.filter((x) => x !== id);
        if (stack.length === 0) document.body.style.overflow = "";
      },
    };
  },

  isTop(id: number): boolean {
    return stack[stack.length - 1] === id;
  },

  /** How deep a layer is, for stacking order. */
  depth(id: number): number {
    return stack.indexOf(id);
  },

  get any(): boolean {
    return stack.length > 0;
  },
};
