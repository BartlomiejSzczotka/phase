import type { GameFormat } from "../../adapter/types";

interface FormatOption {
  format: GameFormat;
  label: string;
  description: string;
}

interface FormatGroup {
  label: string;
  tone: string;
  formats: FormatOption[];
}

const FORMAT_GROUPS: FormatGroup[] = [
  {
    label: "Constructed",
    tone: "indigo",
    formats: [
      { format: "Standard", label: "Standard", description: "Rotating card pool" },
      { format: "Pioneer", label: "Pioneer", description: "Non-rotating from 2012" },
      { format: "Historic", label: "Historic", description: "Arena's eternal format" },
      { format: "Pauper", label: "Pauper", description: "Commons only" },
    ],
  },
  {
    label: "Commander",
    tone: "amber",
    formats: [
      { format: "Commander", label: "Commander", description: "100-card singleton, 2–4 players" },
      { format: "Brawl", label: "Brawl", description: "60-card Standard singleton" },
      { format: "HistoricBrawl", label: "Historic Brawl", description: "60-card eternal singleton" },
    ],
  },
  {
    label: "Multiplayer",
    tone: "emerald",
    formats: [
      { format: "FreeForAll", label: "Free-for-All", description: "3–6 player battle royale" },
    ],
  },
];

const GROUP_TONES: Record<string, { kicker: string; accent: string; border: string; bg: string; hover: string }> = {
  indigo: {
    kicker: "text-indigo-300/60",
    accent: "bg-indigo-300/70",
    border: "border-white/10",
    bg: "bg-[linear-gradient(180deg,rgba(76,105,255,0.05),rgba(9,13,24,0.80))]",
    hover: "hover:border-white/18 hover:bg-[linear-gradient(180deg,rgba(76,105,255,0.10),rgba(9,13,24,0.88))]",
  },
  amber: {
    kicker: "text-amber-300/60",
    accent: "bg-amber-300/70",
    border: "border-white/10",
    bg: "bg-[linear-gradient(180deg,rgba(255,196,122,0.05),rgba(9,13,24,0.80))]",
    hover: "hover:border-white/18 hover:bg-[linear-gradient(180deg,rgba(255,196,122,0.10),rgba(9,13,24,0.88))]",
  },
  emerald: {
    kicker: "text-emerald-300/60",
    accent: "bg-emerald-300/70",
    border: "border-white/10",
    bg: "bg-[linear-gradient(180deg,rgba(52,211,153,0.05),rgba(9,13,24,0.80))]",
    hover: "hover:border-white/18 hover:bg-[linear-gradient(180deg,rgba(52,211,153,0.10),rgba(9,13,24,0.88))]",
  },
};

interface FormatPickerProps {
  onFormatSelect: (format: GameFormat) => void;
}

export function FormatPicker({ onFormatSelect }: FormatPickerProps) {
  return (
    <div className="flex w-full max-w-3xl flex-col gap-8 px-4">
      {FORMAT_GROUPS.map((group) => {
        const tone = GROUP_TONES[group.tone];
        return (
          <div key={group.label} className="flex flex-col gap-3">
            <span className={`text-[0.68rem] uppercase tracking-[0.22em] ${tone.kicker}`}>
              {group.label}
            </span>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
              {group.formats.map((opt) => (
                <button
                  key={opt.format}
                  onClick={() => onFormatSelect(opt.format)}
                  className={`group relative flex flex-col overflow-hidden rounded-[18px] border px-4 py-4 text-left transition-colors ${tone.border} ${tone.bg} ${tone.hover} cursor-pointer`}
                >
                  <div className={`absolute inset-y-4 left-0 w-[3px] rounded-r ${tone.accent}`} />
                  <div className="text-[1.05rem] font-semibold text-white">
                    {opt.label}
                  </div>
                  <p className="mt-1.5 text-[0.78rem] leading-5 text-slate-400">
                    {opt.description}
                  </p>
                </button>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
