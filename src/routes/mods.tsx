import { For, Show, createMemo, createResource, createSignal, onMount } from "solid-js";
import { useSearchParams, useNavigate } from "@solidjs/router";
import { IconSearch, IconDownload, IconCircleCheck, IconLoader, IconArrowLeft } from "@tabler/icons-solidjs";
import { commands } from "../bindings";
import type { SearchHit } from "../bindings";
import { Focusable } from "../input/Focusable";
import { registerFocusable, restoreFocus } from "../input/focus";
import { showToast } from "../components/Toast";
import { unwrap } from "../utils";

interface CfHit { id: number; name: string; summary: string; author: string; download_count: number | null; slug: string; logo_url: string | null; allow_mod_distribution: boolean | null; }

export default function ModBrowser() {
  const [searchParams] = useSearchParams<{ instance: string }>();
  const navigate = useNavigate();
  const instanceId = () => searchParams.instance || "";
  const [query, setQuery] = createSignal("");
  const [source, setSource] = createSignal<"modrinth" | "curseforge">("modrinth");
  const [mrResults, setMrResults] = createSignal<SearchHit[]>([]);
  const [cfResults, setCfResults] = createSignal<CfHit[]>([]);
  const [searching, setSearching] = createSignal(false);
  const [searched, setSearched] = createSignal(false);
  const [installing, setInstalling] = createSignal<Set<string>>(new Set());
  const [expandedMod, setExpandedMod] = createSignal<string | null>(null);
  const [cfVersions, setCfVersions] = createSignal<{ id: number; display_name: string; game_versions: string[]; is_available: boolean }[]>([]);
  let searchInput!: HTMLInputElement;
  onMount(() => { if (searchInput) registerFocusable({ id: "search-input", element: searchInput, onActivate: () => searchInput.focus() }); });

  const results = () => source() === "modrinth" ? mrResults() : cfResults();

  const [instance, { refetch: refetchInstance }] = createResource(() => instanceId() || null, async (id) => unwrap(commands.getInstance(id)));
  const mcVersion = () => instance()?.summary.minecraft_version || "1.21.1";
  const loader = () => instance()?.summary.loader === "Vanilla" ? "vanilla" : "fabric";

  const installedMrIds = createMemo(() => {
    const mods = instance()?.mods ?? [];
    return new Set(mods.filter(m => (m.source as { Modrinth?: { project_id: string } }).Modrinth).map(m => (m.source as { Modrinth: { project_id: string } }).Modrinth.project_id));
  });

  const installedCfIds = createMemo(() => {
    const mods = instance()?.mods ?? [];
    return new Set(mods.filter(m => (m.source as { CurseForge?: { project_id: string } }).CurseForge).map(m => parseInt((m.source as { CurseForge: { project_id: string } }).CurseForge.project_id)));
  });

  async function search() {
    if (!query()) return; setSearching(true); setSearched(true);
    try {
      if (source() === "modrinth") setMrResults(await unwrap(commands.searchMods(query(), loader(), mcVersion())));
      else setCfResults(await unwrap(commands.searchCfMods(query(), loader(), mcVersion())));
    } catch (e) { showToast("Search failed", "error"); }
    setSearching(false);
    setTimeout(() => restoreFocus(), 0);
  }

  async function toggleExpandMr(projectId: string) {
    if (expandedMod() === projectId) { setExpandedMod(null); return; }
    setExpandedMod(projectId);
  }

  async function toggleExpandCf(modId: number) {
    const key = `cf-${modId}`;
    if (expandedMod() === key) { setExpandedMod(null); return; }
    setExpandedMod(key);
    try { setCfVersions(await unwrap(commands.getCfFiles(modId))); } catch {}
  }

  async function installMr(projectId: string) {
    if (!instanceId()) return;
    setInstalling((prev) => new Set([...prev, projectId]));
    try {
      const versions = await unwrap(commands.getModVersions(projectId, loader(), mcVersion()));
      const version = versions[0]; if (!version) throw new Error("no version");
      await unwrap(commands.installMod(instanceId(), version.id));
      showToast("Installed", "success"); refetchInstance();
    } catch (e) { showToast(`Failed: ${e}`, "error"); }
    setInstalling((prev) => { const s = new Set(prev); s.delete(projectId); return s; });
  }

  async function installCfVersion(modId: number, fileId: number) {
    if (!instanceId()) return;
    const key = `cf-${modId}`;
    setInstalling((prev) => new Set([...prev, key]));
    try { await unwrap(commands.installCfMod(instanceId(), modId, fileId)); showToast("Installed", "success"); refetchInstance(); }
    catch (e) { showToast(`Failed: ${e}`, "error"); }
    setInstalling((prev) => { const s = new Set(prev); s.delete(key); return s; });
  }

  async function installCf(modId: number) {
    if (!instanceId()) return;
    const key = `cf-${modId}`;
    setInstalling((prev) => new Set([...prev, key]));
    try {
      const files = await unwrap(commands.getCfFiles(modId));
      const file = files.find(f => f.is_available && f.game_versions.some(v => v === mcVersion()));
      if (!file) throw new Error("no compatible version");
      await unwrap(commands.installCfMod(instanceId(), modId, file.id));
      showToast("Installed", "success"); refetchInstance();
    } catch (e) { showToast(`Failed: ${e}`, "error"); }
    setInstalling((prev) => { const s = new Set(prev); s.delete(key); return s; });
  }

  return (
    <div class="flex flex-1 flex-col gap-xl p-xl">
      <div class="flex items-center gap-md">
        <Show when={instanceId()}>
          <button onClick={() => navigate(`/instance/${instanceId()}`)} class="flex items-center gap-sm rounded-lg px-md py-sm text-sm text-muted-foreground transition-colors hover:bg-secondary">
            <IconArrowLeft size={18} /> Back to instance
          </button>
        </Show>
        <h1 class="font-serif text-4xl font-bold tracking-tight text-foreground">Mods</h1>
        <div class="flex-1" />
        <Focusable id="source-switcher" onActivate={() => { const s = document.getElementById("source-select") as HTMLSelectElement; s?.focus(); s?.click(); }}>
          <select id="source-select" value={source()} onChange={(e) => { setSource(e.currentTarget.value as "modrinth" | "curseforge"); setMrResults([]); setCfResults([]); setSearched(false); }} class="rounded-lg border border-input bg-card px-md py-sm text-sm text-foreground">
            <option value="modrinth">Modrinth</option>
            <option value="curseforge">CurseForge</option>
          </select>
        </Focusable>
      </div>

      <div class="flex gap-md">
        <div class="relative flex-1">
          <IconSearch size={18} class="absolute left-md top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none" />
          <input ref={searchInput} value={query()} onInput={(e) => setQuery(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && search()} placeholder="Search mods..." class="w-full rounded-xl border border-input bg-card py-md pl-xl pr-md text-foreground placeholder:text-muted-foreground" />
        </div>
        <Focusable id="btn-search" onActivate={search}>
          <button onClick={search} disabled={searching()} class="flex items-center gap-sm rounded-xl bg-primary px-lg py-md text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50">
            <IconSearch size={18} /> {searching() ? "..." : "Search"}
          </button>
        </Focusable>
      </div>

      <Show when={searched() && results().length === 0}><p class="text-muted-foreground">No mods found for "{query()}".</p></Show>
      <Show when={!searched()}><p class="text-muted-foreground">Search for mods to install.</p></Show>

      <div class="flex flex-col gap-md">
        <For each={source() === "modrinth" ? mrResults() : []}>
          {(hit) => {
            const isInstalled = () => installedMrIds().has(hit.project_id);
            const isInstalling = () => installing().has(hit.project_id);
            return (
              <Focusable id={`mod-${hit.project_id}`}>
                <div class="flex items-start gap-lg rounded-xl border border-border bg-card p-lg transition-colors hover:border-primary/30 cursor-pointer" onClick={() => toggleExpandMr(hit.project_id)}>
                  {hit.icon_url ? <img src={hit.icon_url} alt="" class="h-14 w-14 shrink-0 rounded-xl" /> : <div class="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl bg-secondary text-muted-foreground"><IconDownload size={22} /></div>}
                  <div class="flex flex-1 flex-col gap-xs">
                    <p class="font-semibold text-card-foreground">{hit.title}</p>
                    <p class="text-xs text-muted-foreground">by {hit.author} · {hit.downloads.toLocaleString()} downloads</p>
                    <p class="text-sm text-muted-foreground line-clamp-2">{hit.description}</p>
                    <Show when={expandedMod() === hit.project_id}>
                      <p class="mt-sm text-sm text-muted-foreground">{hit.description}</p>
                      <button onClick={(e) => { e.stopPropagation(); installMr(hit.project_id); }} disabled={isInstalling()} class="mt-sm self-start flex items-center gap-sm rounded-xl bg-accent px-lg py-sm text-sm font-medium text-accent-foreground transition-colors hover:bg-accent/90 disabled:opacity-50">
                        {isInstalling() ? <><IconLoader size={16} class="animate-spin" /> Installing...</> : <><IconDownload size={16} /> Install Latest</>}
                      </button>
                    </Show>
                  </div>
                  <Show when={isInstalled()}><span class="flex shrink-0 items-center gap-sm rounded-xl bg-primary/10 px-lg py-sm text-sm font-medium text-primary"><IconCircleCheck size={16} /> Installed</span></Show>
                  <Show when={!isInstalled() && expandedMod() !== hit.project_id}>
                    <button onClick={(e) => { e.stopPropagation(); installMr(hit.project_id); }} disabled={isInstalling()} class="shrink-0 flex items-center gap-sm rounded-xl bg-accent px-lg py-sm text-sm font-medium text-accent-foreground transition-colors hover:bg-accent/90 disabled:opacity-50">
                      {isInstalling() ? <><IconLoader size={16} class="animate-spin" /> Installing...</> : <><IconDownload size={16} /> Install</>}
                    </button>
                  </Show>
                </div>
              </Focusable>
            );
          }}
        </For>
        <For each={source() === "curseforge" ? cfResults() : []}>
          {(hit: CfHit) => {
            const isInstalled = () => installedCfIds().has(hit.id);
            const key = `cf-${hit.id}`;
            const isInstalling = () => installing().has(key);
            return (
              <Focusable id={`cf-${hit.id}`}>
                <div class="flex items-start gap-lg rounded-xl border border-border bg-card p-lg transition-colors hover:border-primary/30 cursor-pointer" onClick={() => toggleExpandCf(hit.id)}>
                  {hit.logo_url ? <img src={hit.logo_url} alt="" class="h-14 w-14 shrink-0 rounded-xl" /> : <div class="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl bg-secondary text-muted-foreground"><IconDownload size={22} /></div>}
                  <div class="flex flex-1 flex-col gap-xs">
                    <p class="font-semibold text-card-foreground">{hit.name}</p>
                    <p class="text-xs text-muted-foreground">by {hit.author} · {hit.download_count?.toLocaleString()} downloads</p>
                    <p class="text-sm text-muted-foreground line-clamp-2">{hit.summary}</p>
                    <Show when={expandedMod() === key}>
                      <p class="mt-sm text-sm text-muted-foreground">{hit.summary}</p>
                      <Show when={cfVersions().length > 0}>
                        <p class="mt-sm text-xs font-medium text-muted-foreground">Versions:</p>
                        <div class="flex flex-wrap gap-xs mt-xs">
                          <For each={cfVersions().filter(v => v.is_available).slice(0, 8)}>{(fv) => (
                            <button onClick={(e) => { e.stopPropagation(); installCfVersion(hit.id, fv.id); }} class="rounded-md border border-border px-sm py-xs text-xs text-muted-foreground hover:bg-secondary hover:text-foreground transition-colors">{fv.display_name}</button>
                          )}</For>
                        </div>
                      </Show>
                    </Show>
                  </div>
                  <Show when={isInstalled()}><span class="flex shrink-0 items-center gap-sm rounded-xl bg-primary/10 px-lg py-sm text-sm font-medium text-primary"><IconCircleCheck size={16} /> Installed</span></Show>
                  <Show when={!isInstalled() && expandedMod() !== key}>
                    <button onClick={(e) => { e.stopPropagation(); installCf(hit.id); }} disabled={isInstalling()} class="shrink-0 flex items-center gap-sm rounded-xl bg-accent px-lg py-sm text-sm font-medium text-accent-foreground transition-colors hover:bg-accent/90 disabled:opacity-50">
                      {isInstalling() ? <><IconLoader size={16} class="animate-spin" /> Installing...</> : <><IconDownload size={16} /> Install</>}
                    </button>
                  </Show>
                </div>
              </Focusable>
            );
          }}
        </For>
      </div>

      <style>{`
        @keyframes spin { to { transform: rotate(360deg); } }
        .animate-spin { animation: spin 1s linear infinite; }
        @media (prefers-reduced-motion: reduce) { .animate-spin { animation: none; } }
      `}</style>
    </div>
  );
}
