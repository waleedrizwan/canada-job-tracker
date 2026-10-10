// Small hand-rolled SVG charts: horizontal bars and vertical histograms.
// Single-series, so one accent color; hover shows a tooltip, click is optional.

const NS = "http://www.w3.org/2000/svg";
const tip = () => document.getElementById("tooltip");

const el = (name, attrs = {}, text) => {
  const n = document.createElementNS(NS, name);
  for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v);
  if (text != null) n.textContent = text;
  return n;
};

export function showTip(evt, html) {
  const t = tip();
  t.innerHTML = html;
  t.hidden = false;
  const pad = 14;
  const { innerWidth: W, innerHeight: H } = window;
  const r = t.getBoundingClientRect();
  let x = evt.clientX + pad, y = evt.clientY + pad;
  if (x + r.width > W - 8) x = evt.clientX - r.width - pad;
  if (y + r.height > H - 8) y = evt.clientY - r.height - pad;
  t.style.left = `${x}px`;
  t.style.top = `${y}px`;
}
export const hideTip = () => { tip().hidden = true; };

const niceMax = (v) => {
  if (v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  return [1, 2, 2.5, 5, 10].map((m) => m * p).find((m) => m >= v);
};

const empty = (host) => { host.innerHTML = `<div class="empty">No postings match these filters</div>`; };

// Rounded on the data end only, square on the baseline.
const hBarPath = (x, y, w, h, r = 4) => {
  r = Math.min(r, w, h / 2);
  return `M${x},${y}h${w - r}a${r},${r} 0 0 1 ${r},${r}v${h - 2 * r}a${r},${r} 0 0 1 -${r},${r}h-${w - r}z`;
};
const vBarPath = (x, y, w, h, r = 4) => {
  r = Math.min(r, h, w / 2);
  return `M${x},${y + h}v-${h - r}a${r},${r} 0 0 1 ${r},-${r}h${w - 2 * r}a${r},${r} 0 0 1 ${r},${r}v${h - r}z`;
};

/**
 * rows: [{ label, value, tip?, active? }]
 * opts: { format(value), onClick(label), width }
 */
export function hbar(host, rows, opts = {}) {
  if (!rows.length) return empty(host);
  const fmt = opts.format ?? ((v) => v.toLocaleString());
  const W = opts.width ?? 520;
  const labelW = 150, valueW = 64, rowH = 26, barH = 16;
  const plotW = W - labelW - valueW;
  const H = rows.length * rowH + 4;
  const max = Math.max(...rows.map((r) => r.value));
  const anyActive = rows.some((r) => r.active);

  const svg = el("svg", { viewBox: `0 0 ${W} ${H}`, role: "img", "aria-label": opts.ariaLabel ?? "Bar chart" });
  rows.forEach((r, i) => {
    const y = i * rowH + 2;
    const w = max ? Math.max(2, (r.value / max) * plotW) : 2;
    const g = el("g", { class: "row" });
    g.append(
      el("text", { class: "lbl", x: labelW - 10, y: y + rowH / 2 + 4, "text-anchor": "end" }, truncate(r.label, 22)),
      el("path", { class: `bar${anyActive && !r.active ? " dim" : ""}`, d: hBarPath(labelW, y + (rowH - barH) / 2, w, barH) }),
      el("text", { class: "val", x: labelW + w + 6, y: y + rowH / 2 + 4 }, fmt(r.value)),
    );
    const hit = el("rect", { class: "hit", x: 0, y, width: W, height: rowH });
    hit.addEventListener("mousemove", (e) => showTip(e, r.tip ?? `${r.label}<br><b>${fmt(r.value)}</b>`));
    hit.addEventListener("mouseleave", hideTip);
    if (opts.onClick) hit.addEventListener("click", () => { hideTip(); opts.onClick(r.label); });
    else hit.style.cursor = "default";
    g.append(hit);
    svg.append(g);
  });
  host.replaceChildren(svg);
}

/**
 * bins: [{ label, value, tip? }]  (in order, left to right)
 * opts: { format(value), marker: { index, label }, xLabelEvery }
 */
export function vbar(host, bins, opts = {}) {
  if (!bins.length || bins.every((b) => b.value === 0)) return empty(host);
  const fmt = opts.format ?? ((v) => v.toLocaleString());
  const W = 520, H = 230, m = { t: 14, r: 8, b: 30, l: 44 };
  const pw = W - m.l - m.r, ph = H - m.t - m.b;
  const max = niceMax(Math.max(...bins.map((b) => b.value)));
  const step = pw / bins.length, gap = 2, bw = Math.max(1, step - gap);
  const every = opts.xLabelEvery ?? 1;

  const svg = el("svg", { viewBox: `0 0 ${W} ${H}`, role: "img", "aria-label": opts.ariaLabel ?? "Histogram" });
  const axis = el("g", { class: "axis" });
  for (let i = 0; i <= 4; i++) {
    const v = (max / 4) * i, y = m.t + ph - (v / max) * ph;
    axis.append(
      el("line", { class: i ? "gridline" : "baseline", x1: m.l, x2: W - m.r, y1: y, y2: y }),
      el("text", { x: m.l - 6, y: y + 4, "text-anchor": "end" }, compact(v)),
    );
  }
  bins.forEach((b, i) => {
    if (i % every === 0) axis.append(el("text", { x: m.l + i * step + step / 2, y: H - 10, "text-anchor": "middle" }, b.label));
  });
  svg.append(axis);

  bins.forEach((b, i) => {
    const h = (b.value / max) * ph, x = m.l + i * step + gap / 2;
    const g = el("g", { class: "row" });
    const hit = el("rect", { class: "hit", x: m.l + i * step, y: m.t, width: step, height: ph });
    hit.style.cursor = "default";
    hit.addEventListener("mousemove", (e) => showTip(e, b.tip ?? `${b.label}<br><b>${fmt(b.value)}</b>`));
    hit.addEventListener("mouseleave", hideTip);
    g.append(hit);
    if (b.value > 0) g.append(el("path", { class: "bar", d: vBarPath(x, m.t + ph - Math.max(h, 2), bw, Math.max(h, 2)) }));
    svg.append(g);
  });

  if (opts.marker) {
    const x = m.l + opts.marker.index * step;
    svg.append(
      el("line", { class: "marker", x1: x, x2: x, y1: m.t - 4, y2: m.t + ph }),
      el("text", { class: "marker-lbl", x: x + 4, y: m.t + 6 }, opts.marker.label),
    );
  }
  host.replaceChildren(svg);
}

export const compact = (v) =>
  v >= 1e6 ? `${+(v / 1e6).toFixed(1)}M` : v >= 1e3 ? `${+(v / 1e3).toFixed(1)}k` : `${Math.round(v)}`;
const truncate = (s, n) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
