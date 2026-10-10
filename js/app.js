import { hbar, vbar, compact } from "./charts.js";

const $ = (id) => document.getElementById(id);
const DAY = 86400000;
const PAGE_SIZE = 25;
const STALE_DAYS = 30;

const money = (v) => `$${Math.round(v).toLocaleString()}`;
const moneyK = (v) => `$${compact(v)}`;
const pct = (n, d) => (d ? `${Math.round((n / d) * 100)}%` : "–");
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const median = (arr) => {
  if (!arr.length) return null;
  const s = [...arr].sort((a, b) => a - b), mid = s.length >> 1;
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
};
const countBy = (rows, key) => {
  const m = new Map();
  for (const r of rows) m.set(r[key], (m.get(r[key]) ?? 0) + 1);
  return [...m].map(([label, value]) => ({ label, value })).sort((a, b) => b.value - a.value);
};

let ALL = [];
let asOf = new Date();
const sort = { key: "age", dir: 1 };
let page = 0;

const F = {
  q: $("f-q"), category: $("f-category"), city: $("f-city"), mode: $("f-mode"), type: $("f-type"),
  seniority: $("f-seniority"), pay: $("f-pay"), age: $("f-age"), open: $("f-open"), salary: $("f-salary"),
};

async function load() {
  const res = await fetch("data/jobs.json", { cache: "no-store" });
  if (!res.ok) throw new Error(`Couldn't load data/jobs.json (${res.status})`);
  const data = await res.json();
  asOf = new Date(data.generatedAt);
  $("sample-banner").hidden = !data.sample;
  $("as-of").textContent = `${data.jobs.length.toLocaleString()} postings · ${data.source} · as of ${asOf.toLocaleDateString("en-CA", { dateStyle: "medium", timeZone: "UTC" })}`;

  ALL = data.jobs.map((j) => {
    const pay = j.salaryMin != null || j.salaryMax != null ? ((j.salaryMin ?? j.salaryMax) + (j.salaryMax ?? j.salaryMin)) / 2 : null;
    return { ...j, pay, age: Math.max(0, Math.round((asOf - new Date(j.postedAt)) / DAY)), search: `${j.title} ${j.company}`.toLowerCase() };
  });

  fillSelect(F.category, ALL, "category");
  fillSelect(F.city, ALL, "city");
  fillSelect(F.mode, ALL, "workMode");
  fillSelect(F.type, ALL, "employmentType");
  fillSelect(F.seniority, ALL, "seniority", ["Entry", "Mid", "Senior", "Executive"]);
  readUrl();
}

function fillSelect(sel, rows, key, order) {
  const values = order ?? [...new Set(rows.map((r) => r[key]))].sort();
  for (const v of values) sel.append(new Option(v, v));
}

function filtered({ ignoreAge = false } = {}) {
  const q = F.q.value.trim().toLowerCase();
  const minPay = +F.pay.value;
  const maxAge = ignoreAge || F.age.value === "" ? Infinity : +F.age.value;
  return ALL.filter((j) =>
    (!q || j.search.includes(q)) &&
    (!F.category.value || j.category === F.category.value) &&
    (!F.city.value || j.city === F.city.value) &&
    (!F.mode.value || j.workMode === F.mode.value) &&
    (!F.type.value || j.employmentType === F.type.value) &&
    (!F.seniority.value || j.seniority === F.seniority.value) &&
    (!minPay || (j.pay != null && j.pay >= minPay)) &&
    (!F.salary.checked || j.pay != null) &&
    (!F.open.checked || !j.closedAt) &&
    j.age <= maxAge
  );
}

function render() {
  const rows = filtered();
  writeUrl();
  $("f-pay-out").textContent = +F.pay.value ? `${moneyK(+F.pay.value)}+` : "Any";
  renderKpis(rows);
  renderCharts(rows);
  renderTable(rows);
}

function renderKpis(rows) {
  const paid = rows.filter((r) => r.pay != null);
  const med = median(paid.map((r) => r.pay));
  // Measured across every age, otherwise the default 30-day window hides it.
  const anyAge = filtered({ ignoreAge: true });
  const stale = anyAge.filter((r) => r.age > STALE_DAYS).length;
  const companies = new Set(rows.map((r) => r.company)).size;
  const remote = rows.filter((r) => r.workMode !== "On-site").length;
  const kpis = [
    ["Matching postings", rows.length.toLocaleString(), `of ${ALL.length.toLocaleString()} total`],
    ["Median pay", med ? money(med) : "–", `annual midpoint · ${paid.length.toLocaleString()} with pay`],
    ["Pay disclosed", pct(paid.length, rows.length), "postings listing a salary"],
    ["Hiring companies", companies.toLocaleString(), "distinct employers"],
    ["Remote or hybrid", pct(remote, rows.length), "of matching postings"],
    [`Older than ${STALE_DAYS} days`, pct(stale, anyAge.length), "ignores the date filter · ghost-job risk"],
  ];
  $("kpis").innerHTML = kpis
    .map(([l, v, n]) => `<div class="kpi"><div class="label">${l}</div><div class="value">${v}</div><div class="note">${n}</div></div>`)
    .join("");
}

