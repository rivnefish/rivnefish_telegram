use reqwest;
use serde_json;
use time;

const RIVNEFISHURL: &'static str = "https://rivnefish.com/api/v1";

#[derive(Deserialize)]
struct RfPagination {
    next_page: Option<usize>,
}

#[derive(Deserialize)]
struct RfPage<Entry> {
    meta: RfPagination,
    data: Vec<Entry>,
}

#[derive(Deserialize)]
pub struct RfReportPhoto {
    pub medium_url: String,
}

#[derive(Deserialize)]
pub struct RfReportInfo {
    pub id: i32,
    pub title: String,
    pub short_description: String,
    pub url: String,
    //pub place: RfPlace,
    pub photos: Vec<RfReportPhoto>,
}

#[derive(Deserialize, Debug)]
pub struct RfPlace {
    pub name: String,
    pub id: i32,
}

#[derive(Deserialize, Debug)]
pub struct RfPlaceInfoRaw {
    pub name: String,
    pub url: String,
    pub notes: Option<String>,
    pub address: Option<String>,
    pub rating_avg: Option<String>,
    pub rating_votes: Option<i32>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub thumbnail: Option<String>,
    pub featured_image: Option<String>,
    pub permit: Option<String>, // "free", "paid", "prohibited"
    pub area: Option<String>,
    pub time_to_fish: Option<String>, // "full_day", "day_only"
    pub price_notes: Option<String>,
    pub info_updated_at: Option<String>,
    pub id: i32,
}

#[derive(Clone)]
pub struct RfPlaceInfo {
    pub name: String,
    pub thumbnail: String,
    pub featured_image: String,
    pub payment_str: String,
    pub payment_info: String,
    pub rating_str: String,
    pub votes: i32,
    pub important: Option<String>,
    pub area_str: Option<String>,
    pub hours_str: Option<String>,
    pub update_str: Option<String>,
    pub contact_str: Option<String>,
    pub desc_short: String,
    pub url: String,
    pub id: i32,
}

struct RfPageIter<'a> {
    http_client: &'a reqwest::Client,
    next_page: Option<usize>,
}

impl<'a> RfPageIter<'a> {
    fn new(client: &'a reqwest::Client) -> Self {
        Self {
            http_client: client,
            next_page: Some(1),
        }
    }
}

impl<'a> Iterator for RfPageIter<'a> {
    type Item = Vec<RfReportInfo>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_page.is_none() { return None };

        let pageid = self.next_page.unwrap();
        let url = format!("{}/{}?page={}", RIVNEFISHURL, "reports", pageid);

        let page = match self.http_client.get(&url).send() {
            Ok(resp) => match serde_json::from_reader::<reqwest::Response, RfPage<RfReportInfo>>(resp) {
                Ok(page) => Some(page),
                Err(err) => {
                    error!("error parsing rivnefish reports page #{}: {}", pageid, err);
                    None
                },
            },
            Err(err) => {
                error!("error fetching rivnefish reports page #{}: {}", pageid, err);
                None
            },
        };

        self.next_page = page.as_ref().and_then(|p| p.meta.next_page);

        page.map(|p| p.data)
    }
}

pub struct RfApi {
    http_client: reqwest::Client,
}

impl RfApi {
    pub fn new() -> RfApi {
        RfApi {
            http_client: reqwest::Client::new(),
        }
    }

    pub fn fetch_all_places(&self) -> Vec<RfPlace> {
        let url = format!("{}/{}", RIVNEFISHURL, "places");

        match self.http_client.get(&url).send() {
            Ok(resp) => match serde_json::from_reader::<reqwest::Response, Vec<RfPlace>>(resp) {
                Ok(ps) => {
                    info!("fetched {} places", ps.len());
                    ps
                }
                Err(e) => {
                    error!("error parsing rivnefish places: {}", &e);
                    Vec::new()
                }
            },
            Err(e) => {
                error!("error reading rivnefish response: {}", &e);
                Vec::new()
            }
        }
    }

    pub fn fetch_place_info(&self, placeid: i32) -> Option<RfPlaceInfo> {
        let url = format!("{}/{}/{}", RIVNEFISHURL, "places", placeid);

        match self.http_client.get(&url).send() {
            Ok(resp) => match serde_json::from_reader::<reqwest::Response, RfPlaceInfoRaw>(resp) {
                Ok(pi) => Some(normalize_place_info(pi)),
                Err(err) => {
                    error!("error parsing rivnefish place {}", err);
                    None
                }
            },
            Err(err) => {
                error!("error fetching rivnefish place {}", err);
                None
            }
        }
    }

