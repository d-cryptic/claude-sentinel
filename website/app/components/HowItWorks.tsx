const steps = [
  {
    n: "01",
    title: "INSTALL",
    desc: "Get the binary and initialize the data directory.",
    cmd: "cargo install cst && cst init",
  },
  {
    n: "02",
    title: "ADD PROFILES",
    desc: "Create profiles for each account. Mix OAuth and API keys freely.",
    cmd: "cst new work --auth oauth",
  },
  {
    n: "03",
    title: "BUILD YOUR PIPELINE",
    desc: "Declare thresholds. The daemon advances to the next account automatically — or run `cst next` yourself.",
    cmd: "cst pipeline configure work",
  },
];

export function HowItWorks() {
  return (
    <section
      id="how"
      className="bg-bg border-y-2 border-neongreen/30 relative overflow-hidden"
    >
      <div className="absolute inset-0 hero-grid opacity-40" aria-hidden="true" />
      <div className="relative max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-20 sm:py-28">
        <div className="max-w-2xl">
          <p className="eyebrow text-magenta">&gt; WORKFLOW_</p>
          <h2 className="mt-4 font-vt text-5xl sm:text-6xl lg:text-7xl uppercase text-neongreen glow-green leading-none">
            {"// HOW IT WORKS"}
          </h2>
          <p className="mt-5 font-mono text-sm sm:text-base text-gray-400 leading-relaxed">
            From zero to full multi-account isolation in under a minute.
          </p>
        </div>

        <ol className="mt-12 grid grid-cols-1 lg:grid-cols-3 gap-6">
          {steps.map((s, i) => (
            <li
              key={s.n}
              className="relative bg-card border-2 border-magenta/30 p-6 sm:p-7 hover:border-magenta hover:box-glow-magenta transition-all duration-300"
            >
              <div className="flex items-start justify-between">
                <div className="font-vt text-7xl sm:text-8xl text-magenta glow-magenta leading-none">
                  {s.n}
                </div>
                {i < steps.length - 1 && (
                  <span
                    aria-hidden="true"
                    className="hidden lg:block text-cyan text-3xl font-mono mt-4"
                  >
                    &gt;&gt;
                  </span>
                )}
              </div>
              <h3 className="mt-4 font-mono text-base font-bold uppercase tracking-widest text-cyan glow-cyan">
                {s.title}
              </h3>
              <p className="mt-3 font-mono text-[13px] text-gray-400 leading-relaxed">
                {s.desc}
              </p>
              <pre className="mt-5 bg-black border border-neongreen/50 p-3 text-[12.5px] font-mono overflow-x-auto">
                <code>
                  <span className="term-cyan">$</span>{" "}
                  <span className="text-neongreen phosphor">{s.cmd}</span>
                </code>
              </pre>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
