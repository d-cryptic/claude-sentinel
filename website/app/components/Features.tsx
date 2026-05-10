type IconProps = { className?: string };

const icons = {
  profile: (p: IconProps) => (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className={p.className}
      aria-hidden="true"
    >
      <rect
        x="3"
        y="5"
        width="18"
        height="14"
        rx="1"
        stroke="currentColor"
        strokeWidth="2"
      />
      <circle cx="9" cy="11" r="2.2" stroke="currentColor" strokeWidth="2" />
      <path
        d="M5.5 17c.8-2 2.2-3 3.5-3s2.7 1 3.5 3M14 9h5M14 13h4"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  ),
  daemon: (p: IconProps) => (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className={p.className}
      aria-hidden="true"
    >
      <path
        d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
      <circle cx="12" cy="12" r="3.5" stroke="currentColor" strokeWidth="2" />
    </svg>
  ),
  session: (p: IconProps) => (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className={p.className}
      aria-hidden="true"
    >
      <rect
        x="3"
        y="4"
        width="18"
        height="16"
        rx="1"
        stroke="currentColor"
        strokeWidth="2"
      />
      <path
        d="M3 9h18M7 14l2 2 4-4"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  ),
  team: (p: IconProps) => (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className={p.className}
      aria-hidden="true"
    >
      <circle cx="9" cy="9" r="3" stroke="currentColor" strokeWidth="2" />
      <circle cx="17" cy="11" r="2.4" stroke="currentColor" strokeWidth="2" />
      <path
        d="M3.5 19c.8-3 3-4.5 5.5-4.5s4.7 1.5 5.5 4.5M14.5 19c.6-2 2-3 3.5-3s2.9 1 3.5 3"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  ),
  dashboard: (p: IconProps) => (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className={p.className}
      aria-hidden="true"
    >
      <rect
        x="3"
        y="4"
        width="18"
        height="16"
        rx="1"
        stroke="currentColor"
        strokeWidth="2"
      />
      <path
        d="M7 16V12M12 16V8M17 16v-6"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
      />
    </svg>
  ),
  shell: (p: IconProps) => (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className={p.className}
      aria-hidden="true"
    >
      <rect
        x="3"
        y="4"
        width="18"
        height="16"
        rx="1"
        stroke="currentColor"
        strokeWidth="2"
      />
      <path
        d="m7 10 3 2-3 2M13 14h4"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  ),
};

const features: {
  title: string;
  desc: string;
  icon: keyof typeof icons;
}[] = [
  {
    title: "PROFILE MANAGEMENT",
    desc: "Separate OAuth/API accounts per project. Switch instantly with `cst use work`.",
    icon: "profile",
  },
  {
    title: "ACCOUNT PIPELINE",
    desc: "Declare a sequence of profiles with your own usage thresholds. Advance automatically or with `cst next`.",
    icon: "daemon",
  },
  {
    title: "SESSION ISOLATION",
    desc: "Each session gets its own CLAUDE_CONFIG_DIR, project history, and settings.",
    icon: "session",
  },
  {
    title: "TEAM SYNC",
    desc: "Share profile configs via a shared git remote. Onboard teammates in seconds.",
    icon: "team",
  },
  {
    title: "LIVE DASHBOARD",
    desc: "`cst top` gives you an htop-style real-time usage view across all profiles.",
    icon: "dashboard",
  },
  {
    title: "SHELL INTEGRATION",
    desc: "Starship module, tmux segment, Zsh/Fish/Bash hooks. Fits your existing workflow.",
    icon: "shell",
  },
];

function renderDesc(text: string) {
  const parts = text.split(/(`[^`]+`)/g);
  return parts.map((part, i) =>
    part.startsWith("`") && part.endsWith("`") ? (
      <code
        key={i}
        className="font-mono text-[12px] bg-black border border-neongreen/40 text-neongreen rounded-none px-1.5 py-0.5"
      >
        {part.slice(1, -1)}
      </code>
    ) : (
      <span key={i}>{part}</span>
    ),
  );
}

export function Features() {
  return (
    <section
      id="features"
      className="bg-[#0A0A0A] relative overflow-hidden border-t border-border"
    >
      <div className="relative max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-20 sm:py-28">
        <div className="max-w-2xl">
          <p className="eyebrow text-cyan">&gt; CAPABILITIES_</p>
          <h2 className="mt-4 font-vt text-5xl sm:text-6xl lg:text-7xl uppercase text-yellow glow-yellow leading-none">
            {"// FEATURES"}
          </h2>
          <p className="mt-5 font-mono text-sm sm:text-base text-gray-400 leading-relaxed">
            A complete toolkit for managing Claude Code at scale &mdash; built
            for developers who ship.
          </p>
        </div>

        <div className="mt-12 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
          {features.map((f) => {
            const Icon = icons[f.icon];
            return (
              <article
                key={f.title}
                className="group relative bg-card border-2 border-cyan/30 p-6 transition-all duration-300 hover:border-cyan hover:box-glow-cyan"
              >
                <div className="h-12 w-12 border-2 border-cyan text-cyan flex items-center justify-center group-hover:bg-cyan group-hover:text-bg transition-colors">
                  <Icon className="h-6 w-6" />
                </div>
                <h3 className="mt-5 font-mono text-sm font-bold uppercase tracking-wider text-white">
                  {f.title}
                </h3>
                <p className="mt-3 font-mono text-[13px] text-gray-400 leading-relaxed">
                  {renderDesc(f.desc)}
                </p>
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}
