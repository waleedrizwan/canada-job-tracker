// Builds data/jobs.json from official Government of Canada sources:
//
//  1. Job Bank open data: every posting advertised on Canada's National Job Bank,
//     published monthly on open.canada.ca. We keep Ontario postings from the
//     latest MONTHS files.
//  2. Statistics Canada Job Vacancy and Wage Survey (table 14-10-0444): estimated
//     job vacancies and average offered hourly wage for Ontario by broad
//     occupational category, used as a benchmark for what postings miss.
//
//   node scripts/fetch-data.mjs            (MONTHS=3 by default)
import { writeFileSync } from "node:fs";

const MONTHS = Number(process.env.MONTHS ?? 3);
const DATASET = "ea639e28-c0fc-48bf-b5dd-b8899bd43072";
const HOURS_PER_YEAR = 2080;

// NOC 2021 broad occupational categories (first digit of the 5-digit code).
const BROAD = {
  0: "Senior management",
  1: "Business, finance & admin",
  2: "Sciences & tech",
  3: "Health",
  4: "Education, law & social services",
  5: "Arts, culture & recreation",
  6: "Sales & service",
  7: "Trades & transport",
  8: "Natural resources & agriculture",
  9: "Manufacturing & utilities",
};
// TEER (second digit): the training, education, experience and responsibilities a job needs.
const TEER = {
  0: "TEER 0 · Management",
  1: "TEER 1 · University degree",
  2: "TEER 2 · College / apprenticeship (2+ yrs)",
  3: "TEER 3 · College / apprenticeship (<2 yrs)",
  4: "TEER 4 · High school",
  5: "TEER 5 · No formal education",
};
// Toronto's former municipalities are reported separately in Job Bank.
const TORONTO = new Set(["North York", "Etobicoke", "Scarborough", "York", "East York"]);

const get = async (url, as = "json") => {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status} fetching ${url}`);
  return as === "json" ? res.json() : res.arrayBuffer();
};
const na = (v) => (v == null || v === "" || v === "NA" || v.startsWith("*") ? null : v);
const titleCase = (s) => s.replace(/\b\w/g, (c) => c.toUpperCase());

// ---- 1. Job Bank -----------------------------------------------------------

async function jobBankFiles() {
  const { result } = await get(`https://open.canada.ca/data/api/action/package_show?id=${DATASET}`);
  return result.resources
    .filter((r) => /-en-/.test(r.url) && /\.csv$/i.test(r.url))
    .sort((a, b) => (a.created < b.created ? 1 : -1))
    .slice(0, MONTHS)
    .map((r) => ({ url: r.url, name: typeof r.name === "object" ? r.name.en : r.name }));
}

function hourly(per, value, hoursPer, hoursMax) {
  const v = Number(value);
  if (!v) return null;
  const weekly = hoursPer === "Week" && Number(hoursMax) > 0 ? Number(hoursMax) : 40;
  switch (per) {
    case "Hour": return v;
    case "Day": return v / 8;
    case "Week": return v / weekly;
    case "Bi-weekly": return v / (weekly * 2);
    case "Month": return (v * 12) / (weekly * 52);
    case "Year": return v / HOURS_PER_YEAR;
    default: return null;
  }
}
const sane = (h) => (h != null && h >= 10 && h <= 300 ? Math.round(h * 100) / 100 : null);

async function loadMonth(file) {
  const buf = await get(file.url, "buffer");
  // Files are UTF-16LE tab-separated values without quoting.
  const text = new TextDecoder("utf-16le").decode(buf).replace(/^﻿/, "");
  const lines = text.split(/\r?\n/);
  const header = lines[0].split("\t").map((h) => h.trim().replace(/\s+/g, " "));
  const col = Object.fromEntries(header.map((h, i) => [h, i]));
  const need = ["Province/Territory", "First Posting Date", "NOC21 Code", "Salary Per"];
  for (const n of need) if (!(n in col)) throw new Error(`Job Bank file is missing column "${n}"`);

  const out = [];
  for (let i = 1; i < lines.length; i++) {
    const f = lines[i].split("\t");
    if (f[col["Province/Territory"]] !== "Ontario") continue;
    const v = (name) => na(f[col[name]]?.trim());

    const noc = v("NOC21 Code");
    const per = v("Salary Per");
    const lo = sane(hourly(per, v("Salary Minimum"), v("Hours Per"), v("Hours Maximum")));
    const hi = sane(hourly(per, v("Salary Maximum"), v("Hours Per"), v("Hours Maximum"))) ?? lo;
    let city = v("City") ?? "Unspecified";
    if (TORONTO.has(city)) city = "Toronto";
    const empType = v("Employment Type");

    out.push({
      id: v("WIC Job Location Snapshot ID"),
      title: v("Original Job Title") ?? titleCase(v("Job Title") ?? "Untitled"),
      occupation: v("NOC21 Code Name") ?? "Unclassified",
      category: noc && /^\d/.test(noc) ? BROAD[noc[0]] : "Unclassified",
      level: noc && /^\d\d/.test(noc) ? TEER[noc[1]] ?? "Unclassified" : "Unclassified",
      city,
      employmentType: empType ? (/^Part time leading/.test(empType) ? "Part time → full time" : empType) : "Unspecified",
      term: (v("Employment Term") ?? "Unspecified").replace(/ employment$/, ""),
      wageMin: lo,
      wageMax: hi && lo && hi < lo ? lo : hi,
      postedAt: v("First Posting Date")?.replaceAll("/", "-"),
      vacancies: Number(v("Vacancy Count")) || 1,
      source: v("External Indicator") === "1" ? "Partner job boards" : "Posted on Job Bank",
    });
  }
  console.log(`  ${file.name.replace(/ Job Postings.*/, "")}: ${out.length.toLocaleString()} Ontario postings`);
  return out;
}

