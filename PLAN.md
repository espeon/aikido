# controller-first minecraft launcher: build plan

a tauri + rust + solid launcher for minecraft java edition, designed controller-first with steam deck as the primary target device. mac and linux only for v1.

this document is structured for incremental implementation. each milestone produces a working, testable artifact. do not skip ahead — earlier milestones de-risk later ones.

---

## 0. project context and constraints

**target platforms:** linux (steam deck primary), macos (apple silicon + intel). no windows for v1.

**mod loaders:** fabric required. neoforge and quilt as stretch goals after core is stable. forge is explicitly out of scope.

**mod sources:** modrinth (primary), curseforge (secondary), local zip/jar drops.

**modpack formats:** `.mrpack` (modrinth), curseforge zip packs.

**design philosophy:**
- controller is a first-class input alongside mouse/keyboard, not a bolted-on afterthought
- every screen must be fully navigable with a gamepad without ever touching the mouse
- focus is always visible. always
- chunky ui suitable for a 7" steam deck screen at 1280x800 viewed at arm's length
- in-game controller support is delivered via auto-installing controlify on new fabric instances

**non-goals for v1:**
- no microsoft account migration flows (just modern msa)
- no minecraft bedrock support
- no skin/cape management
- no in-launcher news/changelog feeds
- no built-in voice chat or social features

---

## 1. high-level architecture

```
┌──────────────────────────────────────────────────────┐
│  frontend (solid + vite + tailwind)                  │
│  - controller-first ui                               │
│  - hand-rolled spatial navigation on signals         │
│  - solid-router for view switching                   │
│  - @tanstack/solid-query for api caching             │
│  - @tanstack/solid-virtual for long lists            │
│  - tauri event subscriber for progress/logs          │
└──────────────────────────────────────────────────────┘
                        ▲ tauri ipc (commands + events)
                        ▼
┌──────────────────────────────────────────────────────┐
│  tauri shell (rust)                                  │
│  - command handlers (thin, delegate to core)         │
│  - event emitters for progress streams               │
│  - gilrs gamepad bridge                              │
└──────────────────────────────────────────────────────┘
                        ▲
                        ▼
┌──────────────────────────────────────────────────────┐
│  launcher-core (rust crate)                          │
│  - instance / auth / manifest / download             │
│  - libraries / assets / jvm / launch / process       │
│  - modloaders (fabric, [neoforge, quilt])            │
└──────────────────────────────────────────────────────┘
                        ▲
                        ▼
┌──────────────────────────────────────────────────────┐
│  launcher-mods (rust crate)                          │
│  - modrinth client                                   │
│  - curseforge client                                 │
│  - mrpack + curseforge zip parsers                   │
│  - local mod validation                              │
└──────────────────────────────────────────────────────┘
```

**workspace layout:**
```
launcher/
├── Cargo.toml                  # workspace root
├── crates/
│   ├── launcher-core/          # pure logic, no tauri deps
│   ├── launcher-mods/          # mod source clients
│   └── launcher-app/           # tauri binary, depends on above
├── frontend/
│   ├── src/
│   ├── package.json
│   └── vite.config.ts
└── README.md
```

keeping `launcher-core` and `launcher-mods` free of tauri dependencies is a hard rule. it means you can write integration tests against them as plain rust, and it forces clean api boundaries.

**frontend stack specifics:**
- solid 1.9+
- `@solidjs/router` for navigation
- `@tanstack/solid-query` for caching modrinth/curseforge api calls
- `@tanstack/solid-virtual` for the log view and long mod lists
- `solid-primitives` for misc utilities (debounce, event listeners, etc)
- `@kobalte/core` for accessible headless primitives (dialog, dropdown, slider)
- tailwind 3.4+ for styling
- typescript strict mode

---

## 2. data model

these are the shapes that flow across the ipc boundary. define them once in rust with `serde`, derive typescript types via `specta` (with `tauri-specta` for full command + event type-safety on both sides).

```rust
// crates/launcher-core/src/model.rs

#[derive(Serialize, Deserialize, Type)]
pub struct InstanceSummary {
    pub id: String,            // ulid
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderInfo,
    pub icon_path: Option<String>,
    pub last_played: Option<DateTime<Utc>>,
    pub play_time_seconds: u64,
    pub mod_count: u32,
}

#[derive(Serialize, Deserialize, Type)]
pub struct InstanceDetail {
    pub summary: InstanceSummary,
    pub path: PathBuf,
    pub memory_mb: MemorySettings,
    pub java_path: Option<PathBuf>,
    pub extra_jvm_args: Vec<String>,
    pub mods: Vec<InstalledMod>,
}

#[derive(Serialize, Deserialize, Type)]
pub enum LoaderInfo {
    Vanilla,
    Fabric { loader_version: String },
    NeoForge { version: String },
    Quilt { loader_version: String },
}

#[derive(Serialize, Deserialize, Type)]
pub struct InstalledMod {
    pub filename: String,
    pub source: ModSource,
    pub mod_id: Option<String>,
    pub version: Option<String>,
    pub sha1: String,
    pub disabled: bool,
}

#[derive(Serialize, Deserialize, Type)]
pub enum ModSource {
    Modrinth { project_id: String, version_id: String },
    CurseForge { project_id: u32, file_id: u32 },
    Local,
}

#[derive(Serialize, Deserialize, Type)]
pub struct Account {
    pub uuid: String,
    pub username: String,
    pub access_token_expires: DateTime<Utc>,
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, Type)]
pub enum LaunchEvent {
    StateChanged(LaunchState),
    Progress { step: String, percent: f32, message: String },
    LogLine { level: LogLevel, text: String },
    Failed { reason: String },
    Exited { code: i32 },
}

#[derive(Serialize, Deserialize, Type)]
pub enum LaunchState {
    Preparing,
    Authenticating,
    DownloadingLibraries,
    DownloadingAssets,
    ExtractingNatives,
    Starting,
    Running,
    Stopped,
}
```

