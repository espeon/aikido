import { unwrap } from "../../utils";
import { For, Show, createSignal, onMount } from "solid-js";
import { useNavigate, useParams } from "@solidjs/router";
import { IconArrowLeft, IconPlayerPlayFilled, IconBox, IconEdit, IconCheck, IconX } from "@tabler/icons-solidjs";
import { commands } from "../../bindings";
import type { InstanceDetail } from "../../bindings";
import { listen } from "@tauri-apps/api/event";
import { Focusable } from "../../input/Focusable";
import { restoreFocus } from "../../input/focus";
import { showToast } from "../../components/Toast";
import { Spinner } from "../../components/Spinner";


interface LogLine { stream: "Stdout" | "Stderr"; text: string; level: string; }

function loaderLabel(d: InstanceDetail): string {
  const l = d.summary.loader;
  if (l === "Vanilla") return "Vanilla";
  if ("Fabric" in l && l.Fabric) return `Fabric ${l.Fabric.loader_version}`;
  return "?";
}

export default function InstanceDetail() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [detail, setDetail] = createSignal<InstanceDetail | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [launching, setLaunching] = createSignal(false);
  const [progress, setProgress] = createSignal<{ phase: string; percent: number; message: string } | null>(null);
  const [logs, setLogs] = createSignal<LogLine[]>([]);
  const [expandedMod, setExpandedMod] = createSignal<string | null>(null);
  const [editing, setEditing] = createSignal(false);
  const [editName, setEditName] = createSignal("");
  const [editVersion, setEditVersion] = createSignal("");
  const [editMemory, setEditMemory] = createSignal(4096);
  const [editLoader, setEditLoader] = createSignal("vanilla");
  const [editFabricVer, setEditFabricVer] = createSignal("0.16.0");
  const [saving, setSaving] = createSignal(false);
  const [mcVersions, setMcVersions] = createSignal<{ id: string; version_type: string }[]>([]);
  const [fabricVersions, setFabricVersions] = createSignal<{ version: string; stable: boolean }[]>([]);
  const [requiredJava, setRequiredJava] = createSignal<{ majorVersion: number; component: string } | null>(null);

  onMount(async () => {
    try { await refresh(); } finally { setLoading(false); }
    const iid = params.id;
    const unlistenStarted = await listen<string>(`launch:${iid}:started`, () => setProgress({ phase: "preparing", percent: 0, message: "Preparing..." }));
    const unlistenLog = await listen<LogLine>(`launch:${iid}:log`, (e) => setLogs((p) => [...p, e.payload]));
    const unlistenProgress = await listen<{ phase: string; percent: number; message: string }>(`launch:${iid}:progress`, (e) => setProgress(e.payload));
    const unlistenExited = await listen<number>(`launch:${iid}:exited`, () => { setLaunching(false); setProgress(null); refresh(); });
    const unlistenFailed = await listen<string>(`launch:${iid}:failed`, () => { setLaunching(false); setProgress(null); });
    return () => { unlistenStarted(); unlistenLog(); unlistenProgress(); unlistenExited(); unlistenFailed(); };
  });

  async function refresh() {
    try {
      const d = await unwrap(commands.getInstance(params.id));
      setDetail(d);
      try { setRequiredJava(await unwrap(commands.getJavaVersion(d.summary.minecraft_version))); } catch {}
    } catch {}
    setTimeout(() => restoreFocus(), 0);
  }
  async function launch() { setLaunching(true); setLogs([]); setProgress(null); try { await unwrap(commands.launchInstance(params.id)); } catch (e) { setLaunching(false); } }
  async function toggleMod(f: string) { try { await unwrap(commands.toggleMod(params.id, f)); await refresh(); } catch (e) { showToast(String(e), "error"); } }
  async function deleteMod(f: string) { try { await unwrap(commands.deleteMod(params.id, f)); await refresh(); } catch (e) { showToast(String(e), "error"); } }

  async function startEdit(d: InstanceDetail) {
    setEditName(d.summary.name);
    setEditMemory(d.memory_mb);
    setEditLoader(d.summary.loader === "Vanilla" ? "vanilla" : "fabric");
    setEditFabricVer(typeof d.summary.loader === "object" && "Fabric" in d.summary.loader ? (d.summary.loader as { Fabric: { loader_version: string } }).Fabric.loader_version : "0.16.0");
    setEditing(true);
    try {
      const raw = await unwrap(commands.listVersions());
      let list = raw.map(x => ({ id: x.id, version_type: x.type }));
      if (!list.some(x => x.id === d.summary.minecraft_version)) {
        list = [{ id: d.summary.minecraft_version, version_type: "current" }, ...list];
      }
      setMcVersions(list);
      setEditVersion(d.summary.minecraft_version);
    } catch {}
    try {
      const raw = await unwrap(commands.listFabricLoaders(d.summary.minecraft_version));
      let fv = raw;
      const curFv = typeof d.summary.loader === "object" && "Fabric" in d.summary.loader ? (d.summary.loader as { Fabric: { loader_version: string } }).Fabric.loader_version : "0.16.0";
      if (!fv.some(v => v.version === curFv)) {
        fv = [{ version: curFv, stable: true }, ...fv];
      }
      setFabricVersions(fv);
      setEditFabricVer(curFv);
    } catch {}
  }

  async function saveEdit() {
    setSaving(true);
    try {
      await unwrap(commands.updateInstance(params.id, {
        name: editName(),
        minecraft_version: editVersion(),
        memory_mb: editMemory(),
        loader: editLoader(),
        fabric_loader_version: editLoader() === "fabric" ? editFabricVer() : null,
      }));
      setEditing(false);
      await refresh();
    } catch (e) { showToast(String(e), "error"); }
    setSaving(false);
  }

  return (
    <div class="flex flex-1 flex-col overflow-auto bg-background">
      <Show when={!loading()} fallback={<Spinner />}>
        <Show when={detail()} fallback={<p class="p-xl text-muted-foreground">Instance not found.</p>}>
          {(d) => (
            <>
              {/* back arrow */}
              <div class="flex items-center justify-between px-xl pt-lg">
                <button onClick={() => navigate("/")} class="flex items-center gap-sm rounded-lg px-md py-sm text-sm text-muted-foreground transition-colors hover:bg-secondary" aria-label="Back to library">
                  <IconArrowLeft size={18} /> Library
                </button>
                <Focusable id="btn-delete" onActivate={async () => { await unwrap(commands.deleteInstance(params.id)); navigate("/"); }}>
                  <button class="rounded-lg px-md py-sm text-sm text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground">Delete</button>
                </Focusable>
                <Focusable id="btn-edit" onActivate={() => startEdit(d())}>
                  <button onClick={() => startEdit(d())} class="flex items-center gap-sm rounded-lg px-md py-sm text-sm text-muted-foreground transition-colors hover:bg-secondary">
                    <IconEdit size={16} /> Edit
                  </button>
                </Focusable>
              </div>

              {/* edit panel */}
              <Show when={editing()}>
                <div class="mx-xl mt-md flex flex-wrap items-end gap-md rounded-xl border border-accent/30 bg-card p-lg">
                  <div class="flex flex-col gap-xs">
                    <label class="text-xs text-muted-foreground">Name</label>
                    <input value={editName()} onInput={(e) => setEditName(e.currentTarget.value)} class="w-40 rounded-lg border border-input bg-background px-md py-sm text-sm text-foreground" />
                  </div>
                  <div class="flex flex-col gap-xs">
                    <label class="text-xs text-muted-foreground">Version</label>
                    <select value={editVersion()} onChange={(e) => setEditVersion(e.currentTarget.value)} class="rounded-lg border border-input bg-background px-md py-sm text-sm text-foreground">
                      <For each={mcVersions()}>{(v) => <option value={v.id}>{v.id}</option>}</For>
                    </select>
                  </div>
                  <div class="flex flex-col gap-xs">
                    <label class="text-xs text-muted-foreground">Memory</label>
                    <div class="flex items-center gap-sm">
                      <input type="number" value={editMemory()} onInput={(e) => setEditMemory(parseInt(e.currentTarget.value) || 2048)} class="w-24 rounded-lg border border-input bg-background px-md py-sm text-sm text-foreground" />
                      <span class="text-xs text-muted-foreground">MB</span>
                    </div>
                  </div>
                  <div class="flex flex-col gap-xs">
                    <label class="text-xs text-muted-foreground">Loader</label>
                    <select value={editLoader()} onChange={(e) => setEditLoader(e.currentTarget.value)} class="rounded-lg border border-input bg-background px-md py-sm text-sm text-foreground">
                      <option value="vanilla">Vanilla</option>
                      <option value="fabric">Fabric</option>
                    </select>
                  </div>
                  <Show when={editLoader() === "fabric"}>
                    <div class="flex flex-col gap-xs">
                      <label class="text-xs text-muted-foreground">Fabric</label>
                      <select value={editFabricVer()} onChange={(e) => setEditFabricVer(e.currentTarget.value)} class="rounded-lg border border-input bg-background px-md py-sm text-sm text-foreground">
                        <For each={fabricVersions()}>{(v) => <option value={v.version}>{v.version}{v.stable ? "" : " (beta)"}</option>}</For>
                      </select>
                    </div>
                  </Show>
                  <div class="flex gap-sm">
                    <Focusable id="btn-save" onActivate={saveEdit}>
                      <button onClick={saveEdit} disabled={saving()} class="flex items-center gap-sm rounded-lg bg-primary px-lg py-sm text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50">
                        <IconCheck size={16} /> {saving() ? "..." : "Save"}
                      </button>
                    </Focusable>
                    <Focusable id="btn-cancel" onActivate={() => setEditing(false)}>
                      <button onClick={() => setEditing(false)} class="rounded-lg px-md py-sm text-sm text-muted-foreground transition-colors hover:bg-secondary">
                        <IconX size={16} />
                      </button>
                    </Focusable>
                    {requiredJava() && <span class="self-center rounded-full border border-accent/20 px-md py-xs text-xs text-accent">Requires Java {requiredJava()!.majorVersion}</span>}
                  </div>
                </div>
              </Show>

              {/* hero */}
              <div class="relative mx-xl mt-md flex flex-col justify-end overflow-hidden rounded-2xl bg-gradient-to-b from-secondary via-secondary via-70% to-accent/10 p-xl" style={{ "min-height": "280px" }}>
                <div>
                  <h1 class="font-serif text-4xl font-bold tracking-tight text-foreground">{d().summary.name}</h1>
                  <div class="mt-sm flex flex-wrap items-center gap-sm">
                    <span class="rounded-full border border-border/60 px-md py-xs text-xs text-muted-foreground">{d().summary.minecraft_version}</span>
                    <span class="rounded-full border border-accent/30 px-md py-xs text-xs text-accent">{loaderLabel(d())}</span>
                    <span class="rounded-full border border-border/60 px-md py-xs text-xs text-muted-foreground">{d().memory_mb}MB</span>
                    {d().summary.last_played != null && <span class="rounded-full border border-border/60 px-md py-xs text-xs text-muted-foreground">Played {new Date(d().summary.last_played! * 1000).toLocaleDateString()}</span>}
                    {requiredJava() && <span class="rounded-full border border-accent/20 px-md py-xs text-xs text-accent">Java {requiredJava()!.majorVersion}</span>}
                  </div>
                </div>
                <div class="mt-xl flex items-center gap-md">
                  <Focusable id="btn-play" onActivate={launch}>
                    <button onClick={launch} class="flex items-center gap-sm rounded-full bg-primary px-xl py-lg text-base font-semibold text-primary-foreground transition-all hover:bg-primary/90 hover:scale-105 disabled:opacity-50" aria-label="Play">
                      {launching() ? "..." : <><IconPlayerPlayFilled size={20} /> Play</>}
                    </button>
                  </Focusable>
                  <Focusable id="btn-mods-nav" onActivate={() => navigate(`/mods?instance=${params.id}`)}>
                    <button onClick={() => navigate(`/mods?instance=${params.id}`)} class="rounded-full border border-border px-lg py-lg text-sm text-muted-foreground transition-colors hover:bg-secondary">
                      Browse Mods
                    </button>
                  </Focusable>
                </div>
              </div>

              {/* progress bar */}
              <Show when={progress()} keyed>{(p) =>
                <div class="mx-xl mt-md flex items-center gap-md">
                  <span class="text-sm text-muted-foreground">{p.message}</span>
                  <div class="h-2 flex-1 overflow-hidden rounded-full bg-secondary"><div class="h-full rounded-full bg-primary transition-all duration-200" style={{ width: `${p.percent}%` }} /></div>
                  <span class="text-sm text-muted-foreground">{p.percent}%</span>
                </div>
              }</Show>

              {/* log viewer — only when launching */}
              <Show when={logs().length > 0}>
                <div class="mx-xl mt-md flex flex-1 flex-col overflow-hidden rounded-xl border border-border bg-card" style={{ "max-height": "400px" }}>
                  <div class="flex items-center justify-between border-b border-border px-md py-sm">
                    <span class="text-xs font-medium text-muted-foreground">Output</span>
                    <span class="text-xs text-muted-foreground">{logs().length} lines</span>
                  </div>
                  <div class="flex-1 overflow-auto p-md font-mono text-xs leading-relaxed">
                    <For each={logs()}>{(log) => (<div class={log.level === "Error" ? "text-destructive" : log.stream === "Stderr" ? "text-muted-foreground" : "text-foreground"}>{log.text}</div>)}</For>
                  </div>
                </div>
              </Show>

              {/* mods */}
              <div class="mx-xl mb-xl mt-xl">
                <div class="mb-md flex items-center justify-between">
                  <h2 class="text-lg font-semibold">Mods ({d().summary.mod_count})</h2>
                </div>

                <Show when={d().mods.length > 0} fallback={
                  <div class="flex flex-col items-center gap-sm rounded-xl border border-dashed border-border p-xl text-center">
                    <p class="text-muted-foreground">No mods installed yet</p>
                    <button onClick={() => navigate(`/mods?instance=${params.id}`)} class="rounded-full bg-accent px-lg py-sm text-sm font-medium text-accent-foreground transition-colors hover:bg-accent/90">Browse Modrinth</button>
                  </div>
                }>
                  <div class="flex flex-col gap-sm">
                    <For each={d().mods}>
                      {(mod) => (
                        <Focusable id={`mod-${mod.filename}`} onActivate={() => setExpandedMod(expandedMod() === mod.filename ? null : mod.filename)}>
                          <div class="group flex items-center rounded-lg border border-border bg-card p-md transition-colors hover:border-primary/30" onClick={() => setExpandedMod(expandedMod() === mod.filename ? null : mod.filename)}>
                            <div class="mr-md h-10 w-10 shrink-0 overflow-hidden rounded-md bg-secondary">
                              {mod.icon_url ? <img src={mod.icon_url} alt="" class="h-full w-full object-cover" /> : <IconBox size={16} class="m-auto text-muted-foreground" />}
                            </div>
                            <div class="flex flex-1 flex-col gap-xs">
                              <p class={`text-sm font-medium ${mod.disabled ? "text-muted-foreground line-through" : "text-card-foreground"}`}>{mod.name || mod.filename}</p>
                              <p class="text-xs text-muted-foreground">{mod.description || `v${mod.version || "?"}`}</p>
                            </div>
                            <Show when={expandedMod() === mod.filename}>
                              <div class="flex gap-xs pl-md">
                                <button onClick={(e) => { e.stopPropagation(); toggleMod(mod.filename); }} class="rounded-md border border-border px-sm py-xs text-xs text-muted-foreground hover:bg-secondary">{mod.disabled ? "Enable" : "Disable"}</button>
                                <button onClick={(e) => { e.stopPropagation(); deleteMod(mod.filename); }} class="rounded-md border border-border px-sm py-xs text-xs text-muted-foreground hover:bg-destructive hover:text-destructive-foreground">Remove</button>
                              </div>
                            </Show>
                          </div>
                        </Focusable>
                      )}
                    </For>
                  </div>
                </Show>
              </div>
            </>
          )}
        </Show>
      </Show>
    </div>
  );
}
