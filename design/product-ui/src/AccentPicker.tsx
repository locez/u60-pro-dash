import { useEffect, useId, useState, type CSSProperties } from "react";
import { Check } from "lucide-react";

export const accentColors = [
  { name: "蓝色", value: "#2563eb" },
  { name: "青色", value: "#0891b2" },
  { name: "绿色", value: "#258052" },
  { name: "橙色", value: "#d97706" },
  { name: "玫红", value: "#db3d70" },
  { name: "灰色", value: "#64748b" },
];
export const validAccent = (value: string) => /^#[0-9a-f]{6}$/i.test(value);
const preferenceKey = "u60-design-screen-accent";
export function readScreenAccent() {
  try {
    const color = localStorage.getItem(preferenceKey) || "";
    if (validAccent(color)) return color.toLowerCase();
  } catch {
    /* The preview also works with storage disabled. */
  }
  return accentColors[0].value;
}
export function rememberScreenAccent(color: string) {
  if (!validAccent(color)) return;
  try {
    localStorage.setItem(preferenceKey, color);
  } catch {
    /* Optional preference. */
  }
}
type RGB = [number, number, number];
const rgb = (color: string): RGB =>
  [1, 3, 5].map((i) => parseInt(color.slice(i, i + 2), 16)) as RGB;
function luminance(color: RGB) {
  const linear = color.map((channel) => {
    const n = channel / 255;
    return n <= 0.04045 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4;
  });
  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
}
function contrast(a: RGB, b: RGB) {
  const x = luminance(a),
    y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
export function accentForeground(color: string) {
  const c = rgb(color);
  return contrast(c, [255, 255, 255]) >= contrast(c, [0, 0, 0])
    ? "#ffffff"
    : "#000000";
}
export function screenAccentStyle(value: string, dark = true): CSSProperties {
  const color = validAccent(value) ? value : accentColors[0].value;
  const chosen = rgb(color);
  let ink = chosen;
  // Match the native preference: preserve the chosen fill, lift text until it
  // is legible against every surface of the selected theme.
  const background: RGB = dark ? [41, 55, 76] : [234, 240, 247];
  for (let n = 0; n < 32 && contrast(ink, background) < 4.5; n++) {
    ink = ink.map((channel) =>
      dark ? Math.min(255, channel + 8) : Math.max(0, channel - 8),
    ) as RGB;
  }
  return {
    "--screen-accent": color,
    "--screen-accent-ink": `rgb(${ink.join(" ")})`,
    "--screen-accent-on": accentForeground(color),
    "--screen-accent-soft": `rgb(${chosen.join(" ")} / 0.15)`,
  } as CSSProperties;
}
export default function AccentPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  const id = useId();
  useEffect(() => setDraft(value), [value]);
  const change = (next: string) => {
    setDraft(next);
    if (validAccent(next)) onChange(next.toLowerCase());
  };
  return (
    <fieldset className="accent-picker">
      <legend>强调色</legend>
      <div className="accent-swatches" aria-label="预设强调色">
        {accentColors.map((color) => (
          <button
            type="button"
            key={color.value}
            aria-label={color.name}
            aria-pressed={value.toLowerCase() === color.value}
            onClick={() => change(color.value)}
            style={
              {
                "--swatch": color.value,
                "--swatch-on": accentForeground(color.value),
              } as CSSProperties
            }
          >
            {value.toLowerCase() === color.value && <Check size={18} />}
          </button>
        ))}
      </div>
      <label htmlFor={id}>自定义颜色</label>
      <div className="accent-custom">
        <input
          type="color"
          aria-label="拾取强调色"
          value={value}
          onChange={(e) => change(e.target.value)}
        />
        <input
          id={id}
          type="text"
          inputMode="text"
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
          value={draft}
          maxLength={7}
          aria-invalid={!validAccent(draft)}
          aria-describedby={!validAccent(draft) ? id + "-error" : undefined}
          onChange={(e) => change(e.target.value)}
        />
      </div>
      {!validAccent(draft) && (
        <small id={id + "-error"} className="accent-error">
          输入 #RRGGBB，例如 #d97706
        </small>
      )}
    </fieldset>
  );
}
