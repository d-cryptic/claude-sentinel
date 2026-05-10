import { CopyCommand } from "./CopyCommand";
import { TerminalCard } from "./TerminalCard";

const REPO = "https://github.com/d-cryptic/ccsentinel";

export function Hero() {
  return (
    <section className="relative bg-bg overflow-hidden -mt-16 pt-16 min-h-screen flex items-center">
      {/* Layered backgrounds */}
      <div className="absolute inset-0 hero-grid" aria-hidden="true" />
      <div className="absolute inset-0 horizon-glow" aria-hidden="true" />
      <div
        className="absolute inset-0 perspective-grid opacity-30"
        aria-hidden="true"
      />
      {/* Top fade */}
      <div
        className="absolute inset-x-0 top-0 h-40 bg-gradient-to-b from-bg to-transparent"
        aria-hidden="true"
      />

      <div className="relative z-10 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-16 sm:py-24 w-full">
        <div className="text-center max-w-4xl mx-auto">
          <p className="font-mono text-xs sm:text-sm text-cyan uppercase tracking-widest">
            &gt; INTELLIGENT ACCOUNT MANAGER_
            <span className="animate-blink">&#9613;</span>
          </p>

          <h1 className="mt-6 font-vt text-6xl sm:text-7xl md:text-8xl lg:text-9xl leading-[0.95] tracking-tight text-white glow-cyan uppercase">
            Switch Claude
            <br />
            accounts
            <br />
            <span className="text-magenta glow-magenta">instantly</span>
          </h1>

          <p className="mt-8 font-mono text-sm sm:text-base text-gray-400 max-w-2xl mx-auto leading-relaxed">
            Manage all your Claude Code accounts, profiles, and sessions in one
            place. Build a pipeline between accounts &mdash; switch context
            with a single command or let the daemon advance automatically.
          </p>

          <div className="mt-10 flex flex-col sm:flex-row items-stretch sm:items-center justify-center gap-4">
            <a
              href={REPO}
              target="_blank"
              rel="noopener noreferrer"
              className="btn-retro-fill"
            >
              Install Now &gt;
            </a>
            <a
              href={REPO}
              target="_blank"
              rel="noopener noreferrer"
              className="btn-retro"
            >
              View GitHub
            </a>
          </div>

          <div className="mt-8 flex justify-center">
            <CopyCommand command="cargo install cst" />
          </div>
        </div>

        <div className="mt-16 sm:mt-20 relative animate-float">
          <TerminalCard />
          <div
            className="absolute -inset-x-12 -bottom-10 h-40 blur-3xl bg-magenta/20 rounded-full -z-10"
            aria-hidden="true"
          />
          <div
            className="absolute -inset-x-8 -top-6 h-24 blur-3xl bg-cyan/20 rounded-full -z-10"
            aria-hidden="true"
          />
        </div>
      </div>
    </section>
  );
}
