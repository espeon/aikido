import { unwrap } from "../utils";
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { IconUserPlus } from "@tabler/icons-solidjs";
import { commands } from "../bindings";
import type { MicrosoftAccount, DeviceCodeInfo } from "../bindings";
import { listen } from "@tauri-apps/api/event";
import { Focusable } from "../input/Focusable";
import { restoreFocus } from "../input/focus";
import { showToast } from "../components/Toast";


export default function Accounts() {
  const [accounts, setAccounts] = createSignal<MicrosoftAccount[]>([]);
  const [deviceCode, setDeviceCode] = createSignal<DeviceCodeInfo | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [initial, setInitial] = createSignal(true);

  onMount(async () => {
    try { setAccounts(await unwrap(commands.listAccounts())); } catch {}
    setTimeout(() => restoreFocus(), 0);
    setInitial(false);

    const unlistenCompleted = await listen<MicrosoftAccount>("login-completed", (event) => {
      setDeviceCode(null);
      setAccounts((prev) => [...prev, event.payload]); setLoading(false);
    });
    const unlistenFailed = await listen<string>("login-failed", (event) => {
      showToast(event.payload, "error"); setDeviceCode(null); setLoading(false);
    });
    onCleanup(() => { unlistenCompleted(); unlistenFailed(); });
  });

  async function startLogin() {
    setLoading(true);
    try { setDeviceCode(await unwrap(commands.beginMsaLogin())); } catch (e) { showToast(String(e), "error"); setLoading(false); }
  }

  return (
    <div class="flex flex-1 flex-col gap-xl p-xl">
      <div class="flex items-center justify-between">
        <h1 class="font-serif text-4xl font-bold tracking-tight text-foreground">Accounts</h1>
        <Focusable id="btn-login" onActivate={startLogin}>
          <button onClick={startLogin} disabled={loading()} class="flex items-center gap-sm rounded-xl bg-primary px-lg py-sm text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50">
            <IconUserPlus size={18} /> {loading() ? "..." : "Add"}
          </button>
        </Focusable>
      </div>

      <Show when={deviceCode()}>{(dc) =>
        <div class="rounded-xl border border-border bg-card p-xl">
          <p class="font-semibold text-card-foreground">Sign in with Microsoft</p>
          <p class="mt-sm text-muted-foreground">Go to <span class="font-mono text-primary">{dc().verification_uri}</span> and enter:</p>
          <p class="my-lg font-mono text-4xl tracking-widest text-primary">{dc().user_code}</p>
          <p class="text-xs text-muted-foreground">Waiting for sign-in...</p>
        </div>
      }</Show>

      <Show when={accounts().length > 0} fallback={
        <Show when={!initial()}>
          <div class="flex flex-1 flex-col items-center justify-center gap-md rounded-2xl border border-dashed border-border p-xxl text-center">
            <IconUserPlus size={48} class="text-muted-foreground/30" />
            <p class="text-lg font-medium text-muted-foreground">No accounts connected</p>
            <p class="text-sm text-muted-foreground">Sign in with Microsoft to play online</p>
          </div>
        </Show>
      }>
        <div class="flex flex-col gap-md">
          <For each={accounts()}>
            {(account) => (
              <Focusable id={`acct-${account.id}`}>
                <div class="flex items-center justify-between rounded-xl border border-border bg-card p-lg">
                  <div>
                    <p class="font-semibold text-card-foreground">{account.username}</p>
                    <p class="text-xs text-muted-foreground font-mono">{account.uuid}</p>
                  </div>
                  {account.is_default && <span class="rounded-full bg-primary/15 px-lg py-xs text-xs font-medium text-primary">default</span>}
                </div>
              </Focusable>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