function renderCharts(rows) {
  const toggle = (sel) => (label) => { sel.value = sel.value === label ? "" : label; page = 0; render(); };

  hbar($("c-category"), countBy(rows, "category").map((r) => ({ ...r, active: r.label === F.category.value })), {
    onClick: toggle(F.category), ariaLabel: "Postings by category",
  });

  hbar($("c-city"), countBy(rows, "city").slice(0, 10).map((r) => ({ ...r, active: r.label === F.city.value })), {
    onClick: toggle(F.city), ariaLabel: "Postings by city",
  });

  // Salary histogram in $10k bins, capped so a few outliers don't flatten the shape.
  const paid = rows.filter((r) => r.pay != null);
  const BIN = 10000, LO = 20000, HI = 200000;
  const salaryBins = [];
  for (let lo = LO; lo < HI; lo += BIN) salaryBins.push({ lo, hi: lo + BIN, value: 0 });
  for (const r of paid) salaryBins[Math.min(salaryBins.length - 1, Math.max(0, Math.floor((r.pay - LO) / BIN)))].value++;
  vbar($("c-salary"), salaryBins.map((b, i) => ({
    label: moneyK(b.lo),
    value: b.value,
    tip: `${i === 0 ? "Under " + moneyK(b.hi) : i === salaryBins.length - 1 ? moneyK(b.lo) + "+" : `${moneyK(b.lo)}–${moneyK(b.hi)}`}<br><b>${b.value.toLocaleString()}</b> postings`,
  })), { xLabelEvery: 3, ariaLabel: "Salary distribution" });

  const byCat = new Map();
  for (const r of paid) (byCat.get(r.category) ?? byCat.set(r.category, []).get(r.category)).push(r.pay);
  const payRows = [...byCat]
    .map(([label, pays]) => ({ label, value: median(pays), n: pays.length }))
    .filter((r) => r.n >= 3)
    .sort((a, b) => b.value - a.value)
    .map((r) => ({ ...r, active: r.label === F.category.value, tip: `${esc(r.label)}<br>Median <b>${money(r.value)}</b><br>${r.n.toLocaleString()} postings with pay` }));
  hbar($("c-pay-cat"), payRows, { format: moneyK, onClick: toggle(F.category), ariaLabel: "Median pay by category" });

  // Posting age in weekly bins up to 12 weeks, then 12+.
  const WEEKS = 12;
  const ageBins = Array.from({ length: WEEKS + 1 }, (_, i) => ({ value: 0, i }));
  for (const r of rows) ageBins[Math.min(WEEKS, Math.floor(r.age / 7))].value++;
  vbar($("c-age"), ageBins.map(({ i, value }) => ({
    label: i === WEEKS ? `${WEEKS * 7}+` : `${i * 7}`,
    value,
    tip: `${i === WEEKS ? `${WEEKS * 7}+ days` : `${i * 7}–${i * 7 + 6} days`}<br><b>${value.toLocaleString()}</b> postings`,
  })), { xLabelEvery: 2, marker: { index: STALE_DAYS / 7, label: `${STALE_DAYS} days` }, ariaLabel: "Posting age" });

  hbar($("c-source"), countBy(rows, "source").map((r) => ({ ...r, tip: `${esc(r.label)}<br><b>${r.value.toLocaleString()}</b> · ${pct(r.value, rows.length)}` })), {
    ariaLabel: "Source mix",
  });
}

function sortRows(rows) {
  const { key, dir } = sort;
  return [...rows].sort((a, b) => {
    const x = a[key], y = b[key];
    if (x == null && y == null) return 0;
    if (x == null) return 1; // nulls (e.g. no pay) always last
    if (y == null) return -1;
    return (typeof x === "number" ? x - y : String(x).localeCompare(String(y))) * dir;
  });
}

