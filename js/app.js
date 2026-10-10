import { hbar, vbar, hbarPair, compact } from "./charts.js";

const $ = (id) => document.getElementById(id);
const PAGE_SIZE = 25;
const HOURS_PER_YEAR = 2080;

const wage = (v) => `$${v.toFixed(2)}`;
const wageShort = (v) => `$${v % 1 ? v.toFixed(2) : v}`;
const annual = (h) => `≈ $${compact(h * HOURS_PER_YEAR)}/yr`;
const pct = (n, d) => (d ? `${Math.round((n / d) * 100)}%` : "–");
const pct1 = (v) => `${(v * 100).toFixed(1)}%`;
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const monthKey = (iso) => iso.slice(0, 7);
const monthName = (key) => new Date(`${key}-15T00:00:00Z`).toLocaleDateString("en-CA", { month: "long", year: "numeric", timeZone: "UTC" });
const median = (arr) => {
  if (!arr.length) return null;
  const s = Float64Array.from(arr).sort(), mid = s.length >> 1;
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
};
const countBy = (rows, key) => {
  const m = new Map();
  for (const r of rows) m.set(r[key], (m.get(r[key]) ?? 0) + 1);
  return [...m].map(([label, value]) => ({ label, value })).sort((a, b) => b.value - a.value);
};

let DATA = null;
let ALL = [];
const sort = { key: "postedAt", dir: -1 };
let page = 0;

const F = {
  q: $("f-q"), category: $("f-category"), occupation: $("f-occupation"), city: $("f-city"), level: $("f-level"),
  type: $("f-type"), term: $("f-term"), wage: $("f-wage"), month: $("f-month"), source: $("f-source"),
};
const FIELD = { category: "category", occupation: "occupation", city: "city", level: "level", type: "employmentType", term: "term", source: "source" };

// Rows are stored column-wise with dictionaries for repeated strings; expand them here.
function decode(data) {
  return data.rows.map((row) => {
    const j = {};
    data.cols.forEach((c, i) => { j[c] = data.dicts[c] ? data.dicts[c][row[i]] : row[i]; });
    j.wage = j.wageMin != null ? (j.wageMin + (j.wageMax ?? j.wageMin)) / 2 : null;
    j.month = monthKey(j.postedAt);
    j.search = `${j.title} ${j.occupation}`.toLowerCase();
    return j;
  });
}

async function load() {
  const res = await fetch("data/jobs.json");
  if (!res.ok) throw new Error(`Couldn't load postings (${res.status})`);
  DATA = await res.json();
  ALL = decode(DATA);

  const asOf = new Date(`${DATA.asOf}T00:00:00Z`).toLocaleDateString("en-CA", { dateStyle: "medium", timeZone: "UTC" });
  $("as-of").textContent = `${ALL.length.toLocaleString()} postings across Ontario, ${DATA.months.at(-1)} to ${DATA.months[0]} · Government of Canada Job Bank · latest posting ${asOf}`;

  const byCount = (key) => countBy(ALL, key).map((r) => r.label);
  fillSelect(F.category, byCount("category").sort());
  fillSelect(F.occupation, byCount("occupation").sort());
  fillSelect(F.city, byCount("city").filter((c) => c !== "Unspecified").sort().concat("Unspecified"));
  fillSelect(F.level, byCount("level").sort());
  fillSelect(F.type, byCount("employmentType"));
  fillSelect(F.term, byCount("term"));
  fillSelect(F.source, byCount("source"));
  for (const m of [...new Set(ALL.map((j) => j.month))].sort().reverse()) F.month.append(new Option(monthName(m), m));
  readUrl();
}

function fillSelect(sel, values) {
  for (const v of values) sel.append(new Option(v, v));
}

function filtered({ only } = {}) {
  const q = F.q.value.trim().toLowerCase();
  const minWage = +F.wage.value;
  return ALL.filter((j) => {
    if (only) return !F.month.value || j.month === F.month.value;
    if (q && !j.search.includes(q)) return false;
    for (const [k, field] of Object.entries(FIELD)) if (F[k].value && j[field] !== F[k].value) return false;
    if (minWage && (j.wage == null || j.wage < minWage)) return false;
    if (F.month.value && j.month !== F.month.value) return false;
    return true;
  });
}

