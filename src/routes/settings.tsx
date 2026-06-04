import { unwrap } from "../utils";
import { createSignal, onMount } from "solid-js";
import { IconLogout } from "@tabler/icons-solidjs";
import { commands } from "../bindings";
import { Focusable } from "../input/Focusable";
import { restoreFocus } from "../input/focus";


export default function Settings() {
  const [dataDir, setDataDir] = createSignal("");
  onMount(() => { setDataDir(navigator.platform.includes("Mac") ? "~/Library/Application Support/kidomc" : "~/.local/share/kidomc"); setTimeout(() => restoreFocus(), 0); });

  return (
    <div class="flex flex-1 flex-col gap-xl p-xl">
      <h1 class="font-serif text-4xl font-bold tracking-tight text-foreground">Settings</h1>

      <div class="flex flex-col gap-sm">
        <p class="text-sm text-muted-foreground">Version</p>
        <p class="text-foreground">kidomc 0.1.0</p>
      </div>
      <div class="flex flex-col gap-sm">
        <p class="text-sm text-muted-foreground">Data Directory</p>
        <p class="font-mono text-sm text-foreground">{dataDir()}</p>
      </div>
      <div class="flex flex-col gap-sm">
        <p class="text-sm text-muted-foreground">Controls</p>
        <p class="text-foreground">Press Start to open the menu. D-pad to navigate, A to select, B to go back.</p>
      </div>

      <Focusable id="btn-quit" onActivate={async () => { try { await unwrap(commands.quitApp()); } catch {} }}>
        <button onClick={async () => { try { await unwrap(commands.quitApp()); } catch {} }} class="flex items-center gap-sm self-start rounded-xl bg-destructive px-lg py-sm text-sm font-medium text-destructive-foreground transition-colors hover:bg-destructive/90">
          <IconLogout size={18} /> Quit
        </button>
      </Focusable>
    </div>
  );
}
