export function Spinner() {
  return (
    <div class="flex items-center justify-center p-xl" aria-label="Loading">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-muted border-t-primary" />
      <style>{`
        @keyframes spin { to { transform: rotate(360deg); } }
        .animate-spin { animation: spin 0.6s linear infinite; }
        @media (prefers-reduced-motion: reduce) { .animate-spin { animation: none; } }
      `}</style>
    </div>
  );
}
