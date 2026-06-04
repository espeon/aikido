import { onCleanup } from "solid-js";
import { listen } from "@tauri-apps/api/event";

// ── types ──

interface GamepadEvent {
  kind: "ButtonPressed" | "ButtonReleased" | "AxisChanged";
  button: string;
  value: number;
  gamepad_id: number;
}

type Handler = () => void;

// ── state ──

const pressHandlers = new Map<string, Set<Handler>>();
const releaseHandlers = new Map<string, Set<Handler>>();
const buttonState: Record<string, boolean> = {};
const axisState: Record<string, number> = {};
const justPressed = new Set<string>();

// ── public api ──

export function useButton(button: string, handler: Handler) {
  if (!pressHandlers.has(button)) pressHandlers.set(button, new Set());
  pressHandlers.get(button)!.add(handler);
  onCleanup(() => pressHandlers.get(button)?.delete(handler));
}

export function useButtonRelease(button: string, handler: Handler) {
  if (!releaseHandlers.has(button)) releaseHandlers.set(button, new Set());
  releaseHandlers.get(button)!.add(handler);
  onCleanup(() => releaseHandlers.get(button)?.delete(handler));
}

export function isPressed(button: string): boolean {
  return buttonState[button] ?? false;
}

export function getAxis(axis: string): number {
  return axisState[axis] ?? 0;
}

export function wasJustPressed(button: string): boolean {
  return justPressed.has(button);
}

// ── init ──

let started = false;

export function initGamepad() {
  if (started) return;
  started = true;

  // gilrs bridge (Rust backend events)
  listen<GamepadEvent>("gamepad", (event) => {
    processEvent(event.payload.kind, event.payload.button, event.payload.value);
  });

  // browser Gamepad API poll (primary on macOS)
  startBrowserPoll();
}

// ── browser gamepad api polling ──

const BUTTON_MAP: Record<number, string> = {
  0: "A", 1: "B", 2: "X", 3: "Y",
  4: "LB", 5: "RB", 6: "LT", 7: "RT",
  8: "Select", 9: "Start",
  10: "LStickClick", 11: "RStickClick",
  12: "DPadUp", 13: "DPadDown", 14: "DPadLeft", 15: "DPadRight",
};

function startBrowserPoll() {
  const prevButtons: Record<string, boolean> = {};
  const prevDpad: Record<string, boolean> = {};

  window.setInterval(() => {
    for (const gp of navigator.getGamepads()) {
      if (!gp) continue;
      pollButtons(gp, prevButtons);
      pollAxes(gp, prevDpad);
    }
  }, 50);
}

function pollButtons(gp: Gamepad, prev: Record<string, boolean>) {
  for (let i = 0; i < gp.buttons.length; i++) {
    const name = BUTTON_MAP[i] || `Button${i}`;
    const pressed = gp.buttons[i].pressed;
    if (pressed !== prev[name]) {
      prev[name] = pressed;
      processEvent(pressed ? "ButtonPressed" : "ButtonReleased", name, pressed ? 1 : 0);
    }
  }
}

function pollAxes(gp: Gamepad, prevDpad: Record<string, boolean>) {
  for (let i = 0; i < gp.axes.length; i++) {
    const val = gp.axes[i];
    processEvent("AxisChanged", `Axis${i}`, val);

    if (i === 6) {
      toggleDpad("DPadLeft", val < -0.7, "DPadRight", val > 0.7, prevDpad);
    }
    if (i === 7) {
      toggleDpad("DPadUp", val < -0.7, "DPadDown", val > 0.7, prevDpad);
    }
  }
}

function toggleDpad(
  negName: string, negOn: boolean,
  posName: string, posOn: boolean,
  prev: Record<string, boolean>,
) {
  if (negOn !== prev[negName]) { prev[negName] = negOn; processEvent(negOn ? "ButtonPressed" : "ButtonReleased", negName, negOn ? 1 : 0); }
  if (posOn !== prev[posName]) { prev[posName] = posOn; processEvent(posOn ? "ButtonPressed" : "ButtonReleased", posName, posOn ? 1 : 0); }
  if (!negOn && !posOn && (prev[negName] || prev[posName])) {
    if (prev[negName]) processEvent("ButtonReleased", negName, 0);
    if (prev[posName]) processEvent("ButtonReleased", posName, 0);
    prev[negName] = false; prev[posName] = false;
  }
}

// ── event processing ──

function processEvent(kind: string, button: string, value: number) {
  if (kind === "AxisChanged") {
    const deadzoned = Math.abs(value) < 0.15 ? 0 : value;
    axisState[button] = deadzoned;
    if (deadzoned === 0) return;
  }
  if (kind === "ButtonPressed") {
    justPressed.add(button);
    setTimeout(() => justPressed.delete(button), 100);
    buttonState[button] = true;
    pressHandlers.get(button)?.forEach((fn) => fn());
  } else if (kind === "ButtonReleased") {
    buttonState[button] = false;
    releaseHandlers.get(button)?.forEach((fn) => fn());
  }
}