**directory layout (per platform):**
- linux: `$XDG_DATA_HOME/com.yourname.launcher/` (typically `~/.local/share/...`)
- macos: `~/Library/Application Support/com.yourname.launcher/`

inside:
```
<app_data>/
├── instances/
│   └── <instance_id>/
│       ├── instance.json
│       ├── minecraft/         # the actual .minecraft dir for this instance
│       │   ├── mods/
│       │   ├── saves/
│       │   └── ...
│       └── natives/           # extracted per-launch, cleared on exit
├── meta/
│   ├── versions/              # cached mojang version manifests
│   ├── fabric/                # cached fabric metadata
│   └── assets/
│       ├── indexes/
│       └── objects/<2char>/<hash>
├── libraries/                 # content-addressed library cache, shared across instances
├── jvm/                       # downloaded zulu/temurin runtimes
├── accounts.json              # encrypted at rest if possible
└── settings.json
```

key insight from prism: **libraries and assets are shared across instances.** never duplicate them per-instance. instances only have their own `mods/`, `config/`, `saves/`, etc.

---

## 3. milestone plan

each milestone produces a runnable, demoable build. do not move on until the current milestone works end-to-end.

### milestone 1: headless vanilla launch (no ui, no tauri)

**goal:** a rust binary `launch-cli` that takes `--version 1.21.1 --username Player --uuid <fake>` and successfully runs vanilla minecraft. demo-mode launch is fine here so you don't need msa yet.

**why first:** this de-risks every hard part of the launcher in isolation. if this works, the tauri wrapper is the easy half.

**tasks:**

1. set up the cargo workspace. create `launcher-core` with stub modules.

2. **manifest fetching** (`launcher-core::manifest`)
   - fetch `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json`
   - cache locally with etag/last-modified honoring
   - fetch per-version manifest by url from the index
   - parse with serde into typed structs. don't be tempted to use `serde_json::Value` long-term — the version manifest schema is stable enough to be worth typing

3. **download queue** (`launcher-core::download`)
   - `Downloader` struct holding a `reqwest::Client` and a semaphore for concurrency (start with 8 concurrent)
   - `download_with_sha1(url, dest, expected_sha1) -> Result<()>` — validates after write, retries 3x on mismatch or network error
   - resume support via http range requests if `dest.part` exists
   - emit progress via a `tokio::sync::mpsc::Sender<DownloadProgress>` so the caller chooses how to surface it

4. **library resolution** (`launcher-core::libraries`)
   - parse the `libraries` array from the version manifest
   - filter by `rules` field (os name + arch matching). for mac+linux only, you can skip the windows branches but write the rule matcher properly anyway
   - for each library: download the jar to `libraries/<maven-path>/<artifact>.jar`, validate sha1
   - for natives (libraries with a `natives` map): download the os-specific classifier jar, then extract its contents to `<instance>/natives/`, respecting the `extract.exclude` list (typically `META-INF/`)
   - return: full classpath list, native dir path

5. **asset resolution** (`launcher-core::assets`)
   - fetch the asset index from the version manifest's `assetIndex.url`
   - cache it at `meta/assets/indexes/<id>.json`
   - parse the `objects` map: each entry has a hash and size
   - download each object to `meta/assets/objects/<first-2-chars>/<full-hash>`, validate sha1
   - skip objects already present with matching sha1
   - for 1.7.10 and below you'd need the legacy resources/ copy step. skip — out of scope

6. **jvm discovery + download** (`launcher-core::jvm`)
   - check common paths: `/usr/lib/jvm/*`, `/Library/Java/JavaVirtualMachines/*/Contents/Home`, `~/.sdkman/candidates/java/*`
   - parse `java -version` output to determine major version
   - the version manifest tells you which java major you need (`javaVersion.majorVersion`)
   - if no suitable jvm found, download from adoptium api: `https://api.adoptium.net/v3/binary/latest/<major>/ga/<os>/<arch>/jre/hotspot/normal/eclipse`
   - extract to `jvm/temurin-<major>-<os>-<arch>/`, return path to the `java` binary

7. **command line construction** (`launcher-core::launch::args`)
   - build jvm args: `-Xms<min>m -Xmx<max>m`, `-Djava.library.path=<natives>`, classpath, plus the `jvmArgs` from the version manifest (with `${library_directory}` etc. token substitution)
   - on macos: `-XstartOnFirstThread` is required for lwjgl 3
   - the main class comes from the version manifest's `mainClass` field
   - game args: substitute `${auth_player_name}`, `${auth_uuid}`, `${auth_access_token}`, `${version_name}`, `${game_directory}`, `${assets_root}`, `${assets_index_name}`, `${user_type}`, `${version_type}`. for offline/demo mode use placeholder values: token can be `0`, uuid can be a deterministic hash of the username
   - **add `-Dlog4j2.formatMsgNoLookups=true`** always. cheap insurance for older versions

8. **process spawn + log streaming** (`launcher-core::process`)
   - `tokio::process::Command` with `.stdout(Stdio::piped()).stderr(Stdio::piped())`
   - spawn two tasks, one per stream, reading line by line, emitting to an mpsc channel as `LogEvent { stream: Stdout|Stderr, text: String }`
   - parse log4j xml output: lines starting with `<log4j:Event` indicate the start of a structured event. accumulate until `</log4j:Event>`. extract level + logger + message. for non-xml lines, treat as info-level raw output
   - return a `LaunchHandle` with the child process and a receiver for events. caller can `await handle.wait()` for exit