function render() {
  const rows = filtered();
  writeUrl();
  $("f-wage-out").textContent = +F.wage.value ? `$${F.wage.value}/h+` : "Any";
  renderKpis(rows);
  renderCharts(rows);
  renderBenchmark();
  renderTable(rows);
}

function renderKpis(rows) {
  const paid = rows.filter((r) => r.wage != null);
  const med = median(paid.map((r) => r.wage));
  const vacancies = rows.reduce((s, r) => s + r.vacancies, 0);
  const fullTime = rows.filter((r) => r.employmentType === "Full time").length;
  const withTerm = rows.filter((r) => r.term !== "Unspecified");
  const permanent = withTerm.filter((r) => r.term === "Permanent").length;
  const direct = rows.filter((r) => r.source === "Posted on Job Bank").length;
  const kpis = [
    ["Postings", rows.length.toLocaleString(), `of ${ALL.length.toLocaleString()} total`],
    ["Openings advertised", vacancies.toLocaleString(), "a single posting can hire several people"],
    ["Median offered wage", med ? `${wage(med)}/h` : "–", med ? annual(med) : "no wage data"],
    ["Full time", pct(fullTime, rows.length), "of postings"],
    ["Permanent", pct(permanent, withTerm.length), "of postings that state a term"],
    ["Posted on Job Bank", pct(direct, rows.length), "the rest come from partner boards"],
  ];
  $("kpis").innerHTML = kpis
    .map(([l, v, n]) => `<div class="kpi"><div class="label">${l}</div><div class="value">${v}</div><div class="note">${n}</div></div>`)
    .join("");
}

function renderCharts(rows) {
  const toggle = (sel) => (label) => { sel.value = sel.value === label ? "" : label; page = 0; render(); };
  const share = (v) => pct(v, rows.length);
  const countTip = (r) => `${esc(r.label)}<br><b>${r.value.toLocaleString()}</b> postings · ${share(r.value)}`;
  const counted = (key, sel, n = Infinity) =>
    countBy(rows, key).slice(0, n).map((r) => ({ ...r, active: r.label === sel?.value, tip: countTip(r) }));

  hbar($("c-category"), counted("category", F.category), { onClick: toggle(F.category), ariaLabel: "Postings by category" });
  hbar($("c-occupation"), counted("occupation", F.occupation, 12), { onClick: toggle(F.occupation), ariaLabel: "Most-posted occupations" });
  hbar($("c-city"), counted("city", F.city, 12), { onClick: toggle(F.city), ariaLabel: "Postings by city" });
  hbar($("c-level"), counted("level", F.level).sort((a, b) => a.label.localeCompare(b.label)), { onClick: toggle(F.level), ariaLabel: "Skill level" });
  hbar($("c-source"), counted("source"), { ariaLabel: "Posting source" });

  const paid = rows.filter((r) => r.wage != null);
  const byCat = new Map();
  for (const r of paid) (byCat.get(r.category) ?? byCat.set(r.category, []).get(r.category)).push(r.wage);
  const wageRows = [...byCat]
    .map(([label, w]) => ({ label, value: median(w), n: w.length }))
    .filter((r) => r.n >= 5)
    .sort((a, b) => b.value - a.value)
    .map((r) => ({ ...r, active: r.label === F.category.value, tip: `${esc(r.label)}<br>Median <b>${wage(r.value)}/h</b> ${annual(r.value)}<br>${r.n.toLocaleString()} postings` }));
  hbar($("c-wage-cat"), wageRows, { format: (v) => `${wage(v)}`, onClick: toggle(F.category), ariaLabel: "Median wage by category" });

  // $2 bins from $16 to $80/h; the first and last bins catch everything outside.
  const LO = 16, HI = 80, BIN = 2;
  const bins = [];
  for (let lo = LO; lo < HI; lo += BIN) bins.push({ lo, value: 0 });
  for (const r of paid) bins[Math.min(bins.length - 1, Math.max(0, Math.floor((r.wage - LO) / BIN)))].value++;
  vbar($("c-wage"), bins.map((b, i) => ({
    label: `$${b.lo}`,
    value: b.value,
    tip: `${i === 0 ? `Under $${b.lo + BIN}` : i === bins.length - 1 ? `$${b.lo}+` : `$${b.lo}–$${b.lo + BIN}`}/h<br><b>${b.value.toLocaleString()}</b> postings`,
  })), { xLabelEvery: 4, ariaLabel: "Wage distribution" });

  // Weekly buckets keyed by the Monday of each week.
  const weeks = new Map();
  for (const r of rows) {
    const d = new Date(`${r.postedAt}T00:00:00Z`);
    d.setUTCDate(d.getUTCDate() - ((d.getUTCDay() + 6) % 7));
    const k = d.toISOString().slice(0, 10);
    weeks.set(k, (weeks.get(k) ?? 0) + 1);
  }
  const fmtDay = (k) => new Date(`${k}T00:00:00Z`).toLocaleDateString("en-CA", { month: "short", day: "numeric", timeZone: "UTC" });
  const weekRows = [...weeks].sort(([a], [b]) => (a < b ? -1 : 1)).map(([k, value]) => ({
    label: fmtDay(k), value, tip: `Week of ${fmtDay(k)}<br><b>${value.toLocaleString()}</b> new postings`,
  }));
  vbar($("c-weekly"), weekRows, { xLabelEvery: Math.max(1, Math.ceil(weekRows.length / 7)), ariaLabel: "New postings per week" });
}

