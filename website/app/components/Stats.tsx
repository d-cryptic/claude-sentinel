const stats = [
  { value: "5X", label: "FASTER CONTEXT SWITCHING" },
  { value: "∞", label: "PIPELINE STAGES" },
  { value: "100%", label: "CLAUDE CODE COMPATIBLE" },
];

export function Stats() {
  return (
    <section className="bg-bg border-y-2 border-magenta/40 relative overflow-hidden">
      <div
        className="absolute inset-0 opacity-30 hero-grid"
        aria-hidden="true"
      />
      <div className="relative max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-16 sm:py-20">
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-6 sm:gap-8">
          {stats.map((s) => (
            <div
              key={s.label}
              className="bg-card neon-border-magenta px-6 py-8 text-center flex flex-col items-center"
            >
              <div className="font-vt text-7xl sm:text-8xl text-magenta glow-magenta leading-none">
                {s.value}
              </div>
              <div className="mt-4 font-mono text-[11px] sm:text-xs text-cyan uppercase tracking-widest">
                {s.label}
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