// ---- 2. StatCan JVWS benchmark ---------------------------------------------

async function benchmark() {
  const PID = 14100444, ONTARIO = 7, VACANCIES = 1, WAGE = 5;
  // NOC member ids 1 (total) and 2..11 map to broad categories 0..9.
  const members = [1, ...Array.from({ length: 10 }, (_, i) => i + 2)];
  const body = members.flatMap((m) => [VACANCIES, WAGE].map((s) => ({
    productId: PID, coordinate: `${ONTARIO}.${m}.${s}.0.0.0.0.0.0.0`, latestN: 1,
  })));
  const res = await fetch("https://www150.statcan.gc.ca/t1/wds/rest/getDataFromCubePidCoordAndLatestNPeriods", {
    method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`StatCan ${res.status}`);
  const data = await res.json();

  const byCat = {};
  let refPeriod = null;
  for (const { status, object } of data) {
    if (status !== "SUCCESS") continue;
    const [, m, s] = object.coordinate.split(".").map(Number);
    const point = object.vectorDataPoint?.[0];
    if (!point) continue;
    refPeriod = point.refPer;
    const label = m === 1 ? "Total" : BROAD[m - 2];
    byCat[label] ??= {};
    byCat[label][s === VACANCIES ? "vacancies" : "wage"] = point.value;
  }
  const d = new Date(refPeriod);
  const quarter = `Q${Math.floor(d.getUTCMonth() / 3) + 1} ${d.getUTCFullYear()}`;
  console.log(`  StatCan JVWS ${quarter}: ${byCat.Total?.vacancies?.toLocaleString()} Ontario vacancies`);
  return { source: "Statistics Canada, Job Vacancy and Wage Survey, table 14-10-0444-01", period: quarter, byCategory: byCat };
}

// ---- Write -------------------------------------------------------------------

// Columnar encoding with per-column dictionaries keeps the file small.
function encode(jobs) {
  const cols = Object.keys(jobs[0]);
  const dictCols = cols.filter((c) => typeof jobs[0][c] === "string" && c !== "id" && c !== "postedAt");
  const dicts = Object.fromEntries(dictCols.map((c) => [c, []]));
  const index = Object.fromEntries(dictCols.map((c) => [c, new Map()]));
  const rows = jobs.map((j) => cols.map((c) => {
    if (!dicts[c]) return j[c];
    let k = index[c].get(j[c]);
    if (k == null) { k = dicts[c].push(j[c]) - 1; index[c].set(j[c], k); }
    return k;
  }));
  return { cols, dicts, rows };
}

console.log("Job Bank open data:");
const files = await jobBankFiles();
const jobs = (await Promise.all(files.map(loadMonth))).flat().filter((j) => j.postedAt);
jobs.sort((a, b) => (a.postedAt < b.postedAt ? 1 : -1));

console.log("StatCan:");
const bench = await benchmark();

const latest = jobs[0].postedAt;
const out = {
  asOf: latest,
  sample: false,
  source: "Job Bank open data (Government of Canada)",
  sourceUrl: `https://open.canada.ca/data/en/dataset/${DATASET}`,
  months: files.map((f) => f.name.replace(/ Job Postings.*/, "")),
  benchmark: bench,
  ...encode(jobs),
};
writeFileSync(new URL("../data/jobs.json", import.meta.url), JSON.stringify(out));
console.log(`Wrote ${jobs.length.toLocaleString()} postings (latest ${latest}) to data/jobs.json`);
