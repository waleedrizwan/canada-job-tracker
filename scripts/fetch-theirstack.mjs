// Pulls recent Ontario job postings from TheirStack and writes data/jobs.json
// in the dashboard's schema.
//
//   THEIRSTACK_API_KEY=... node scripts/fetch-theirstack.mjs            # fetch
//   THEIRSTACK_API_KEY=... node scripts/fetch-theirstack.mjs --count    # size only, fetches 1 job
//
// Env:
//   THEIRSTACK_API_KEY   required
//   MAX_JOBS             cap on jobs fetched (default 500). Each job costs 1 credit.
//   MAX_AGE_DAYS         posted within this many days (default 60)
//   ONTARIO_LOCATION_ID  TheirStack location id for Ontario; without it we fall back
//                        to a text pattern on the location.
//   CAD_PER_USD          used only when a posting has USD pay but no native CAD pay (default 1.37)
import { readFileSync, writeFileSync, existsSync } from "node:fs";

// Minimal .env support so the key can live in a gitignored file.
const envFile = new URL("../.env", import.meta.url);
if (existsSync(envFile)) {
  for (const line of readFileSync(envFile, "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z_]+)\s*=\s*(.*?)\s*$/);
    if (m && !process.env[m[1]]) process.env[m[1]] = m[2].replace(/^["']|["']$/g, "");
  }
}

const KEY = process.env.THEIRSTACK_API_KEY;
if (!KEY) {
  console.error("Set THEIRSTACK_API_KEY (env or .env). Get one at https://app.theirstack.com");
  process.exit(1);
}
const COUNT_ONLY = process.argv.includes("--count");
const MAX_JOBS = Number(process.env.MAX_JOBS ?? 500);
const MAX_AGE_DAYS = Number(process.env.MAX_AGE_DAYS ?? 60);
const CAD_PER_USD = Number(process.env.CAD_PER_USD ?? 1.37);
const PAGE_SIZE = 100;

const baseFilters = {
  job_country_code_or: ["CA"],
  posted_at_max_age_days: MAX_AGE_DAYS,
  ...(process.env.ONTARIO_LOCATION_ID
    ? { job_location_or: [{ id: Number(process.env.ONTARIO_LOCATION_ID) }] }
    : { job_location_pattern_or: ["Ontario"] }),
  order_by: [{ field: "date_posted", desc: true }],
};

async function search(body) {
  const res = await fetch("https://api.theirstack.com/v1/jobs/search", {
    method: "POST",
    headers: { Authorization: `Bearer ${KEY}`, "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`TheirStack ${res.status}: ${await res.text()}`);
  return res.json();
}

// TheirStack has no industry field on the posting itself, so bucket by title keywords.
// Order matters: first match wins.
const CATEGORY_RULES = [
  ["Healthcare", /nurse|\brn\b|\brpn\b|psw|personal support|physio|pharmac|medical|dental|clinic|health|therap|care aide|paramedic|physician|caregiver/i],
  ["Software & IT", /software|developer|devops|\bit\b|data (analyst|scien|engineer)|cloud|cyber|security analyst|programmer|full.?stack|front.?end|back.?end|\bqa\b|sre|machine learning|product manager|systems admin/i],
  ["Engineering", /engineer|drafts|cad technician/i],
  ["Finance & Accounting", /account|financ|bookkeep|payroll|audit|tax|credit|bank|controller|actuar|underwrit|insurance/i],
  ["Skilled Trades", /electrician|plumb|hvac|millwright|mechanic|welder|carpent|technician|apprentice|journeyman|trades|maintenance/i],
  ["Logistics & Transport", /driver|truck|warehouse|forklift|logistic|dispatch|supply chain|shipping|courier|delivery/i],
  ["Manufacturing", /production|machine operator|assembl|manufactur|quality (inspector|control)|operator|labourer|laborer/i],
  ["Education", /teacher|educat|tutor|instructor|professor|ece\b|early childhood|school/i],
  ["Hospitality & Food", /cook|chef|server|barista|kitchen|restaurant|hotel|bartend|dishwasher|food|hospitality/i],
  ["Marketing & Media", /marketing|designer|content|social media|brand|communications|copywriter|seo/i],
  ["Sales & Retail", /sales|retail|cashier|store|account executive|business development|customer success|merchandis/i],
  ["Admin & Office", /admin|receptionist|office|clerk|assistant|coordinator|data entry|secretary/i],
];
const categorize = (title) => CATEGORY_RULES.find(([, re]) => re.test(title))?.[0] ?? "Other";

const SENIORITY = { junior: "Entry", mid_level: "Mid", senior: "Senior", staff: "Senior", c_level: "Executive" };
const EMPLOYMENT = { full_time: "Full-time", part_time: "Part-time", contract: "Contract", temporary: "Temporary", internship: "Internship" };

function sourceOf(job) {
  const host = (() => { try { return new URL(job.source_url ?? job.url).hostname; } catch { return ""; } })();
  if (/indeed\./.test(host)) return "Indeed";
  if (/linkedin\./.test(host)) return "LinkedIn";
  if (/myworkdayjobs|workday/.test(host)) return "Workday";
  if (/jobbank\.gc\.ca/.test(host)) return "Job Bank";
  if (/greenhouse|lever\.co|ashbyhq|smartrecruiters|icims|bamboohr/.test(host)) return "Company site";
  return host ? "Other boards" : "Unknown";
}

function cityOf(job) {
  const loc = job.locations?.find((l) => l.state_code === "ON" || l.admin1_name === "Ontario");
  if (loc?.city) return loc.city;
  if (loc?.name) return loc.name;
  return (job.short_location ?? job.location ?? "Ontario").split(",")[0].trim() || "Ontario";
}

const isOntario = (job) =>
  job.state_code === "ON" ||
  job.locations?.some((l) => l.state_code === "ON" || l.admin1_name === "Ontario") ||
  /ontario|\bON\b/.test(job.long_location ?? job.location ?? "");

function annualCad(job) {
  let min = null, max = null;
  if (job.salary_currency === "CAD" && (job.min_annual_salary || job.max_annual_salary)) {
    min = job.min_annual_salary; max = job.max_annual_salary;
  } else if (job.min_annual_salary_usd || job.max_annual_salary_usd) {
    min = job.min_annual_salary_usd && job.min_annual_salary_usd * CAD_PER_USD;
    max = job.max_annual_salary_usd && job.max_annual_salary_usd * CAD_PER_USD;
  }
  const r = (v) => (v ? Math.round(v / 500) * 500 : null);
  min = r(min); max = r(max);
  return { salaryMin: min ?? max, salaryMax: max ?? min };
}

function toRow(job) {
  return {
    id: String(job.id),
    title: job.job_title,
    company: job.company ?? job.company_object?.name ?? "Unknown",
    category: categorize(job.job_title ?? ""),
    city: cityOf(job),
    workMode: job.remote ? "Remote" : job.hybrid ? "Hybrid" : "On-site",
    employmentType: EMPLOYMENT[job.employment_statuses?.[0]] ?? "Unspecified",
    seniority: SENIORITY[job.seniority] ?? "Mid",
    ...annualCad(job),
    postedAt: (job.date_posted ?? job.discovered_at).slice(0, 10),
    closedAt: job.closed_at ? job.closed_at.slice(0, 10) : null,
    source: sourceOf(job),
    url: job.final_url ?? job.url ?? job.source_url ?? null,
  };
}

if (COUNT_ONLY) {
  const res = await search({ ...baseFilters, limit: 1, include_total_results: true });
  console.log(`${res.metadata?.total_results?.toLocaleString() ?? "?"} Ontario postings in the last ${MAX_AGE_DAYS} days`);
  process.exit(0);
}

const jobs = [];
const seen = new Set();
for (let page = 0; jobs.length < MAX_JOBS; page++) {
  const limit = Math.min(PAGE_SIZE, MAX_JOBS - jobs.length);
  const res = await search({ ...baseFilters, limit, page, include_total_results: page === 0 });
  if (page === 0) console.log(`Matching postings: ${res.metadata?.total_results?.toLocaleString() ?? "?"} (fetching up to ${MAX_JOBS})`);
  for (const job of res.data ?? []) {
    if (seen.has(job.id) || !isOntario(job)) continue;
    seen.add(job.id);
    jobs.push(toRow(job));
  }
  process.stdout.write(`\r  fetched ${jobs.length}`);
  if (!res.data?.length || res.data.length < limit) break;
}
process.stdout.write("\n");

const out = { generatedAt: new Date().toISOString().slice(0, 10), sample: false, source: "TheirStack", jobs };
writeFileSync(new URL("../data/jobs.json", import.meta.url), JSON.stringify(out));
console.log(`Wrote ${jobs.length} Ontario postings to data/jobs.json`);
