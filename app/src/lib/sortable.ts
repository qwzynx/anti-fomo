import type { Action } from "svelte/action";

/**
 * Drag-to-reorder, from a handle.
 *
 * Pointer events rather than HTML5 drag and drop: Tauri's window takes over
 * native drag and drop for files (on Windows it switches the webview's HTML5
 * drag events off entirely), and the HTML5 API has no touch support anyway.
 * This works the same under a mouse, a pen and a finger.
 *
 * The handle sits inside an element marked `data-sort-item`; that element's
 * siblings marked the same way are the list. While dragging, the item follows
 * the pointer and the others slide aside to show where it will land; nothing
 * is moved in the DOM until the drop, which reports `onMove(from, to)` and
 * leaves the actual reordering to the caller's state. The nearest scrolling
 * ancestor scrolls when the pointer nears its edge.
 *
 * The handle also takes ↑/↓ from the keyboard, so reordering never needs a
 * pointer at all.
 */
export interface SortParams {
  onMove: (from: number, to: number) => void;
}

function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let node = el.parentElement; node; node = node.parentElement) {
    const { overflowY } = getComputedStyle(node);
    if ((overflowY === "auto" || overflowY === "scroll") && node.scrollHeight > node.clientHeight) {
      return node;
    }
  }
  return null;
}

function siblings(item: HTMLElement): HTMLElement[] {
  const parent = item.parentElement;
  if (!parent) return [];
  return [...parent.children].filter(
    (c): c is HTMLElement => c instanceof HTMLElement && c.hasAttribute("data-sort-item"),
  );
}

export const sortHandle: Action<HTMLElement, SortParams> = (handle, initial) => {
  let params = initial;

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    const item = handle.closest<HTMLElement>("[data-sort-item]");
    if (!item) return;
    const list = siblings(item);
    const from = list.indexOf(item);
    if (from < 0 || list.length < 2) return;
    e.preventDefault();
    handle.setPointerCapture(e.pointerId);

    const scroller = scrollParent(item);
    const startY = e.clientY;
    const startScroll = scroller?.scrollTop ?? 0;
    const rects = list.map((el) => el.getBoundingClientRect());
    const mids = rects.map((r) => r.top + r.height / 2);
    // What the others slide by to make room: the dragged item and one gap.
    const gap =
      from + 1 < list.length
        ? rects[from + 1].top - rects[from].bottom
        : from > 0
          ? rects[from].top - rects[from - 1].bottom
          : 0;
    const shift = rects[from].height + Math.max(0, gap);

    let to = from;
    let lastY = e.clientY;
    let frame = 0;

    item.style.position = "relative";
    item.style.zIndex = "20";
    item.dataset.sorting = "";
    for (const el of list) if (el !== item) el.style.transition = "transform 150ms ease";
    document.body.style.cursor = "grabbing";

    function update() {
      // In content coordinates, so scrolling the pane mid-drag keeps the item
      // under the pointer and the measured midpoints still meaningful.
      const scrolled = (scroller?.scrollTop ?? 0) - startScroll;
      const dy = lastY - startY + scrolled;
      item!.style.transform = `translateY(${dy}px)`;
      const centre = mids[from] + dy;
      let target = 0;
      for (let i = 0; i < list.length; i++) if (i !== from && mids[i] < centre) target++;
      to = target;
      list.forEach((el, i) => {
        if (i === from) return;
        const off =
          from < to && i > from && i <= to ? -shift : from > to && i >= to && i < from ? shift : 0;
        el.style.transform = off ? `translateY(${off}px)` : "";
      });
    }

    function autoscroll() {
      if (scroller) {
        const r = scroller.getBoundingClientRect();
        const edge = 48;
        const v =
          lastY < r.top + edge
            ? -Math.ceil((r.top + edge - lastY) / 4)
            : lastY > r.bottom - edge
              ? Math.ceil((lastY - (r.bottom - edge)) / 4)
              : 0;
        if (v) {
          scroller.scrollTop += v;
          update();
        }
      }
      frame = requestAnimationFrame(autoscroll);
    }
    frame = requestAnimationFrame(autoscroll);

    function onMove(ev: PointerEvent) {
      lastY = ev.clientY;
      update();
    }

    function onEnd() {
      cancelAnimationFrame(frame);
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onEnd);
      handle.removeEventListener("pointercancel", onEnd);
      for (const el of list) {
        el.style.transform = "";
        el.style.transition = "";
      }
      item!.style.position = "";
      item!.style.zIndex = "";
      delete item!.dataset.sorting;
      document.body.style.cursor = "";
      if (to !== from) params.onMove(from, to);
    }

    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onEnd);
    handle.addEventListener("pointercancel", onEnd);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    const item = handle.closest<HTMLElement>("[data-sort-item]");
    if (!item) return;
    const list = siblings(item);
    const from = list.indexOf(item);
    const to = from + (e.key === "ArrowUp" ? -1 : 1);
    e.preventDefault();
    if (from < 0 || to < 0 || to >= list.length) return;
    params.onMove(from, to);
  }

  handle.addEventListener("pointerdown", onPointerDown);
  handle.addEventListener("keydown", onKeydown);
  return {
    update(next: SortParams) {
      params = next;
    },
    destroy() {
      handle.removeEventListener("pointerdown", onPointerDown);
      handle.removeEventListener("keydown", onKeydown);
    },
  };
};

/** `list` with the element at `from` moved to `to`. */
export function moved<T>(list: readonly T[], from: number, to: number): T[] {
  const next = [...list];
  const [it] = next.splice(from, 1);
  next.splice(to, 0, it);
  return next;
}