9. **end-to-end test**: `cargo run --bin launch-cli -- --version 1.21.1 --memory 4096`. minecraft window opens, you can play (in demo mode), logs stream to stdout, ctrl-c kills it cleanly.

**acceptance:** vanilla 1.21.1 launches and runs on both a steam deck (or linux dev box) and an apple silicon mac. cold start (nothing cached) downloads everything correctly. warm start uses caches and is fast (<5 sec to spawn).

---

### milestone 2: microsoft authentication

**goal:** working msa device-code login flow producing a valid minecraft access token. still no ui — extend `launch-cli` with `--login` that prints "go to microsoft.com/link, enter code XXXXX" and on completion saves a token.

**why now:** auth is its own beast and easier to nail down before ui complicates it.

**the chain (this trips up everyone, write it down):**

1. **device code request**: POST to `https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode` with `client_id` and `scope=XboxLive.signin offline_access`. response: `device_code`, `user_code`, `verification_uri`, `expires_in`, `interval`.

2. **poll for completion**: POST to `https://login.microsoftonline.com/consumers/oauth2/v2.0/token` every `interval` seconds with `grant_type=urn:ietf:params:oauth:grant-type:device_code&client_id=<id>&device_code=<code>`. you'll get `authorization_pending` until the user completes the flow, then an `access_token` + `refresh_token`.

3. **xbox live auth**: POST `https://user.auth.xboxlive.com/user/authenticate` with `{Properties: {AuthMethod: "RPS", SiteName: "user.auth.xboxlive.com", RpsTicket: "d=<msa_token>"}, RelyingParty: "http://auth.xboxlive.com", TokenType: "JWT"}`. response: `Token` (the xbl token) + `DisplayClaims.xui[0].uhs` (user hash).

4. **xsts auth**: POST `https://xsts.auth.xboxlive.com/xsts/authorize` with `{Properties: {SandboxId: "RETAIL", UserTokens: [<xbl_token>]}, RelyingParty: "rp://api.minecraftservices.com/", TokenType: "JWT"}`. response: `Token` (xsts token) + same `uhs`. handle the documented error codes here: `2148916233` (no xbox account), `2148916238` (under 18 / requires adult), etc — surface them with actionable messages.

5. **minecraft token**: POST `https://api.minecraftservices.com/authentication/login_with_xbox` with `{identityToken: "XBL3.0 x=<uhs>;<xsts_token>"}`. response: `access_token` (the actual mc token), `expires_in`.

6. **ownership check**: GET `https://api.minecraftservices.com/entitlements/mcstore` with `Authorization: Bearer <mc_token>`. items array must contain `product_minecraft` and `game_minecraft`. if missing → user doesn't own java edition.

7. **profile**: GET `https://api.minecraftservices.com/minecraft/profile`. gives you `id` (uuid without dashes) and `name`.

**refresh flow:** when the mc token is near expiry (or returns 401), use the saved msa `refresh_token` to get a new msa access token, then re-run steps 3-7. xbl tokens are short-lived (~24h), mc tokens last ~24h, msa refresh tokens last ~90 days.

**implementation:**

- `launcher-core::auth::msa` — device code + polling + refresh
- `launcher-core::auth::xbox` — xbl + xsts dance
- `launcher-core::auth::minecraft` — mc token + ownership + profile
- `launcher-core::auth::store` — encrypted local storage of refresh tokens. use `keyring` crate on macos (keychain) and secret-service on linux (gnome-keyring / kwallet). fall back to plaintext file with a clear warning if keyring is unavailable (will be common on steam deck in desktop mode)
- `AccountState` enum mirroring prism's: `Working | Online | Offline | Expired | Errored(String)`
- one `MicrosoftAccount` struct holding all the tokens + expiries + profile

**you'll need to register an azure app** to get a `client_id`. for a foss launcher this is your own azure tenant. document this in the readme. the [minecraft auth docs](https://wiki.vg/Microsoft_Authentication_Scheme) and prism's `MSAAccount.cpp` are the references.

**acceptance:** `launch-cli --login` succeeds, refresh token is stored in keyring. subsequent `launch-cli --version 1.21.1` uses the real account, joins a real minecraft server, multiplayer auth works.

---

### milestone 3: tauri shell with minimal solid ui

**goal:** electron-like wrapping. one window, one button "launch latest version with my logged-in account", logs scroll in real time. no controller yet, no instances yet.

**tasks:**

1. **set up tauri 2.x** (use `cargo tauri init` from inside `launcher-app/`). configure `tauri.conf.json` for mac+linux bundles only.

2. **solid frontend scaffolding**
   - `npm create solid@latest frontend -- --template ts` (or use the bare template)
   - install: `@solidjs/router @tanstack/solid-query @tanstack/solid-virtual @kobalte/core @solid-primitives/event-listener @solid-primitives/debounce tailwindcss`
   - typescript strict mode in `tsconfig.json`
   - set up `specta` + `tauri-specta` to generate frontend bindings: rust types with `#[derive(Type)]`, `tauri-specta` writes `frontend/src/bindings.ts` containing typed `commands` and `events` modules. this gives you full type safety across the ipc boundary with zero hand-written ts

3. **first tauri commands** (paired with specta):
   ```rust
   #[tauri::command]
   #[specta::specta]
   async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, AppError>;

   #[tauri::command]
   #[specta::specta]
   async fn begin_msa_login(app: AppHandle, state: State<'_, AppState>) -> Result<DeviceCodeInfo, AppError>;
   // emits 'auth-completed' event when polling finishes

   #[tauri::command]
   #[specta::specta]
   async fn launch_vanilla(
       app: AppHandle,
       version: String,
       memory_mb: u32,
       state: State<'_, AppState>,
   ) -> Result<String, AppError>; // returns launch_id
   ```

