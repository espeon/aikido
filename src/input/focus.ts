import { createSignal, onCleanup } from "solid-js";

export interface FocusableNode {
  id: string;
  element: HTMLElement;
  onActivate?: () => void;
}

const nodes = new Map<string, FocusableNode>();
const [focusedId, setFocusedId] = createSignal<string | null>(null);
const savedFocus: Record<string, string> = {};

export { focusedId, setFocusedId };

export function saveFocus() {
  const id = focusedId();
  if (!id) return;
  savedFocus[routeKey()] = id;
}

export function restoreFocus() {
  const key = routeKey();
  const id = savedFocus[key];
  if (id && nodes.has(id)) {
    setFocusedId(id);
    nodes.get(id)?.element.scrollIntoView({ block: "nearest" });
  } else {
    focusFirst();
  }
}

export function registerFocusable(node: FocusableNode) {
  nodes.set(node.id, node);
  onCleanup(() => {
    nodes.delete(node.id);
    if (focusedId() === node.id) setFocusedId(null);
  });
}

function routeKey(): string {
  return location.hash.slice(1) || "/";
}

function autoSave(id: string) {
  savedFocus[routeKey()] = id;
}

export function moveFocus(direction: "up" | "down" | "left" | "right") {
  const fid = focusedId();
  const current = fid ? nodes.get(fid) : null;

  let candidates = [...nodes.values()].filter((n) => {
    if (n.element.offsetParent === null) return false;
    return n !== current;
  });

  if (candidates.length === 0) return null;

  const next = findNearestInDirection(current ?? null, candidates, direction);
  if (next) {
    setFocusedId(next.id);
    autoSave(next.id);
    next.element.scrollIntoView({ block: "nearest", behavior: "smooth" });
    return next;
  }
  return null;
}

export function activateCurrent() {
  const id = focusedId();
  if (!id) return;
  saveFocus();
  const node = nodes.get(id);
  if (node) node.onActivate?.();
}

export function focusFirst() {
  const candidates = [...nodes.values()].filter((n) => n.element.offsetParent !== null);
  if (candidates.length > 0) {
    setFocusedId(candidates[0].id);
    autoSave(candidates[0].id);
  }
}

function findNearestInDirection(
  from: FocusableNode | null,
  candidates: FocusableNode[],
  direction: string,
): FocusableNode | null {
  if (candidates.length === 0) return null;
  if (!from) return candidates[0];

  const fromRect = from.element.getBoundingClientRect();
  const fromX = fromRect.left + fromRect.width / 2;
  const fromY = fromRect.top + fromRect.height / 2;

  let best: FocusableNode | null = null;
  let bestDist = Infinity;

  for (const candidate of candidates) {
    const rect = candidate.element.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const dx = cx - fromX;
    const dy = cy - fromY;

    const inDirection =
      (direction === "up" && dy < 0) ||
      (direction === "down" && dy > 0) ||
      (direction === "left" && dx < 0) ||
      (direction === "right" && dx > 0);

    if (!inDirection) continue;

    const primary = direction === "up" || direction === "down" ? Math.abs(dy) : Math.abs(dx);
    const secondary = direction === "up" || direction === "down" ? Math.abs(dx) : Math.abs(dy);
    const dist = primary + secondary * 2;

    if (dist < bestDist) {
      bestDist = dist;
      best = candidate;
    }
  }

  return best;
}