    pub fn fetch_report_info(&self, reportid: i32) -> Option<RfReportInfo> {
        let mut page_iter = RfPageIter::new(&self.http_client);
        while let Some(mut rs) = page_iter.next() {
            if let Some(i) = rs.iter().position(|rpt| rpt.id == reportid) {
                return Some(rs.swap_remove(i)) // Vec<T> -> T (at i) without T::clone()
            }
        }
        None
    }
}

fn normalize_place_info(pi: RfPlaceInfoRaw) -> RfPlaceInfo {
    RfPlaceInfo {
        name: pi.name,
        thumbnail: pi.thumbnail.unwrap_or_else(|| "".to_owned()),
        featured_image: pi.featured_image.unwrap_or_else(|| "".to_owned()),
        payment_str: match pi.permit.as_ref().map(|s| s.as_str()) {
            Some("paid") => "Платно",
            Some("free") => "Безкоштовно",
            Some("prohibited") => "Риболовля заборонена",
            _ => "Умови невідомі",
        }.to_owned(),
        payment_info: pi.price_notes.unwrap_or_else(|| "".to_owned()),
        rating_str: pi.rating_avg.unwrap_or_else(|| "--".to_owned()),
        votes: pi.rating_votes.unwrap_or(0),
        important: match pi.notes {
            Some(ref p) if p.is_empty() => None,
            Some(p) => Some(p),
            _ => None,
        },
        area_str: match pi.area {
            Some(ref s) => Some(format!("{}Га", s)),
            None => None,
        },
        hours_str: match pi.time_to_fish.as_ref().map(|s| s.as_str()) {
            Some("full_day") => Some("цілодобово".to_owned()),
            Some("day_only") => Some("вдень".to_owned()),
            _ => None,
        },
        update_str: pi.info_updated_at
            .and_then(|s| time::strptime(&s, "%FT%T.%f%z").ok())
            .and_then(|tm| time::strftime("%F", &tm).ok()),
        contact_str: match (pi.contact_phone, pi.contact_name) {
            (Some(ref p), _) if p.is_empty() => None,
            (Some(p), Some(n)) => Some(format!(
                "{}{} {}",
                if p.starts_with("380") { "+" } else { "" },
                p,
                n
            )),
            (Some(p), None) => Some(p),
            _ => None,
        },
        desc_short: pi.address.unwrap_or_else(|| "".to_owned()),
        url: pi.url,
        id: pi.id,
    }
}

#[allow(dead_code)]
fn get_place_short_desc(long_desc: &str, sz: usize) -> String {
    let end = long_desc.char_indices().map(|(p, _)| p).nth(sz);
    let short_desc = &long_desc[..end.unwrap_or_else(|| long_desc.len())];
    format!("{}{}", short_desc, end.map_or("", |_| "..."))
}

pub fn get_place_text(place: &RfPlaceInfo) -> String {
    format!(
        r#"<b>{n}</b><a href="{t}">&#160;</a>
&#x2B50; {r} <a href="{u}/reports">(звітів: {v})</a>
{w}
{a}{h}{d}
{c}
&#x1F4B2; {p}
{i}"#,
        n = place.name,
        t = place.featured_image,
        r = place.rating_str,
        u = place.url,
        v = place.votes,
        w = match place.important {
            Some(ref s) => format!("&#x26A0; {}\n", s),
            None => "".to_owned(),
        },
        a = match place.area_str {
            Some(ref s) => format!("&#x25FB; {} ", s),
            None => "".to_owned(),
        },
        h = match place.hours_str {
            Some(ref s) => format!("&#x23F0; {} ", s),
            None => "".to_owned(),
        },
        d = match place.update_str {
            Some(ref s) => format!("&#x1F504; {}", s),
            None => "".to_owned(),
        },
        c = match place.contact_str {
            Some(ref s) => format!("&#x1F4DE; {}\n", s),
            None => "".to_owned(),
        },
        p = place.payment_str,
        i = place.payment_info
    )
}

pub fn get_report_text(report: &RfReportInfo) -> String {
    format!("*{t}*\n_{d}_", t = report.title, d = report.short_description)
}