4. **event channel pattern** — commands kick off background tasks that emit events on a per-launch channel:
   ```rust
   app.emit(&format!("launch:{launch_id}:event"), LaunchEvent::Progress {...})?;
   ```
   frontend subscribes via specta-typed event listeners. with solid, the natural pattern is a primitive:
   ```ts
   // useLaunchEvents.ts
   export function useLaunchEvents(launchId: Accessor<string | null>) {
     const [events, setEvents] = createSignal<LaunchEvent[]>([]);
     createEffect(() => {
       const id = launchId();
       if (!id) return;
       const unlistenPromise = listen<LaunchEvent>(`launch:${id}:event`, (e) => {
         setEvents(prev => [...prev, e.payload]);
       });
       onCleanup(() => { unlistenPromise.then(fn => fn()); });
     });
     return events;
   }
   ```

5. **error handling pattern** — never panic across the ipc boundary. define `AppError` as a tagged enum with serde, frontend gets structured errors with codes and human messages. solid's built-in `<ErrorBoundary>` catches render errors.

6. **tanstack solid query setup** — wrap the app:
   ```tsx
   const queryClient = new QueryClient();
   render(() => (
     <QueryClientProvider client={queryClient}>
       <Router>
         <App />
       </Router>
     </QueryClientProvider>
   ), root);
   ```
   useful even at this stage for caching the version manifest:
   ```ts
   const manifest = createQuery(() => ({
     queryKey: ['versionManifest'],
     queryFn: () => commands.getVersionManifest(),
   }));
   ```

7. **solid-router setup** — at this stage just two routes:
   ```tsx
   <Router>
     <Route path="/" component={Home} />
     <Route path="/login" component={Login} />
   </Router>
   ```
   we'll grow this in later milestones.

