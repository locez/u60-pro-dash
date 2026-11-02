/* Appearance preferences only. No device state or credentials are persisted. */
'use strict';
window.U60Appearance = (() => {
  const storageKey = 'u60-screen.appearance.v1';
  const defaults = Object.freeze({ theme: 'dark', accent: '#2563eb' });
  const presets = [
    { name: '海蓝', color: '#2563eb' },
    { name: '青碧', color: '#0891b2' },
    { name: '松绿', color: '#258052' },
    { name: '琥珀', color: '#d97706' },
    { name: '玫红', color: '#db3d70' },
    { name: '石墨', color: '#64748b' },
  ];
  function normalize(value) {
    if (typeof value !== 'string') return null;
    const hex = value.trim().replace(/^#/, '');
    if (/^[\da-f]{3}$/i.test(hex)) return '#' + [...hex].map(c => c + c).join('').toLowerCase();
    return /^[\da-f]{6}$/i.test(hex) ? '#' + hex.toLowerCase() : null;
  }
  const rgb = hex => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
  function luminance(hex) {
    const linear = rgb(hex).map(v => {
      const c = v / 255;
      return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4;
    });
    return linear[0] * .2126 + linear[1] * .7152 + linear[2] * .0722;
  }
  function contrast(a, b) {
    const x = luminance(a), y = luminance(b);
    return (Math.max(x, y) + .05) / (Math.min(x, y) + .05);
  }
  function mix(a, b, amount) {
    const from = rgb(a), to = rgb(b);
    return '#' + from.map((c, i) => Math.round(c + (to[i] - c) * amount).toString(16).padStart(2, '0')).join('');
  }
  function palette(seed, surfaces) {
    const fill = normalize(seed) || defaults.accent;
    const target = luminance(surfaces[0]) > .5 ? '#000000' : '#ffffff';
    let ink = fill;
    // Keep the chosen fill, but adjust accent text to remain legible on both surfaces.
    for (let step = 1; Math.min(...surfaces.map(bg => contrast(ink, bg))) < 4.5 && step <= 100; step++) {
      ink = mix(fill, target, step / 100);
    }
    const onFill = contrast(fill, '#ffffff') >= contrast(fill, '#000000') ? '#ffffff' : '#000000';
    return { fill, ink, onFill, soft: mix(fill, '#ffffff', .93) };
  }
  function read() {
    try {
      const saved = JSON.parse(localStorage.getItem(storageKey));
      return {
        theme: ['light', 'dark'].includes(saved?.theme) ? saved.theme : defaults.theme,
        accent: normalize(saved?.accent) || defaults.accent,
      };
    } catch { return { ...defaults }; }
  }
  function save({ theme, accent }) {
    try {
      localStorage.setItem(storageKey, JSON.stringify({ theme, accent }));
      return true;
    } catch { return false; }
  }
  function apply(screen, { theme, accent }) {
    screen.dataset.theme = theme;
    const styles = getComputedStyle(screen);
    const colors = palette(accent, ['--screen-bg', '--screen-panel'].map(key => styles.getPropertyValue(key).trim()));
    for (const [key, value] of Object.entries({ accent: colors.ink, 'accent-fill': colors.fill, 'button-text': colors.onFill })) {
      screen.style.setProperty('--screen-' + key, value);
    }
    const bench = palette(accent, ['#f5f5f5', '#ffffff']);
    for (const [key, value] of Object.entries({ accent: bench.ink, 'accent-fill': bench.fill, 'on-accent': bench.onFill, 'accent-soft': bench.soft })) {
      document.documentElement.style.setProperty('--' + key, value);
    }
  }
  return { defaults, presets, normalize, palette, read, save, apply };
})();