function renderTable(rows) {
  const sorted = sortRows(rows);
  const pages = Math.max(1, Math.ceil(sorted.length / PAGE_SIZE));
  page = Math.min(page, pages - 1);
  const slice = sorted.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);

  $("t-count").textContent = `· ${rows.length.toLocaleString()}`;
  $("jobs").tBodies[0].innerHTML = slice.map((j) => {
    const title = j.url ? `<a href="${esc(j.url)}" target="_blank" rel="noopener">${esc(j.title)}</a>` : esc(j.title);
    const pay = j.salaryMin != null ? (j.salaryMin === j.salaryMax ? moneyK(j.salaryMin) : `${moneyK(j.salaryMin)}–${moneyK(j.salaryMax)}`) : "–";
    return `<tr class="${j.closedAt ? "closed" : ""}">
      <td class="title">${title}${j.closedAt ? '<span class="tag">closed</span>' : ""}</td>
      <td>${esc(j.company)}</td><td>${esc(j.category)}</td><td>${esc(j.city)}</td><td>${esc(j.workMode)}</td>
      <td class="num">${pay}</td><td class="num">${j.age}d</td><td>${esc(j.source)}</td></tr>`;
  }).join("") || `<tr><td colspan="8" class="muted">No postings match these filters</td></tr>`;

  $("t-page").textContent = `Page ${page + 1} of ${pages}`;
  $("t-prev").disabled = page === 0;
  $("t-next").disabled = page >= pages - 1;
  for (const th of document.querySelectorAll("#jobs th")) {
    th.classList.toggle("sorted", th.dataset.sort === sort.key);
    th.classList.toggle("asc", th.dataset.sort === sort.key && sort.dir === 1);
  }
}

function exportCsv() {
  const cols = ["id", "title", "company", "category", "city", "workMode", "employmentType", "seniority", "salaryMin", "salaryMax", "postedAt", "closedAt", "source", "url"];
  const cell = (v) => (v == null ? "" : /[",\n]/.test(String(v)) ? `"${String(v).replace(/"/g, '""')}"` : String(v));
  const csv = [cols.join(","), ...sortRows(filtered()).map((j) => cols.map((c) => cell(j[c])).join(","))].join("\n");
  const a = Object.assign(document.createElement("a"), {
    href: URL.createObjectURL(new Blob([csv], { type: "text/csv" })),
    download: `ontario-jobs-${new Date().toISOString().slice(0, 10)}.csv`,
  });
  a.click();
  URL.revokeObjectURL(a.href);
}

// Filters live in the URL so a filtered view can be bookmarked or shared.
const URL_KEYS = { q: "q", category: "cat", city: "city", mode: "mode", type: "type", seniority: "level", pay: "pay", age: "age" };
function writeUrl() {
  const p = new URLSearchParams();
  for (const [k, param] of Object.entries(URL_KEYS)) {
    const v = F[k].value;
    if (v && !(k === "pay" && v === "0") && !(k === "age" && v === "30")) p.set(param, v);
  }
  if (F.age.value === "") p.set("age", "all");
  if (!F.open.checked) p.set("closed", "1");
  if (F.salary.checked) p.set("paid", "1");
  history.replaceState(null, "", p.size ? `?${p}` : location.pathname);
}
function readUrl() {
  const p = new URLSearchParams(location.search);
  for (const [k, param] of Object.entries(URL_KEYS)) if (p.has(param)) F[k].value = p.get(param) === "all" ? "" : p.get(param);
  F.open.checked = p.get("closed") !== "1";
  F.salary.checked = p.get("paid") === "1";
}

function resetFilters() {
  for (const k of ["q", "category", "city", "mode", "type", "seniority"]) F[k].value = "";
  F.pay.value = 0;
  F.age.value = "30";
  F.open.checked = true;
  F.salary.checked = false;
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
  F.q.addEventListener("input", () => { clearTimeout(t); t = setTimeout(() => { page = 0; render(); }, 150); });
  for (const k of ["category", "city", "mode", "type", "seniority", "pay", "age", "open", "salary"]) {
    F[k].addEventListener(k === "pay" ? "input" : "change", () => { page = 0; render(); });
  }
  $("f-reset").addEventListener("click", resetFilters);
  $("t-prev").addEventListener("click", () => { page--; render(); });
  $("t-next").addEventListener("click", () => { page++; render(); });
  $("t-export").addEventListener("click", exportCsv);
  for (const th of document.querySelectorAll("#jobs th")) {
    th.addEventListener("click", () => {
      const key = th.dataset.sort;
      sort.dir = sort.key === key ? -sort.dir : key === "pay" ? -1 : 1;
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
