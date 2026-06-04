import { For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { IconLayoutGrid, IconPuzzle, IconUser, IconSettings } from "@tabler/icons-solidjs";
import { isPressed, useButton } from "./gamepad";
import { focusedId, moveFocus, registerFocusable, setFocusedId } from "./focus";

const menuItems = [
  { id: "menu-library", label: "Library", icon: IconLayoutGrid, href: "/" },
  { id: "menu-mods", label: "Mods", icon: IconPuzzle, href: "/mods" },
  { id: "menu-accounts", label: "Accounts", icon: IconUser, href: "/accounts" },
  { id: "menu-settings", label: "Settings", icon: IconSettings, href: "/settings" },
] as const;

export let menuOpen = false;

export function MenuOverlay() {
  const [open, setOpen] = createSignal(false);
  const navigate = useNavigate();

  function toggle() { console.log("menu toggle", open()); open() ? close() : open_(); }
  function close() { menuOpen = false; setOpen(false); }
  function open_() { menuOpen = true; setOpen(true); }
  function select(href: string) { close(); setTimeout(() => navigate(href), 250); }

  onMount(() => {
    useButton("Start", toggle);
    useButton("B", () => { if (open()) close(); });
  });

  createEffect(() => {
    if (open()) {
      setTimeout(() => setFocusedId("menu-library"), 150);
    }
  });

  // menu navigation loop when open
  let navTimer: number | undefined;
  createEffect(() => {
    if (open()) {
      let lastNav = 0;
      navTimer = window.setInterval(() => {
        const now = Date.now();
        if (now - lastNav < 250) return;
        if (isPressed("DPadUp") || isPressed("LStickUp")) { moveFocus("up"); lastNav = now; }
        else if (isPressed("DPadDown") || isPressed("LStickDown")) { moveFocus("down"); lastNav = now; }
      }, 80);
    } else {
      if (navTimer) clearInterval(navTimer);
    }
  });

  onCleanup(() => { if (navTimer) clearInterval(navTimer); });

  onMount(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") { toggle(); return; }
      if ((e.metaKey || e.ctrlKey) && e.key === "k") { e.preventDefault(); toggle(); }
    };
    window.addEventListener("keydown", handler);
    onCleanup(() => window.removeEventListener("keydown", handler));
  });

  return (
    <Show when={open()}>
      <div class="fixed inset-0 z-50 bg-black/50 transition-opacity duration-300" style={{ "backdrop-filter": "blur(10px)", "-webkit-backdrop-filter": "blur(10px)" }} onClick={close} />
      <div class="fixed inset-y-0 left-0 z-50 flex w-[360px] flex-col bg-sidebar shadow-2xl animate-slide-in">
        <div class="px-lg pt-xl pb-lg">
          <p class="text-lg font-bold tracking-tight text-sidebar-primary">kidomc</p>
        </div>

        <div class="flex flex-1 flex-col gap-xs px-md">
          <For each={menuItems}>
            {(item) => {
              let btnRef!: HTMLDivElement;
              onMount(() => {
                registerFocusable({ id: item.id, element: btnRef, onActivate: () => select(item.href) });
              });
              return (
                <div
                  id={item.id}
                  ref={btnRef}
                  classList={{
                    "flex h-16 cursor-pointer items-center gap-md rounded-lg px-md transition-all duration-150": true,
                    "bg-secondary ring-1 ring-primary": focusedId() === item.id,
                  }}
                  onClick={() => select(item.href)}
                >
                  <item.icon size={22} class="text-muted-foreground" />
                  <span class="text-lg font-medium text-sidebar-foreground">{item.label}</span>
                </div>
              );
            }}
          </For>
        </div>

        <div class="flex items-center justify-center gap-lg px-lg pb-lg pt-md">
          <span class="flex items-center gap-xs text-xs text-muted-foreground"><span class="text-primary text-sm">Ⓐ</span> select</span>
          <span class="flex items-center gap-xs text-xs text-muted-foreground"><span class="text-primary text-sm">Ⓑ</span> back</span>
        </div>
      </div>

      <style>{`
        @keyframes slide-in { from { transform: translateX(-100%); } to { transform: translateX(0); } }
        .animate-slide-in { animation: slide-in 280ms cubic-bezier(0.22, 1, 0.36, 1) both; }
        @media (prefers-reduced-motion: reduce) {
          .animate-slide-in { animation: none; }
        }
      `}</style>
    </Show>
  );
}
