import { createEffect, onCleanup, onMount } from "solid-js";
import { RouteSectionProps, useNavigate } from "@solidjs/router";
import { initGamepad, useButton, isPressed } from "./input/gamepad";
import { activateCurrent, moveFocus, restoreFocus, saveFocus } from "./input/focus";
import { MenuOverlay, menuOpen } from "./input/MenuOverlay";
import { ToastContainer } from "./components/Toast";

export default function App(props: RouteSectionProps) {
  let lastNav = 0;
  let navInterval: number | undefined;
  const navigate = useNavigate();

  onMount(() => {
    initGamepad();

    navInterval = window.setInterval(() => {
      if (menuOpen) return;
      const now = Date.now();
      if (now - lastNav < 200) return;

      const up = isPressed("DPadUp") || isPressed("LStickUp");
      const down = isPressed("DPadDown") || isPressed("LStickDown");
      const left = isPressed("DPadLeft") || isPressed("LStickLeft");
      const right = isPressed("DPadRight") || isPressed("LStickRight");

      if (up) { moveFocus("up"); lastNav = now; }
      else if (down) { moveFocus("down"); lastNav = now; }
      else if (left) { moveFocus("left"); lastNav = now; }
      else if (right) { moveFocus("right"); lastNav = now; }
    }, 80);
  });

  onCleanup(() => clearInterval(navInterval));

  useButton("A", () => activateCurrent());
  useButton("B", () => { if (!menuOpen) { saveFocus(); navigate(-1); } });

  createEffect(() => {
    if (isPressed("A") || isPressed("B")) lastNav = Date.now();
  });

  createEffect(() => {
    void props.children;
    setTimeout(() => restoreFocus(), 250);
  });

  return (
    <div class="flex h-screen bg-background text-foreground">
      <main class="flex flex-1 flex-col overflow-auto">
        {props.children}
      </main>
      <MenuOverlay />
      <ToastContainer />
    </div>
  );
}
