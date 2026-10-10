//! Canada's provinces and territories, with the codes each data source uses.

pub struct Region {
    /// Two-letter postal abbreviation, used in URLs and on the map.
    pub code: &'static str,
    pub name: &'static str,
    /// The name as written in Job Bank's "Province/Territory" column.
    pub job_bank: &'static str,
    /// Geography member id in StatCan table 14-10-0444 (job vacancies).
    pub jvws_geo: u32,
    /// Geography member id in StatCan table 17-10-0009 (population estimates).
    pub pop_geo: u32,
}

/// East to west, then the territories, as StatCan orders them.
pub const REGIONS: [Region; 13] = [
    Region { code: "NL", name: "Newfoundland and Labrador", job_bank: "Newfoundland and Labrador", jvws_geo: 2, pop_geo: 2 },
    Region { code: "PE", name: "Prince Edward Island", job_bank: "Prince Edward Island", jvws_geo: 3, pop_geo: 3 },
    Region { code: "NS", name: "Nova Scotia", job_bank: "Nova Scotia", jvws_geo: 4, pop_geo: 4 },
    Region { code: "NB", name: "New Brunswick", job_bank: "New Brunswick", jvws_geo: 5, pop_geo: 5 },
    Region { code: "QC", name: "Quebec", job_bank: "Québec", jvws_geo: 6, pop_geo: 6 },
    Region { code: "ON", name: "Ontario", job_bank: "Ontario", jvws_geo: 7, pop_geo: 7 },
    Region { code: "MB", name: "Manitoba", job_bank: "Manitoba", jvws_geo: 8, pop_geo: 8 },
    Region { code: "SK", name: "Saskatchewan", job_bank: "Saskatchewan", jvws_geo: 9, pop_geo: 9 },
    Region { code: "AB", name: "Alberta", job_bank: "Alberta", jvws_geo: 10, pop_geo: 10 },
    Region { code: "BC", name: "British Columbia", job_bank: "British Columbia", jvws_geo: 11, pop_geo: 11 },
    Region { code: "YT", name: "Yukon", job_bank: "Yukon", jvws_geo: 12, pop_geo: 12 },
    Region { code: "NT", name: "Northwest Territories", job_bank: "Northwest Territories", jvws_geo: 13, pop_geo: 14 },
    Region { code: "NU", name: "Nunavut", job_bank: "Nunavut", jvws_geo: 14, pop_geo: 15 },
];

pub fn by_code(code: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.code == code)
}

pub fn by_job_bank_name(name: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.job_bank == name)
}

pub fn name(code: &str) -> &'static str {
    by_code(code).map_or("Unknown", |r| r.name)
}
