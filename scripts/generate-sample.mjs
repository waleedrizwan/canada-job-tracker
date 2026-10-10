// Generates a realistic-looking but entirely synthetic Ontario job postings
// dataset in the same shape the dashboard expects (see README "Data schema").
// Deterministic: the same seed always yields the same file.
import { writeFileSync } from "node:fs";

const COUNT = Number(process.argv[2] ?? 4000);
const TODAY = new Date(process.env.AS_OF ?? new Date().toISOString().slice(0, 10));

let seed = 20261010;
const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
const pick = (arr) => arr[Math.floor(rand() * arr.length)];
const weighted = (pairs) => {
  const total = pairs.reduce((s, [, w]) => s + w, 0);
  let r = rand() * total;
  for (const [v, w] of pairs) if ((r -= w) <= 0) return v;
  return pairs[pairs.length - 1][0];
};
const normal = () => Math.sqrt(-2 * Math.log(rand() || 1e-9)) * Math.cos(2 * Math.PI * rand());

// [category, weight, median annual CAD, titles]
const CATEGORIES = [
  ["Software & IT", 14, 98000, ["Software Developer", "Data Analyst", "DevOps Engineer", "IT Support Specialist", "QA Analyst", "Cloud Engineer", "Product Manager"]],
  ["Healthcare", 15, 72000, ["Registered Nurse", "Personal Support Worker", "Pharmacy Technician", "Medical Office Assistant", "Physiotherapist", "Dental Hygienist"]],
  ["Finance & Accounting", 8, 78000, ["Accountant", "Financial Analyst", "Bookkeeper", "Payroll Administrator", "Credit Analyst"]],
  ["Skilled Trades", 10, 68000, ["Electrician", "Plumber", "HVAC Technician", "Millwright", "Automotive Technician", "Welder"]],
  ["Sales & Retail", 12, 48000, ["Retail Sales Associate", "Account Executive", "Store Manager", "Customer Success Rep", "Cashier"]],
  ["Admin & Office", 7, 50000, ["Administrative Assistant", "Receptionist", "Office Manager", "Data Entry Clerk"]],
  ["Education", 5, 70000, ["Early Childhood Educator", "Teacher", "Education Assistant", "Tutor"]],
  ["Manufacturing", 8, 52000, ["Production Worker", "Machine Operator", "Quality Inspector", "Assembler"]],
  ["Hospitality & Food", 9, 38000, ["Line Cook", "Server", "Barista", "Hotel Front Desk Agent", "Restaurant Supervisor"]],
  ["Logistics & Transport", 7, 55000, ["AZ Truck Driver", "Warehouse Associate", "Forklift Operator", "Dispatcher", "Supply Chain Coordinator"]],
  ["Engineering", 4, 92000, ["Mechanical Engineer", "Civil Engineer", "Electrical Engineer", "Project Engineer"]],
  ["Marketing & Media", 3, 66000, ["Marketing Coordinator", "Digital Marketing Specialist", "Graphic Designer", "Content Writer"]],
];

const CITIES = [
  ["Toronto", 32], ["Ottawa", 10], ["Mississauga", 9], ["Brampton", 5], ["Hamilton", 5],
  ["Markham", 4], ["Vaughan", 4], ["London", 4], ["Kitchener", 3], ["Waterloo", 3],
  ["Windsor", 3], ["Oshawa", 2], ["Barrie", 2], ["Guelph", 2], ["Kingston", 2],
  ["Greater Sudbury", 2], ["Thunder Bay", 1], ["Peterborough", 1],
];

// Source mix loosely follows TheirStack's published Canadian breakdown.
const SOURCES = [["Indeed", 46.5], ["LinkedIn", 15.7], ["Company site", 18], ["Workday", 4.3], ["Job Bank", 2.9], ["Other boards", 12.6]];

const COMPANY_PREFIX = ["Maple", "Northern", "Lakeshore", "Great Lakes", "Trillium", "Huron", "Algonquin", "Laurentian", "Niagara", "Rideau", "Superior", "Bayview", "Ridgeway", "Cedar", "Pinecrest"];
const COMPANY_SUFFIX = ["Health", "Systems", "Logistics", "Foods", "Group", "Technologies", "Manufacturing", "Financial", "Builders", "Services", "Labs", "Retail"];
const companies = Array.from({ length: 220 }, () => `${pick(COMPANY_PREFIX)} ${pick(COMPANY_SUFFIX)}`);

const SENIORITY = [["Entry", 30], ["Mid", 42], ["Senior", 22], ["Executive", 6]];
const SENIORITY_MULT = { Entry: 0.78, Mid: 1, Senior: 1.3, Executive: 1.75 };

const isoDaysAgo = (d) => new Date(TODAY.getTime() - d * 86400000).toISOString().slice(0, 10);
const round = (n, step) => Math.round(n / step) * step;

const jobs = [];
for (let i = 0; i < COUNT; i++) {
  const [category, , median, titles] = weighted(CATEGORIES.map((c) => [c, c[1]]));
  const seniority = weighted(SENIORITY);
  const remoteFriendly = ["Software & IT", "Marketing & Media", "Finance & Accounting", "Admin & Office"].includes(category);
  const workMode = weighted(remoteFriendly ? [["On-site", 45], ["Hybrid", 38], ["Remote", 17]] : [["On-site", 88], ["Hybrid", 9], ["Remote", 3]]);
  const employmentType = weighted([["Full-time", 72], ["Part-time", 16], ["Contract", 9], ["Temporary", 3]]);

  // Posting age: most are fresh, with a long stale tail (ghost/forgotten postings).
  const ageDays = rand() < 0.62 ? Math.floor(rand() * 30) : 30 + Math.floor(-Math.log(rand() || 1e-9) * 35);
  // A slice of postings has been detected as closed by the source.
  const closed = rand() < Math.min(0.5, ageDays / 160);

  let salaryMin = null, salaryMax = null;
  if (rand() < 0.62) {
    const mid = median * SENIORITY_MULT[seniority] * Math.exp(normal() * 0.18);
    salaryMin = round(mid * 0.9, 1000);
    salaryMax = round(mid * 1.12, 1000);
  }

  jobs.push({
    id: `S${String(i + 1).padStart(5, "0")}`,
    title: `${seniority === "Senior" ? "Senior " : seniority === "Executive" ? "Director, " : ""}${pick(titles)}`,
    company: pick(companies),
    category,
    city: weighted(CITIES),
    workMode,
    employmentType,
    seniority,
    salaryMin,
    salaryMax,
    postedAt: isoDaysAgo(ageDays),
    closedAt: closed ? isoDaysAgo(Math.floor(rand() * ageDays)) : null,
    source: weighted(SOURCES),
    url: null,
  });
}

const out = { generatedAt: TODAY.toISOString().slice(0, 10), sample: true, source: "Synthetic sample data", jobs };
writeFileSync(new URL("../data/jobs.json", import.meta.url), JSON.stringify(out));
console.log(`Wrote ${jobs.length} sample jobs to data/jobs.json`);
