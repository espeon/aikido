import { createSignal, For } from "solid-js";

export interface Toast {
  id: number;
  message: string;
  type: "error" | "success" | "info";
}

let nextId = 0;
const [toasts, setToasts] = createSignal<Toast[]>([]);

export function showToast(message: string, type: Toast["type"] = "info") {
  const id = nextId++;
  setToasts((prev) => [...prev, { id, message, type }]);
  setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 4000);
}

export function ToastContainer() {
  return (
    <div class="fixed bottom-0 left-0 right-0 z-50 flex flex-col items-center gap-sm pb-xl pointer-events-none">
      <For each={toasts()}>
        {(toast) => (
          <div
            classList={{
              "pointer-events-auto animate-slide-up rounded-lg px-lg py-md text-sm font-medium shadow-lg": true,
              "bg-destructive text-destructive-foreground": toast.type === "error",
              "bg-primary text-primary-foreground": toast.type === "success",
              "bg-secondary text-secondary-foreground": toast.type === "info",
            }}
          >
            {toast.message}
          </div>
        )}
      </For>
      <style>{`
        @keyframes slide-up { from { transform: translateY(100%); opacity: 0; } to { transform: translateY(0); opacity: 1; } }
        .animate-slide-up { animation: slide-up 200ms cubic-bezier(0.22, 1, 0.36, 1) both; }
        @media (prefers-reduced-motion: reduce) { .animate-slide-up { animation: none; } }
      `}</style>
    </div>
  );
}