function renderBenchmark() {
  const bench = DATA.benchmark;
  if (!bench) return;
  const rows = filtered({ only: "month" }).filter((r) => r.category !== "Unclassified");
  const counts = new Map(countBy(rows, "category").map((r) => [r.label, r.value]));
  const sTotal = Object.entries(bench.byCategory).filter(([k]) => k !== "Total").reduce((s, [, v]) => s + (v.vacancies ?? 0), 0);
  const pairs = Object.entries(bench.byCategory)
    .filter(([k]) => k !== "Total")
    .map(([label, v]) => {
      const a = (counts.get(label) ?? 0) / (rows.length || 1), b = (v.vacancies ?? 0) / sTotal;
      const ratio = b ? a / b : 0;
      return {
        label, a, b,
        tip: `${esc(label)}<br>Job Bank: <b>${pct1(a)}</b> of postings<br>StatCan: <b>${pct1(b)}</b> of vacancies (${(v.vacancies ?? 0).toLocaleString()})` +
          `<br>${ratio > 1.15 ? "Over-represented" : ratio < 0.85 ? "Under-represented" : "Roughly in line"} on Job Bank (${ratio.toFixed(2)}×)` +
          (v.wage ? `<br>StatCan avg offered wage: <b>${wage(v.wage)}/h</b>` : ""),
      };
    })
    .sort((x, y) => y.b - x.b);
  $("bench-hint").textContent = `Share of the Ontario total in each category · StatCan estimates for ${bench.period} (${bench.byCategory.Total?.vacancies?.toLocaleString() ?? "?"} vacancies)`;
  hbarPair($("c-bench"), pairs, { names: ["Job Bank postings", `StatCan vacancies, ${bench.period}`], format: pct1, ariaLabel: "Job Bank postings vs StatCan vacancies" });
}

function sortRows(rows) {
  const { key, dir } = sort;
  return [...rows].sort((a, b) => {
    const x = a[key], y = b[key];
    if (x == null && y == null) return 0;
    if (x == null) return 1; // missing wages always last
    if (y == null) return -1;
    return (typeof x === "number" ? x - y : String(x).localeCompare(String(y))) * dir;
  });
}

const jobBankSearch = (j) =>
  `https://www.jobbank.gc.ca/jobsearch/jobsearch?searchstring=${encodeURIComponent(j.title)}&locationstring=${encodeURIComponent(j.city === "Unspecified" ? "Ontario" : `${j.city}, ON`)}`;

