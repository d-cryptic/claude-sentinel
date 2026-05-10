export function AnnouncementBanner() {
  return (
    <div className="bg-black border-b-2 border-magenta/60 shadow-[0_0_12px_rgba(255,0,255,0.25)] py-2 px-3 sm:px-6 text-center text-xs sm:text-sm font-mono">
      <a
        href="https://github.com/d-cryptic/ccsentinel"
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-2 text-neongreen hover:text-cyan transition-colors uppercase tracking-wider"
      >
        <span aria-hidden="true" className="text-magenta">
          &gt;
        </span>
        <span className="hidden sm:inline">_ ACCOUNT PIPELINE IS LIVE — STAR US ON GITHUB</span>
        <span className="sm:hidden">_ STAR ON GITHUB</span>
        <span aria-hidden="true" className="text-cyan">
          [&uarr;]
        </span>
        <span aria-hidden="true" className="animate-blink text-neongreen">
          &#9613;
        </span>
      </a>
    </div>
  );
}