8. **minimal ui**: account picker, version dropdown (just hardcode "latest release" and "latest snapshot" pulled from the manifest), memory slider (use kobalte's slider primitive — accessible by default, will matter for controller later), big launch button, log view.

9. **log view component**: use `@tanstack/solid-virtual` for virtualization from the start. log spam will destroy you otherwise. cap the in-memory buffer at 10k lines, write the full log to a file on disk in `instances/<id>/logs/<timestamp>.log` always. solid's fine-grained reactivity makes this view particularly cheap — just append to a signal, the virtualizer handles render.

**acceptance:** double-click the bundled `.app` or `.AppImage`, log in once, click launch, minecraft runs. close minecraft, the launcher stays open and shows exit code.

---

### milestone 4: instance management

**goal:** create, list, configure, and delete instances. each has its own minecraft version, memory settings, and `.minecraft` directory.

**tasks:**

1. **instance persistence** — `instances/<ulid>/instance.json` with the `InstanceDetail` schema from section 2. always write atomically (write to `.tmp`, rename).

2. **instance crud commands:**
   ```rust
   list_instances() -> Vec<InstanceSummary>
   get_instance(id) -> InstanceDetail
   create_instance(spec: NewInstanceSpec) -> InstanceId
   update_instance_settings(id, settings: InstanceSettings) -> ()
   delete_instance(id) -> ()
   duplicate_instance(id) -> InstanceId
   ```

3. **`NewInstanceSpec`:**
   ```rust
   struct NewInstanceSpec {
       name: String,
       minecraft_version: String,
       loader: LoaderChoice,  // Vanilla | Fabric (others later)
       memory_mb: Option<u32>,
       icon: Option<IconChoice>,
   }
   ```

4. **launch flow refactor** — `launch_instance(id)` replaces `launch_vanilla`. the launch pipeline now loads instance config, sets `game_directory` to the instance's `.minecraft` path, and uses instance memory settings.

5. **router expansion** — solid-router routes for the app shell:
   ```tsx
   <Router>
     <Route path="/" component={InstanceGrid} />
     <Route path="/instance/:id" component={InstanceDetail} />
     <Route path="/instance/:id/settings" component={InstanceSettings} />
     <Route path="/new-instance" component={NewInstanceWizard} />
     <Route path="/accounts" component={AccountsScreen} />
     <Route path="/settings" component={GlobalSettings} />
   </Router>
   ```
   even though there's no url bar, solid-router still gives you clean route definitions, typed params, and `useNavigate`/`useParams` hooks. it also makes the b-button "go back" wiring trivial (`useNavigate()(-1)`).

6. **instance queries with tanstack solid query**:
   ```ts
   const instances = createQuery(() => ({
     queryKey: ['instances'],
     queryFn: () => commands.listInstances(),
   }));
   ```
   mutations for create/update/delete invalidate the `['instances']` key. this gives you optimistic updates and refetch-on-focus for free.

7. **ui screens:**
   - **home / instance grid**: tiles showing icon, name, version, last played. "new instance" tile at the end. controller-friendly grid layout (4 columns at 1280x800)
   - **instance detail**: mods list, settings panel, play button. esc / b button returns to grid
   - **new instance flow**: step 1 pick loader (vanilla / fabric), step 2 pick mc version (filter by loader compatibility), step 3 name + memory, step 4 confirm

8. **icon handling** — let users pick from a built-in set (bundle ~30 svg/png icons) or upload an image. store at `instances/<id>/icon.png`, resized to 256x256.

**acceptance:** create 3 instances on different mc versions, launch each independently, settings persist across launcher restarts, deleting an instance cleans up its directory.

---

### milestone 5: fabric loader support

**goal:** create fabric instances that actually run fabric.

**tasks:**

1. **fabric meta api** (`launcher-core::loaders::fabric`)
   - fetch `https://meta.fabricmc.net/v2/versions/loader/<mc_version>` to list available loader versions
   - for a specific combo: `https://meta.fabricmc.net/v2/versions/loader/<mc_version>/<loader_version>/profile/json` returns a version json delta you merge into the vanilla version manifest
   - the delta adds libraries (intermediary, fabric-loader, asm, etc) and overrides `mainClass` to `net.fabricmc.loader.impl.launch.knot.KnotClient`

2. **version json merging** — the prism approach: load vanilla manifest, apply the fabric delta. concretely: append loader libraries to the libraries list, override mainClass, the rest stays the same. this is much simpler than forge.

3. **mods directory** — instance's `.minecraft/mods/` is created and is where mods drop in. that's it for fabric, no extra setup.

4. **update `NewInstanceSpec` flow** to include fabric loader version selection. default to the latest stable loader.

5. **end-to-end test**: create a fabric 1.21.1 instance, drop fabric-api jar into mods (manually for now), launch, confirm fabric loaded by checking logs for "loading 1 mods".

**acceptance:** fabric instances launch and load mods. multiple mc versions of fabric work side by side.

---

### milestone 6: controller input + spatial navigation

**goal:** the launcher is fully usable with an xbox controller (or steam deck built-in controls) without touching the mouse.

**why now:** with instances and launching working, the ui surface is now big enough to make controller navigation meaningful, and stable enough that the focus-management work won't be thrown away.

**tasks:**

1. **gilrs integration** (`launcher-app::input`)
   - spawn a tokio task at app start that owns a `gilrs::Gilrs`
   - poll events in a loop, emit them to the frontend as a typed tauri event `GamepadInput`
   - event payload: `{ kind: ButtonPressed | ButtonReleased | AxisChanged, button: Button, value: f32, gamepad_id: u32 }`
   - normalize button names to a stable enum: `A`, `B`, `X`, `Y`, `LB`, `RB`, `LT`, `RT`, `Start`, `Select`, `DpadUp`/`Down`/`Left`/`Right`, `LStickUp`/etc, `RStickUp`/etc, `LStickClick`, `RStickClick`. gilrs gives you mostly-consistent mappings but the steam deck reports as a generic xinput device which is fine
   - apply analog stick deadzone (0.2 default) before emitting as logical "dpad" events. include both raw axis events (for ui like scroll) and synthesized button events (for navigation). debounce stick-as-dpad so holding doesn't fire 60x/sec — use 200ms initial delay then 80ms repeat

2. **frontend input layer** (`frontend/src/input/`)
   - global gamepad state primitive built on solid signals:
     ```ts
     // gamepad.ts
     const [buttons, setButtons] = createSignal<Partial<Record<Button, boolean>>>({});
     const [axes, setAxes] = createSignal<Partial<Record<Axis, number>>>({});

     // listeners for "button just pressed" (edge detection in rust side too)
     const pressListeners = new Map<Button, Set<() => void>>();

     export function useButtonPress(button: Button, handler: () => void) {
       onMount(() => {
         if (!pressListeners.has(button)) pressListeners.set(button, new Set());
         pressListeners.get(button)!.add(handler);
       });
       onCleanup(() => pressListeners.get(button)?.delete(handler));
     }

     // initialized once at app start, subscribes to the typed tauri event
     export async function initGamepad() {
       await events.gamepadInput.listen((e) => {
         /* update signals, fire press listeners on rising edge */
       });
     }
     ```
   - the signal-based approach is cleaner than the react context version. components that read `buttons().A` only re-evaluate when that specific button's state changes

3. **hand-rolled spatial navigation** — for ~6 screens this is ~200 lines and gives you total control over focus transitions:
   ```ts
   // focus.ts
   type FocusableNode = {
     id: string;
     element: HTMLElement;
     onActivate?: () => void;
     onBack?: () => void;
     group?: string;  // for "trap focus in this dialog" semantics
   };

   const nodes = new Map<string, FocusableNode>();
   const [focusedId, setFocusedId] = createSignal<string | null>(null);
   const [focusGroup, setFocusGroup] = createSignal<string | null>(null);

   export function registerFocusable(node: FocusableNode) {
     nodes.set(node.id, node);
     onCleanup(() => nodes.delete(node.id));
   }

   export function moveFocus(direction: 'up' | 'down' | 'left' | 'right') {
     const current = focusedId() ? nodes.get(focusedId()!) : null;
     const candidates = [...nodes.values()].filter(n => {
       if (focusGroup() && n.group !== focusGroup()) return false;
       return n !== current;
     });
     const next = findNearestInDirection(current, candidates, direction);
     if (next) {
       setFocusedId(next.id);
       next.element.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
     }
   }

   // weighted distance: primary-axis distance + 2x perpendicular distance,
   // disqualify anything not in the chosen direction at all
   function findNearestInDirection(from, candidates, direction) { /* ... */ }
   ```

   solid component:
   ```tsx
   export function Focusable(props: { id: string; onActivate?: () => void; children: any }) {
     let ref!: HTMLDivElement;
     onMount(() => registerFocusable({ id: props.id, element: ref, onActivate: props.onActivate }));
     const isFocused = createMemo(() => focusedId() === props.id);
     return (
       <div ref={ref} class="focusable" classList={{ focused: isFocused() }} tabindex="-1">
         {props.children}
       </div>
     );
   }
   ```

   wire to gamepad:
   ```ts
   useButtonPress('DpadUp', () => moveFocus('up'));
   useButtonPress('DpadDown', () => moveFocus('down'));
   useButtonPress('A', () => {
     const node = nodes.get(focusedId()!);
     node?.onActivate?.();
   });
   useButtonPress('B', () => {
     const node = nodes.get(focusedId()!);
     if (node?.onBack) node.onBack();
     else navigate(-1);  // solid-router pop
   });
   ```

4. **focus styling** — design a single `.focused` style: thick outline (3px), high-contrast (use a brand accent color), slight scale (1.03), shadow. this is the most important visual element in the entire ui. make it animate (150ms) but not bouncy.

5. **button hint bar** — bottom-of-screen component, contextual per route:
   ```tsx
   <ButtonHints>
     <Hint button="A" label="Select" />
     <Hint button="B" label="Back" />
     <Hint button="X" label="Install" />
     <Hint button="Y" label="Details" />
   </ButtonHints>
   ```
   each screen declares its hints via a primitive that pushes/pops from a global signal. render with platform-appropriate button glyphs (you'll want both xbox abxy and switch abxy glyphs — bundle both, swap based on detected controller).

6. **on-screen keyboard** — kobalte's `Dialog` for the modal, custom content. takes over when a text input is focused and the user presses A. grid layout, navigable with dpad, A to type, X to space, Y to backspace, R-trigger to shift, start to confirm. autofocuses the right text field on dismiss.

7. **mouse + keyboard parity** — every focusable element is also a normal click target. tab navigation works (solid handles native focus on the wrapped div via tabindex). esc maps to back. enter maps to A. this is non-negotiable.

8. **steam deck testing** — test in both gaming mode (controller as primary) and desktop mode (keyboard available). gaming mode is the real target.

**acceptance:** boot the launcher on a steam deck in gaming mode, never touch the trackpad/screen, complete a full flow: log in via msa → create a new instance → launch it → return to launcher. button hint bar always shows accurate hints. focus is never invisible.

---

### milestone 7: modrinth integration

**goal:** browse and install mods from modrinth without leaving the launcher.

**tasks:**

1. **modrinth client** (`launcher-mods::modrinth`)
   - base url `https://api.modrinth.com/v2`
   - no auth required for reads, set a `User-Agent` header per their api guidelines (something like `yourname/launcher/0.1.0 (contact@email)`)
   - endpoints:
     - `GET /search?query=&facets=` — search with filters (loader, game_version, categories)
     - `GET /project/<id_or_slug>` — full project info
     - `GET /project/<id>/version` — list versions, filter by `loaders[]=fabric&game_versions[]=1.21.1`
     - `GET /version/<id>` — single version with `files[]` array containing url, filename, hashes
   - typed structs for `SearchResponse`, `Project`, `Version`, `VersionFile`

2. **frontend caching with tanstack solid query** — this is where solid-query starts paying real dividends:
   ```ts
   const debouncedQuery = createMemo(() => debouncedSignal(query()));
   const searchResults = createQuery(() => ({
     queryKey: ['modrinth', 'search', debouncedQuery(), filters()],
     queryFn: () => commands.searchModrinth(debouncedQuery(), filters()),
     staleTime: 5 * 60 * 1000,  // 5min — search results are stable enough
   }));

   const projectDetails = createQuery(() => ({
     queryKey: ['modrinth', 'project', projectId()],
     queryFn: () => commands.getModrinthProject(projectId()),
     enabled: !!projectId(),
   }));
   ```
   use `@solid-primitives/debounce` for the search input.

3. **mod install flow**
   - given a `(project_id, version_id)` and an instance: GET the version, pick the primary file (or only file), download to `instances/<id>/.minecraft/mods/<filename>`, validate sha1 from the version metadata
   - update `instance.json` to record the installed mod with full `ModSource` info
   - check dependencies: modrinth versions have a `dependencies` array. for `required` deps, recursively resolve and install. for `optional`, surface to user
   - on install success, invalidate `['instance', id]` query so the mods list re-fetches

4. **ui: mod browser**
   - reached from instance detail → "browse mods" button
   - search bar at top (uses on-screen keyboard for controller users)
   - filter chips for category (tech, magic, qol, etc) and sort (relevance, downloads, updated)
   - results as a vertical list of cards: icon, title, author, short description, download count. virtualize with `@tanstack/solid-virtual`
   - A button on a result → opens detail pane with full description, screenshots, version list
   - X button → quick install latest compatible version
   - automatically filter by the instance's loader and mc version

5. **installed mods view** — instance detail's mods tab shows installed mods with: name, version, source (modrinth/curseforge/local), enabled toggle (renames `.jar` ↔ `.jar.disabled`), uninstall button.

6. **update check** — background task on instance open: for each modrinth-sourced mod, check if a newer version exists for the same mc version + loader. show a badge. this is just another `createQuery` with infrequent refetch.

**acceptance:** create a fabric 1.21.1 instance, search for "sodium" in the mod browser, install it with one button, launch the instance, confirm sodium loaded. install a mod with dependencies (e.g. one needing fabric-api), see deps install too.

---

### milestone 8: modpack import (.mrpack)

**goal:** drag a `.mrpack` onto the launcher (or pick via file picker), get a fully configured instance ready to launch.

**tasks:**

1. **mrpack parser** (`launcher-mods::modpack::modrinth`)
   - a `.mrpack` is just a zip with:
     - `modrinth.index.json` — the manifest
     - `overrides/` — files to copy verbatim into `.minecraft/`
     - `client-overrides/` — same but only for client (use these)
     - `server-overrides/` — ignore
   - parse the index. relevant fields:
     ```json
     {
       "formatVersion": 1,
       "game": "minecraft",
       "versionId": "1.0.0",
       "name": "Cool Modpack",
       "files": [
         {
           "path": "mods/sodium.jar",
           "hashes": { "sha1": "...", "sha512": "..." },
           "downloads": ["https://cdn.modrinth.com/..."],
           "fileSize": 12345,
           "env": { "client": "required", "server": "optional" }
         }
       ],
       "dependencies": {
         "minecraft": "1.21.1",
         "fabric-loader": "0.16.0"
       }
     }
     ```

2. **import flow**
   - user picks a `.mrpack` file (or drags onto window — tauri 2 supports drag-and-drop, expose to solid via the file-drop event listener)
   - parse index, show preview: name, mc version, loader, mod count, "import" button
   - on confirm: create instance with the right loader/version, download all `files[]` with `env.client != "unsupported"` to their `path`, extract `overrides/` + `client-overrides/` over `.minecraft/`
   - track all files in `instance.json` as modrinth-sourced for future updates
   - **whitelisted download domains only** — modrinth's spec restricts urls to `cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com`, `gitlab.com`, `cdn.azuriom.net`, plus a few others. enforce this for security — refuse to download from anywhere else and surface a clear error

3. **progress ui** — full-screen import progress with phase indicators (downloading 23/47 mods... extracting overrides...). drive from the launch event stream pattern.

**acceptance:** download fabulously.optimized's or simply optimized's `.mrpack` from modrinth, import it, launch the resulting instance. it just works.

---

### milestone 9: in-game controller support via controlify

**goal:** when creating a new fabric instance, offer a "controller-friendly" toggle that auto-installs controlify so the resulting game is playable with a gamepad.

**tasks:**

1. add a checkbox/toggle to the new instance flow: "make this controller-friendly (recommended for steam deck)". on by default if loader is fabric.

2. when enabled, immediately after instance creation, install:
   - **fabric-api** (required by controlify)
   - **controlify** itself
   - both fetched via the modrinth client with the appropriate mc version

3. show a one-time tip card on first launch of a controller-friendly instance: "press F8 in-game to open controlify settings."

4. **note for the user** in the readme: controlify is the third-party mod doing the actual work. credit them prominently.

**acceptance:** create a controller-friendly fabric instance, launch it, plug in a controller, the title screen is navigable with the controller without further config.

---

### milestone 10: curseforge integration

**goal:** mod browsing/install from curseforge, and curseforge zip modpack import.

**why last:** the api key requirement and the "third-party download opt-out" behavior make this the most operationally painful integration. better to ship without it than block on it.

**tasks:**

1. **register for a curseforge api key** at https://console.curseforge.com. document the process. for an open-source project, the key gets baked in but should be loaded from an env var at build time so forks use their own.

2. **curseforge client** (`launcher-mods::curseforge`)
   - base url `https://api.curseforge.com`
   - header `x-api-key: <key>` on every request
   - relevant endpoints (game id 432 = minecraft, class id 6 = mods):
     - `GET /v1/mods/search?gameId=432&classId=6&searchFilter=&gameVersion=&modLoaderType=` (4 = fabric, 5 = quilt, 6 = neoforge — verify against current docs, ids drift)
     - `GET /v1/mods/<id>` — mod details
     - `GET /v1/mods/<id>/files` — files (versions) for a mod
     - `GET /v1/mods/<id>/files/<file_id>/download-url` — actual download url (may fail if mod opted out)

3. **handle the third-party-download opt-out** — `allowModDistribution: false` on a file means the api won't give you a url. ux: surface a clear message "this mod's author has disabled third-party downloads. open in browser to download manually, then drag-and-drop onto this instance." with a button to do exactly that.

4. **curseforge modpack zip parser** (`launcher-mods::modpack::curseforge`)
   - cf modpacks: zip with `manifest.json` + `overrides/` folder
   - manifest schema:
     ```json
     {
       "minecraft": { "version": "1.21.1", "modLoaders": [{"id": "fabric-0.16.0", "primary": true}] },
       "files": [{ "projectID": 306612, "fileID": 5234567, "required": true }],
       "name": "Pack name",
       "version": "1.0.0",
       "overrides": "overrides"
     }
     ```
   - for each `files[]`: resolve via curseforge api to get download url, download to mods/
   - extract `overrides/` into `.minecraft/`
   - same opt-out handling as individual mods — pause import, prompt user to provide the file manually

5. **unify mod browser** — the mod browser ui gains source tabs at the top: `[modrinth] [curseforge]`. search/install flows are otherwise identical (different queryKey prefix, same component shape).

**acceptance:** install a mod from curseforge, import a cf modpack with mostly-cooperative mods, gracefully handle a pack with one or two opted-out mods.

---

## 4. cross-cutting concerns

### error handling

- **never panic across ipc.** every tauri command returns `Result<T, AppError>` where `AppError` is a serializable enum with structured variants:
  ```rust
  enum AppError {
      Auth(AuthError),
      Download { url: String, source_msg: String },
      InstanceNotFound(String),
      ProcessSpawn(String),
      ManifestParse(String),
      Io(String),
  }
  ```
- solid `<ErrorBoundary>` catches render errors. ipc errors get displayed via kobalte toast with a "copy details" button. detailed logs always go to disk regardless.

### logging

- backend: `tracing` + `tracing-subscriber` with a rolling file appender at `<app_data>/logs/launcher-<date>.log`. log level info by default, debug behind a setting.
- frontend: development uses console, production sends warnings/errors to backend via a `log_frontend_event` command.
- every launch attempt gets its own log file at `instances/<id>/logs/launch-<timestamp>.log` containing both launcher events and game output.

### testing

- **unit tests** for: manifest parsing (fixtures of real mojang manifests), library rule matching (test all permutations of os/arch), download retry logic (mockito), auth token parsing.
- **integration tests** for launcher-core: a `tests/` directory that does the full vanilla download + spawn against a clean temp dir. expensive — gate behind `--ignored` or a feature flag, run in ci on a schedule not per-pr.
- **frontend tests**: `@solidjs/testing-library` + vitest for component tests on critical surfaces (focus manager, install flow). skip e2e for v1 — not worth the maintenance for a desktop app. manual testing on macos + steam deck per milestone.

### performance budgets

- cold launch (no caches): < 60s to minecraft window for vanilla, network-bound.
- warm launch (cached): < 5s from button click to jvm spawn.
- ui frame time: < 16ms on steam deck. solid's fine-grained reactivity makes this much easier than react would have been — rarely need explicit memoization, just write idiomatic solid (read signals inside `createMemo`/jsx, never destructure them early).
- memory: launcher itself < 250mb resident (solid + smaller bundle helps here vs react). minecraft will eat much more, separately.

### security

- **tauri csp**: lock down `default-src 'self'` in `tauri.conf.json`. only allowlist the domains you actually fetch from in the frontend (modrinth cdn for icons, basically).
- **command allowlist**: only export the tauri commands you actually need. no shell command exposed to frontend.
- **token storage**: keychain on mac, secret-service on linux. plaintext fallback only with explicit user consent.
- **download url validation**: for mrpack/cf-pack imports, enforce domain whitelists strictly.
- **never log tokens.** filter `access_token`, `refresh_token`, `identityToken` from any structured logging.

### accessibility

controller-first ui is already a big a11y win, but go further:
- support system text size scaling (read from os, scale tailwind base font size)
- high contrast mode toggle in settings
- never convey information by color alone — pair with icons or text
- the on-screen keyboard supports both controller and touch (steam deck has a touchscreen)
- kobalte primitives handle aria attributes correctly by default — lean on them

---

## 5. design system reference (for the frontend)

establish these early, in a single `frontend/src/design/tokens.ts`:

```ts
export const tokens = {
  spacing: { xs: 4, sm: 8, md: 16, lg: 24, xl: 40, xxl: 64 },
  radius: { sm: 6, md: 12, lg: 20, pill: 999 },
  duration: { fast: 120, normal: 200, slow: 320 },

  // focus is the single most important visual primitive
  focus: {
    ringWidth: 3,
    ringColor: 'var(--accent)',
    ringOffset: 2,
    scale: 1.03,
    shadow: '0 0 0 4px rgba(0,0,0,0.15), 0 8px 24px rgba(0,0,0,0.2)',
  },
};
```

tailwind config extends with these tokens. components consume tailwind utilities; tokens stay the source of truth.

**typography:** body 18px at 1280x800 (the steam deck native res). that's chunky for desktop but right for the device. system font stack with `Inter` or similar as the brand preference.

**color:** dark theme by default (most launchers are dark, controller users often play in low light). a single accent color that drives focus rings and primary buttons. keep contrast ratios at wcag aa minimum.

**button glyphs:** bundle both xbox (a/b/x/y green/red/blue/yellow circles) and switch (a/b/x/y mapped differently) glyph sets as svg. detect controller type via gilrs's `gamepad.name()` and swap.

---

## 6. open questions to resolve as you build

these aren't blockers but you'll want answers eventually:

- **multi-account support**: how to switch between accounts mid-session? simple dropdown is fine for v1.
- **instance import/export**: useful for sharing setups but not v1.
- **prism instance import**: nice migration path but real work. consider after curseforge.
- **theming**: let users customize accent color? probably yes, simple hue picker.
- **automatic launcher updates**: tauri has an updater plugin. set it up but don't ship updates until you have a backend signing infrastructure.
- **crash reporting**: sentry is overkill, but a local crash log + "report this" button that opens a github issue with logs is reasonable.

---

## 7. references

- prism launcher source (especially `LaunchController.cpp`, `MinecraftInstance.cpp`, `MSAAccount.cpp`)
- mojang version manifest v2: `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json`
- microsoft auth flow: `https://wiki.vg/Microsoft_Authentication_Scheme`
- fabric meta api: `https://meta.fabricmc.net/`
- modrinth api docs: `https://docs.modrinth.com/`
- curseforge api docs: `https://docs.curseforge.com/`
- mrpack format spec: `https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack`
- solid docs: `https://docs.solidjs.com/`
- solid-router: `https://docs.solidjs.com/solid-router`
- tanstack solid-query: `https://tanstack.com/query/latest/docs/framework/solid/overview`
- tanstack solid-virtual: `https://tanstack.com/virtual/latest/docs/framework/solid/solid-virtual`
- kobalte: `https://kobalte.dev/`
- gilrs: `https://docs.rs/gilrs`
- tauri 2 + specta: `https://github.com/oscartbeaumont/tauri-specta`

---

## handoff notes for claude code

implement milestones in order. after each milestone, run through the acceptance test manually before starting the next. when blocked or uncertain about a design decision not specified here, prefer:

1. matching prism's behavior (it's battle-tested)
2. keeping `launcher-core` and `launcher-mods` free of tauri/ui dependencies
3. controller usability over visual cleverness
4. correctness over performance (you can profile later)
5. shipping milestone n before perfecting milestone n-1

when adding dependencies, prefer the rust ecosystem standards: `reqwest`, `tokio`, `serde`, `tracing`, `thiserror`, `anyhow` (binaries only, not libs), `clap` for the cli tools, `chrono` for time, `ulid` for ids.

see `AGENTS.md` at the repo root for solid-specific patterns, common pitfalls, commit conventions, and what counts as "done" for each milestone.