function renderTable(rows) {
  const sorted = sortRows(rows);
  const pages = Math.max(1, Math.ceil(sorted.length / PAGE_SIZE));
  page = Math.min(page, pages - 1);
  const slice = sorted.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);

  $("t-count").textContent = `· ${rows.length.toLocaleString()}`;
  $("jobs").tBodies[0].innerHTML = slice.map((j) => {
    const w = j.wageMin == null ? "–" : j.wageMin === j.wageMax ? wageShort(j.wageMin) : `${wageShort(j.wageMin)}–${wageShort(j.wageMax)}`;
    return `<tr>
      <td class="title"><a href="${esc(jobBankSearch(j))}" target="_blank" rel="noopener" title="Search Job Bank for current postings like this">${esc(j.title)}</a></td>
      <td>${esc(j.occupation)}</td><td>${esc(j.city)}</td><td>${esc(j.employmentType)}</td><td>${esc(j.term)}</td>
      <td class="num">${w}</td><td class="num">${j.vacancies}</td><td>${j.postedAt}</td></tr>`;
  }).join("") || `<tr><td colspan="8" class="muted">No postings match these filters</td></tr>`;

  $("t-page").textContent = `Page ${(page + 1).toLocaleString()} of ${pages.toLocaleString()}`;
  $("t-prev").disabled = page === 0;
  $("t-next").disabled = page >= pages - 1;
  for (const th of document.querySelectorAll("#jobs th")) {
    th.classList.toggle("sorted", th.dataset.sort === sort.key);
    th.classList.toggle("asc", th.dataset.sort === sort.key && sort.dir === 1);
  }
}

function exportCsv() {
  const cols = ["id", "title", "occupation", "category", "level", "city", "employmentType", "term", "wageMin", "wageMax", "vacancies", "postedAt", "source"];
  const cell = (v) => (v == null ? "" : /[",\n]/.test(String(v)) ? `"${String(v).replace(/"/g, '""')}"` : String(v));
  const csv = [cols.join(","), ...sortRows(filtered()).map((j) => cols.map((c) => cell(j[c])).join(","))].join("\n");
  const a = Object.assign(document.createElement("a"), {
    href: URL.createObjectURL(new Blob([csv], { type: "text/csv" })),
    download: `ontario-jobs-${DATA.asOf}.csv`,
  });
  a.click();
  URL.revokeObjectURL(a.href);
}

// Filters live in the URL so a filtered view can be bookmarked or shared.
const URL_KEYS = { q: "q", category: "cat", occupation: "occ", city: "city", level: "level", type: "hours", term: "term", wage: "wage", month: "month", source: "src" };
function writeUrl() {
  const p = new URLSearchParams();
  for (const [k, param] of Object.entries(URL_KEYS)) if (F[k].value && F[k].value !== "0") p.set(param, F[k].value);
  history.replaceState(null, "", p.size ? `?${p}` : location.pathname);
}
function readUrl() {
  const p = new URLSearchParams(location.search);
  for (const [k, param] of Object.entries(URL_KEYS)) if (p.has(param)) F[k].value = p.get(param);
}

function resetFilters() {
  for (const k of Object.keys(F)) F[k].value = k === "wage" ? 0 : "";
  page = 0;
  render();
}

function initTheme() {
  const stored = (() => { try { return localStorage.getItem("theme"); } catch { return null; } })();
  if (stored) document.documentElement.dataset.theme = stored;
  $("theme-toggle").addEventListener("click", () => {
    const dark = document.documentElement.dataset.theme
      ? document.documentElement.dataset.theme === "dark"
      : matchMedia("(prefers-color-scheme: dark)").matches;
    const next = dark ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    try { localStorage.setItem("theme", next); } catch {}
  });
}

function bind() {
  let t;
  F.q.addEventListener("input", () => { clearTimeout(t); t = setTimeout(() => { page = 0; render(); }, 200); });
  for (const k of Object.keys(F)) {
    if (k === "q") continue;
    F[k].addEventListener(k === "wage" ? "input" : "change", () => { page = 0; render(); });
  }
  $("f-reset").addEventListener("click", resetFilters);
  $("t-prev").addEventListener("click", () => { page--; render(); });
  $("t-next").addEventListener("click", () => { page++; render(); });
  $("t-export").addEventListener("click", exportCsv);
  for (const th of document.querySelectorAll("#jobs th")) {
    th.addEventListener("click", () => {
      const key = th.dataset.sort;
      sort.dir = sort.key === key ? -sort.dir : ["wage", "vacancies", "postedAt"].includes(key) ? -1 : 1;
      sort.key = key;
      render();
    });
  }
}

initTheme();
bind();
load().then(render).catch((err) => {
  $("as-of").textContent = err.message;
  console.error(err);
});
