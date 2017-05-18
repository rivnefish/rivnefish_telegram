extern crate hyper;
extern crate hyper_native_tls;
extern crate serde_json;
extern crate time;

const RIVNEFISHURL: &'static str = "https://rivnefish.com/api/v1/places";

#[derive(Deserialize, Debug)]
pub struct RfPlace {
    pub name: String,
    pub id: i32,
}

#[derive(Deserialize, Debug)]
pub struct RfPlaceInfoRaw {
    pub name: String,
    pub url: String,
    pub address: Option<String>,
    pub rating_avg: Option<String>,
    pub rating_votes: Option<i32>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub thumbnail: Option<String>,
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
    pub payment_str: String,
    pub payment_info: String,
    pub rating_str: String,
    pub votes: i32,
    pub area_str: Option<String>,
    pub hours_str: Option<String>,
    pub update_str: Option<String>,
    pub contact_str: Option<String>,
    pub desc_short: String,
    pub url: String,
    pub id: i32,
}

pub struct RfApi {
    http_client: hyper::Client,
}

impl RfApi {
    pub fn new() -> RfApi {
        let ssl = hyper_native_tls::NativeTlsClient::new().unwrap();
        let connector = hyper::net::HttpsConnector::new(ssl);
        let client = hyper::Client::with_connector(connector);
        RfApi {
            http_client: client,
        }
    }

    pub fn fetch_all_places(&self) -> Vec<RfPlace> {
        match self.http_client.get(RIVNEFISHURL).send() {
            Ok(resp) => {
                match serde_json::from_reader::<hyper::client::Response,
                                                Vec<RfPlace>>(resp) {
                    Ok(ps) => ps,
                    Err(e) => {
                        println!("error parsing rivnefish places: {}", &e);
                        Vec::new()
                    }
                }
            },
            Err(e) => {
                println!("error reading rivnefish response: {}", &e);
                Vec::new()
            }
        }
    }

    pub fn fetch_place_info(&self, placeid: i32) -> Option<RfPlaceInfo> {
        let url = format!("{}/{}", RIVNEFISHURL, placeid);

        match self.http_client.get(&url).send(){
            Ok(resp) => match serde_json::from_reader::<hyper::client::Response,
                                                        RfPlaceInfoRaw>(resp) {
                Ok(pi) => Some(normalize_place_info(pi)),
                Err(err) => {
                    println!("error parsing rivnefish place {}", err);
                    None
                },
            },
            Err(err) => {
                println!("error fetching rivnefish place {}", err);
                None
            }
        }
    }
}

fn normalize_place_info(pi: RfPlaceInfoRaw) -> RfPlaceInfo {
    RfPlaceInfo {
        name: pi.name,
        thumbnail: pi.thumbnail.unwrap_or("".to_owned()),
        payment_str: match pi.permit.as_ref().map(|s| s.as_str()) {
            Some("paid") => "Платно",
            Some("free") => "Безкоштовно",
            Some("prohibited") => "Риболовля заборонена",
            _ => "Умови невідомі",
        }.to_owned(),
        payment_info: pi.price_notes.unwrap_or("".to_owned()),
        rating_str: pi.rating_avg.unwrap_or("--".to_owned()),
        votes: pi.rating_votes.unwrap_or(0),
        area_str: match pi.area {
            Some(ref s) => Some(format!("{}Га", s)),
            None => None,
        },
        hours_str: match pi.time_to_fish.as_ref().map(|s| s.as_str()) {
            Some("full_day") => Some("цілодобово".to_owned()),
            Some("day_only") => Some("вдень".to_owned()),
            _ => None
        },
        update_str: pi.info_updated_at
            .and_then(|s| time::strptime(&s, "%FT%T.%f%z").ok())
            .and_then(|tm| time::strftime("%F", &tm).ok()),
        contact_str: match (pi.contact_phone, pi.contact_name) {
            (Some(ref p), _) if p.len() == 0 => None,
            (Some(p), Some(n)) =>
                Some(format!("{}{} {}",
                             if p.starts_with("380")
                             {"+"} else {""},
                             p, n)),
            (Some(p), None) => Some(p),
            _ => None,
        },
        desc_short: pi.address.unwrap_or("".to_owned()),
        url: pi.url,
        id: pi.id,
    }
}

#[allow(dead_code)]
fn get_place_short_desc(long_desc: &str, sz: usize) -> String {
    let end = long_desc.char_indices().map(|(p, _)| p).nth(sz);
    let short_desc = &long_desc[..end.unwrap_or(long_desc.len())];
    format!("{}{}", short_desc, end.map_or("", |_| "..."))
}

pub fn get_place_text(place: &RfPlaceInfo) -> String {
    format!(r#"<b>{n}</b><a href="{t}">&#160;</a>
&#x2B50; {r} <a href="{u}/reports">(звітів: {v})</a>

{a}{h}{d}
{c}
&#x1F4B2; {p}
{i}"#,
            n = place.name, t = place.thumbnail,
            r = place.rating_str, u = place.url, v = place.votes,
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
            p = place.payment_str, i = place.payment_info)
}
