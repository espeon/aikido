import { unwrap } from "../utils";
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { useNavigate } from "@solidjs/router";
import {
  IconPlus,
  IconPlayerPlayFilled,
  IconSettings,
  IconBox,
  IconPackageImport,
  IconDots,
  IconTrash,
} from "@tabler/icons-solidjs";
import { commands } from "../bindings";
import type { InstanceSummary, VersionEntry } from "../bindings";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Focusable } from "../input/Focusable";
import { restoreFocus } from "../input/focus";
import { showToast } from "../components/Toast";
import { Spinner } from "../components/Spinner";

function loaderLabel(loader: InstanceSummary["loader"]): string {
  if (loader === "Vanilla") return "Vanilla";
  if ("Fabric" in loader && loader.Fabric)
    return `Fabric ${loader.Fabric.loader_version}`;
  return "?";
}

function loaderColor(loader: InstanceSummary["loader"]): string {
  if (loader === "Vanilla") return "bg-primary/15 text-primary";
  return "bg-accent/15 text-accent";
}


interface LogLine {
  stream: "Stdout" | "Stderr";
  text: string;
  level: string;
}

export default function Home() {
  const navigate = useNavigate();
  const [instances, setInstances] = createSignal<InstanceSummary[]>([]);
  const [versions, setVersions] = createSignal<VersionEntry[]>([]);
  const [showCreate, setShowCreate] = createSignal(false);
  const [newName, setNewName] = createSignal("");
  const [newVersion, setNewVersion] = createSignal("1.21.1");
  const [newMemory, setNewMemory] = createSignal(4096);
  const [newLoader, setNewLoader] = createSignal("vanilla");
  const [progress, setProgress] = createSignal<{
    phase: string;
    percent: number;
    message: string;
  } | null>(null);
  const [logs, setLogs] = createSignal<LogLine[]>([]);
  const [initializing, setInitializing] = createSignal(true);
  const [impProgress, setImpProgress] = createSignal<{
    phase: string;
    current?: number;
    total?: number;
  } | null>(null);
  const [menuOpenId, setMenuOpenId] = createSignal<string | null>(null);

  onMount(async () => {
    await refreshInstances();
    setInitializing(false);
    setInitializing(false);
    try {
      setVersions(await unwrap(commands.listVersions()));
    } catch {}

    const unlistenStarted = await listen<string>("launch-started", () =>
      setProgress({ phase: "preparing", percent: 0, message: "Preparing..." }),
    );
    const unlistenLog = await listen<LogLine>("launch-log", (e) =>
      setLogs((p) => [...p, e.payload]),
    );
    const unlistenProgress = await listen<{
      phase: string;
      percent: number;
      message: string;
    }>("launch-progress", (e) => setProgress(e.payload));
    const unlistenExited = await listen<number>("launch-exited", () => {
      setProgress(null);
      refreshInstances();
    });
    const unlistenFailed = await listen<string>("launch-failed", () => {
      setProgress(null);
    });
    const unlistenImpProgress = await listen<{
      phase: string;
      current?: number;
      total?: number;
    }>("import-progress", (e) => setImpProgress(e.payload));
    const unlistenImpDone = await listen<string>("import-completed", () => {
      setImpProgress(null);
      refreshInstances();
    });
    const unlistenImpFail = await listen<string>("import-failed", (e) => {
      showToast(e.payload, "error");
      setImpProgress(null);
    });

    onCleanup(() => {
      unlistenStarted();
      unlistenLog();
      unlistenProgress();
      unlistenExited();
      unlistenFailed();
      unlistenImpProgress();
      unlistenImpDone();
      unlistenImpFail();
    });
  });

  async function refreshInstances() {
    try {
      setInstances(await unwrap(commands.listInstances()));
    } catch {}
    setTimeout(() => restoreFocus(), 0);
  }
  async function doImport() {
    const file = await open({
      filters: [{ name: "Modpack", extensions: ["mrpack"] }],
      multiple: false,
    });
    if (!file) return;
    const path = file as string;
    try {
      setImpProgress({ phase: "loading" });
      await unwrap(commands.importMrpack(path));
    } catch (e) {
      showToast(String(e), "error");
      setImpProgress(null);
    }
  }

  async function doDelete(id: string) {
    try {
      await unwrap(commands.deleteInstance(id));
      setMenuOpenId(null);
      await refreshInstances();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function launch(id: string) {
    setLogs([]);
    setProgress(null);
    try {
      await unwrap(commands.launchInstance(id));
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function doCreate() {
    try {
      await unwrap(
        commands.createInstance({
          name: newName(),
          minecraft_version: newVersion(),
          memory_mb: newMemory(),
          loader: newLoader(),
          fabric_loader_version: newLoader() === "fabric" ? "0.16.0" : null,
        }),
      );
      setShowCreate(false);
      setNewName("");
      await refreshInstances();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  return (
    <div class="flex flex-1 flex-col gap-xl p-xl">
      <Show when={!initializing()} fallback={<Spinner />}>
        {/* header */}
        <div class="flex items-center justify-between">
          <h1 class="font-serif text-4xl font-bold tracking-tight text-foreground">
            kidomc
          </h1>
          <div class="flex gap-sm">
            <Focusable id="btn-import" onActivate={doImport}>
              <button
                onClick={doImport}
               
                class="flex items-center gap-sm rounded-lg border border-border px-lg py-sm text-sm font-medium text-muted-foreground transition-colors hover:bg-secondary disabled:opacity-50"
              >
                <IconPackageImport size={18} /> Import
              </button>
            </Focusable>
            <Focusable
              id="btn-new"
              onActivate={() => setShowCreate(!showCreate())}
            >
              <button
                onClick={() => setShowCreate(!showCreate())}
               
                class="flex items-center gap-sm rounded-lg bg-primary px-lg py-sm text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
              >
                <IconPlus size={18} /> New
              </button>
            </Focusable>
          </div>
        </div>

        {/* create form */}
        <Show when={showCreate()}>
          <div class="flex flex-col gap-md rounded-xl border border-border bg-card p-lg">
            <p class="text-sm font-semibold text-card-foreground">
              New Instance
            </p>
            <input
              value={newName()}
              onInput={(e) => setNewName(e.currentTarget.value)}
              placeholder="Name"
              class="rounded-lg border border-input bg-background px-md py-sm text-foreground placeholder:text-muted-foreground"
             
            />
            <div class="flex gap-md">
              <select
                value={newVersion()}
                onChange={(e) => setNewVersion(e.currentTarget.value)}
                class="flex-1 rounded-lg border border-input bg-background px-md py-sm text-foreground"
               
              >
                <For each={versions()}>
                  {(v) => (
                    <option value={v.id}>
                      {v.id} ({v.type})
                    </option>
                  )}
                </For>
              </select>
              <select
                value={newLoader()}
                onChange={(e) => setNewLoader(e.currentTarget.value)}
                class="rounded-lg border border-input bg-background px-md py-sm text-foreground"
               
              >
                <option value="vanilla">Vanilla</option>
                <option value="fabric">Fabric</option>
              </select>
              <div class="flex items-center gap-sm">
                <input
                  type="number"
                  value={newMemory()}
                  onInput={(e) =>
                    setNewMemory(parseInt(e.currentTarget.value) || 2048)
                  }
                  class="w-24 rounded-lg border border-input bg-background px-md py-sm text-foreground"
                 
                />
                <span class="text-sm text-muted-foreground">MB</span>
              </div>
            </div>
            <Focusable id="btn-create-confirm" onActivate={doCreate}>
              <button
                onClick={doCreate}
                disabled={!newName()}
                class="self-start rounded-lg bg-accent px-lg py-sm text-sm font-semibold text-accent-foreground transition-colors hover:bg-accent/90 disabled:opacity-50"
              >
                Create
              </button>
            </Focusable>
          </div>
        </Show>

        {/* instances or empty state */}
        <Show
          when={instances().length > 0}
          fallback={
            <div class="flex flex-1 flex-col items-center justify-center gap-lg rounded-2xl border border-dashed border-border p-xxl text-center">
              <IconBox size={48} class="text-muted-foreground/30" />
              <div class="flex flex-col gap-sm">
                <p class="text-lg font-medium text-muted-foreground">
                  Your library is empty
                </p>
                <p class="text-sm text-muted-foreground">
                  Create an instance to start playing
                </p>
              </div>
              <button
                onClick={() => setShowCreate(true)}
                class="rounded-lg bg-primary px-lg py-sm text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90"
              >
                Create Instance
              </button>
            </div>
          }
        >
          <div class="flex flex-col gap-lg">
            <For each={instances()}>
              {(inst) => (
                <Focusable
                  id={`inst-${inst.id}`}
                  onActivate={() => navigate(`/instance/${inst.id}`)}
                >
                  <div
                    class="flex items-center gap-lg rounded-xl border border-border bg-card p-lg transition-all hover:border-primary/30 cursor-pointer"
                    onClick={() => navigate(`/instance/${inst.id}`)}
                  >
                    {/* loader badge */}
                    <div
                      class={`flex h-14 w-14 shrink-0 items-center justify-center rounded-xl ${loaderColor(inst.loader)}`}
                    >
                      <span class="text-lg font-bold">
                        {inst.loader === "Vanilla" ? "V" : "F"}
                      </span>
                    </div>

                    {/* info */}
                    <div class="flex flex-1 flex-col gap-xs">
                      <p class="text-lg font-semibold text-card-foreground">
                        {inst.name}
                      </p>
                      <div class="flex flex-wrap items-center gap-xs">
                        <span class="rounded-full border border-border/60 px-md py-xs text-xs text-muted-foreground">
                          {inst.minecraft_version}
                        </span>
                        <span class="rounded-full border border-border/60 px-md py-xs text-xs text-muted-foreground">
                          {loaderLabel(inst.loader)}
                        </span>
                        {inst.last_played != null && (
                          <span class="text-xs text-muted-foreground">
                            Played{" "}
                            {new Date(
                              inst.last_played! * 1000,
                            ).toLocaleDateString()}
                          </span>
                        )}
                      </div>
                    </div>

                    {/* actions */}
                    <div class="flex items-center gap-sm">
                      <Focusable
                        id={`play-${inst.id}`}
                        onActivate={() => launch(inst.id)}
                      >
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            launch(inst.id);
                          }}
                         
                          class="flex items-center gap-sm rounded-lg bg-primary px-lg py-sm text-sm font-semibold text-primary-foreground transition-all hover:bg-primary/90 disabled:opacity-50"
                        >
                          <IconPlayerPlayFilled size={16} /> Play
                        </button>
                      </Focusable>

                      {/* meatballs menu */}
                      <div class="relative">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setMenuOpenId(
                              menuOpenId() === inst.id ? null : inst.id,
                            );
                          }}
                          class="rounded-lg p-sm text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
                          aria-label="Instance options"
                        >
                          <IconDots size={18} />
                        </button>
                        <Show when={menuOpenId() === inst.id}>
                          <div
                            class="absolute right-0 top-full z-10 mt-xs w-44 overflow-hidden rounded-xl border border-border bg-popover shadow-lg"
                            onClick={(e) => e.stopPropagation()}
                          >
                            <Focusable
                              id={`settings-${inst.id}`}
                              onActivate={() => {
                                setMenuOpenId(null);
                                navigate(`/instance/${inst.id}`);
                              }}
                            >
                              <button
                                onClick={() => {
                                  setMenuOpenId(null);
                                  navigate(`/instance/${inst.id}`);
                                }}
                                class="flex w-full items-center gap-sm px-lg py-sm text-sm text-popover-foreground transition-colors hover:bg-secondary"
                              >
                                <IconSettings size={16} /> Settings
                              </button>
                            </Focusable>
                            <Focusable
                              id={`delete-${inst.id}`}
                              onActivate={() => doDelete(inst.id)}
                            >
                              <button
                                onClick={() => doDelete(inst.id)}
                                class="flex w-full items-center gap-sm px-lg py-sm text-sm text-destructive transition-colors hover:bg-destructive/10"
                              >
                                <IconTrash size={16} /> Delete
                              </button>
                            </Focusable>
                          </div>
                        </Show>
                      </div>
                    </div>
                  </div>
                </Focusable>
              )}
            </For>
          </div>
        </Show>

        {/* progress */}
        <Show when={progress()} keyed>
          {(p) => (
            <div class="flex items-center gap-md">
              <span class="text-sm text-muted-foreground">{p.message}</span>
              <div class="h-2 flex-1 overflow-hidden rounded-full bg-secondary">
                <div
                  class="h-full rounded-full bg-primary transition-all duration-200"
                  style={{ width: `${p.percent}%` }}
                />
              </div>
              <span class="text-sm text-muted-foreground">{p.percent}%</span>
            </div>
          )}
        </Show>

        {/* logs */}
        <Show when={logs().length > 0}>
          <div class="flex flex-1 flex-col overflow-hidden rounded-xl border border-border bg-card max-h-64 min-h-64 h-screen">
            <div class="flex items-center justify-between border-b border-border px-md py-sm min-h-64 h-64">
              <span class="text-xs font-medium text-muted-foreground">
                Output
              </span>
              <span class="text-xs text-muted-foreground">
                {logs().length} lines
              </span>
            </div>
            <div class="flex-1 overflow-auto p-md font-mono text-xs leading-relaxed h-64">
              <For each={logs()}>
                {(log) => (
                  <div
                    class={
                      log.level === "Error"
                        ? "text-destructive"
                        : log.stream === "Stderr"
                          ? "text-muted-foreground"
                          : "text-foreground"
                    }
                  >
                    {log.text}
                  </div>
                )}
              </For>
            </div>
          </div>
        </Show>
        {/* import progress */}
        <Show when={impProgress()}>
          {(p) => (
            <div class="rounded-xl border border-border bg-card p-lg">
              <div class="flex items-center justify-between">
                <span class="text-sm font-medium text-card-foreground capitalize">
                  {p().phase}
                </span>
                {p().current && (
                  <span class="text-sm text-muted-foreground">
                    {p().current}/{p().total}
                  </span>
                )}
              </div>
              {p().current && p().total && (
                <div class="mt-sm h-2 overflow-hidden rounded-full bg-secondary">
                  <div
                    class="h-full rounded-full bg-primary transition-all duration-200"
                    style={{ width: `${(p().current! / p().total!) * 100}%` }}
                  />
                </div>
              )}
            </div>
          )}
        </Show>
      </Show>
    </div>
  );
}
