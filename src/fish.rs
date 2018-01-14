use reqwest;
use serde;
use serde_json;
use time;

const RIVNEFISHURL: &str = "https://rivnefish.com/api/v1";

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
pub struct RfFishingType {
    name: String,
}

#[derive(Deserialize)]
pub struct RfFish {
    id: u32,
    name: String,
}

#[derive(Deserialize)]
pub struct RfFishReport {
    pub fish_id: u32,
    pub qty: Option<u32>,
    pub weight: Option<f32>,
    pub featured: bool,
    pub baits: Vec<String>,
}

#[derive(Deserialize)]
pub struct RfReportInfo {
    pub id: i32,
    pub title: String,
    pub short_description: String,
    pub url: String,
    pub place: Option<RfPlace>,
    pub photos: Vec<RfReportPhoto>,
    pub start_at: String,
    pub rating: Option<u32>,
    pub fishing_types: Vec<RfFishingType>,
    pub featured_image: Option<String>,
    pub report_fishes: Vec<RfFishReport>,
}

#[derive(Deserialize)]
pub struct RfPlace {
    pub name: String,
    pub id: i32,
}

#[derive(Deserialize)]
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
    api: &'a RfApi,
    next_page: Option<usize>,
}

impl<'a> RfPageIter<'a> {
    fn new(api: &'a RfApi) -> Self {
        Self { api, next_page: Some(1) }
    }
}

impl<'a> Iterator for RfPageIter<'a> {
    type Item = Vec<RfReportInfo>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_page.is_none() { return None };

        let pageid = self.next_page.unwrap();
        let page = self.api.fetch_reports_page(pageid);

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

    fn fetch<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, String> {
        self.http_client.get(url).send()
            .map_err(|err| err.to_string())
            .and_then(|r|
                serde_json::from_reader::<reqwest::Response, T>(r)
                    .map_err(|e| e.to_string())
            )
    }

    fn fetch_reports_page(&self, pageid: usize) -> Option<RfPage<RfReportInfo>> {
        let url = format!("{}/{}?page={}", RIVNEFISHURL, "reports", pageid);

        match self.fetch::<RfPage<RfReportInfo>>(&url) {
            Ok(p) => Some(p),
            Err(e) => {
                error!("fetching reports page #{}: {}", pageid, e);
                None
            },
        }
    }

    pub fn fetch_all_fish(&self) -> Vec<RfFish> {
        let url = format!("{}/{}", RIVNEFISHURL, "fish");

        match self.fetch::<Vec<RfFish>>(&url) {
            Ok(fs) => {
                info!("fetched {} kinds of fish", fs.len());
                fs
            },
            Err(e) => {
                error!("fetching fish kinds: {}", e);
                Vec::new()
            },
        }
    }

    pub fn fetch_all_places(&self) -> Vec<RfPlace> {
        let url = format!("{}/{}", RIVNEFISHURL, "places");

        match self.fetch::<Vec<RfPlace>>(&url) {
            Ok(ps) => {
                info!("fetched {} places", ps.len());
                ps
            },
            Err(e) => {
                error!("fetching places: {}", e);
                Vec::new()
            },
        }
    }

    pub fn fetch_place_info(&self, placeid: i32) -> Option<RfPlaceInfo> {
        let url = format!("{}/{}/{}", RIVNEFISHURL, "places", placeid);

        match self.fetch::<RfPlaceInfoRaw>(&url) {
            Ok(pi) => Some(normalize_place_info(pi)),
            Err(e) => {
                error!("fetching place #{}: {}", placeid, e);
                None
            }
        }
    }

    pub fn fetch_report_info(&self, reportid: i32) -> Option<RfReportInfo> {
        let page_iter = RfPageIter::new(&self);
        for rs in page_iter {
            if let Some(rpt) = rs.into_iter().find(|rpt| rpt.id == reportid) {
                return Some(rpt);
            }
        }
        None
    }
}

fn normalize_place_info(pi: RfPlaceInfoRaw) -> RfPlaceInfo {
    RfPlaceInfo {
        name: pi.name,
        thumbnail: pi.thumbnail.unwrap_or_default(),
        featured_image: pi.featured_image.unwrap_or_default(),
        payment_str: match pi.permit.as_ref().map(|s| s.as_str()) {
            Some("paid") => "Платно",
            Some("free") => "Безкоштовно",
            Some("prohibited") => "Риболовля заборонена",
            _ => "Умови невідомі",
        }.to_owned(),
        payment_info: pi.price_notes.unwrap_or_default(),
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
        desc_short: pi.address.unwrap_or_default(),
        url: pi.url,
        id: pi.id,
    }
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
        w = place.important.as_ref().map(|s| format!("&#x26A0; {}\n", s)).unwrap_or_default(),
        a = place.area_str.as_ref().map(|s| format!("&#x25FB; {} ", s)).unwrap_or_default(),
        h = place.hours_str.as_ref().map(|s| format!("&#x23F0; {} ", s)).unwrap_or_default(),
        d = place.update_str.as_ref().map(|s| format!("&#x1F504; {}", s)).unwrap_or_default(),
        c = place.contact_str.as_ref().map(|s| format!("&#x1F4DE; {}\n", s)).unwrap_or_default(),
        p = place.payment_str,
        i = place.payment_info,
    )
}

pub fn get_report_text(report: &RfReportInfo, place: Option<&RfPlaceInfo>, fishes: &[RfFish]) -> String {
    format!(
r#"<b>{t}</b>{fi}
{r}
{p}
<b>Тип рибалки:</b>{f}
<b>Дата:</b> {d}
<b>Спіймана риба:</b>{fs}

<i>{s}</i>"#,
        t = report.title,
        fi = report.featured_image.as_ref()
            .map(|s| format!("<a href=\"{}\">&#160;</a>", s))
            .unwrap_or_default(),
        r = report.rating.map(|r| format!(
            "<b>Оцінка цієї риболовлі</b> {}\n",
            "&#x2B50".repeat(r as usize),
        )).unwrap_or_default(),
        p = place.map(|p| format!(
            "<b>Рибне місце:</b> <a href=\"{pu}\">{pn}</a> (середній рейтинг: {pr})\n",
            pu = p.url,
            pn = p.name,
            pr = p.rating_str,
        )).unwrap_or_default(),
        f = report.fishing_types.iter().fold(
            String::new(),
            |mut acc, x| { acc.push(' '); acc.push_str(&x.name); acc }
        ),
        d = time::strptime(&report.start_at, "%FT%T.%f%z").ok()
            .and_then(|tm| time::strftime("%F", &tm).ok())
            .unwrap_or_default(),
        fs = report.report_fishes.iter().fold(
            String::new(),
            |mut acc, x| { acc.push_str(&format!(
                "\n&#x2022 {i}{q}{w}{t}{b}",

                i = fishes.iter().find(|f| f.id == x.fish_id).map(|f| f.name.as_str()).unwrap_or("?"),
                q = x.qty.map(|n| format!(" {}шт", n)).unwrap_or_default(),
                w = x.weight.map(|n| format!(" {}кг", n)).unwrap_or_default(),
                t = if x.featured {" &#x1F3C6"} else {""},
                b = if !x.baits.is_empty() {
                    let mut res = x.baits.iter().fold(
                        " (".to_owned(),
                        |mut acc, x| { acc.push(' '); acc.push_str(x); acc },
                    );
                    res.push_str(")");
                    res
                } else { "".to_owned() },
            )); acc }
        ),
        s = report.short_description,
    )
}
