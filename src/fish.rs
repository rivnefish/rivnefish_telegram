extern crate hyper;
extern crate hyper_native_tls;
extern crate serde_json;

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
    pub description: String,
    pub rating_avg: Option<String>,
    pub rating_votes: Option<i32>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub thumbnail: Option<String>,
    pub permit: Option<String>, // "free", "paid"
    pub area: Option<String>,
    pub time_to_fish: Option<String>, // "full_day", "day_only"
    pub price_notes: Option<String>,
    pub id: i32,
}

#[derive(Clone)]
pub struct RfPlaceInfo {
    pub name: String,
    pub thumbnail: String,
    pub payment_str: String,
    pub rating_str: String,
    pub votes: i32,
    pub area_str: Option<String>,
    pub hours_str: Option<String>,
    pub contact_str: Option<String>,
    pub desc: String,
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
        payment_str: match (pi.permit.as_ref().map(|s| s.as_str()),
                            pi.price_notes) {
            (Some("paid"), Some(n)) => format!("{}: {}", "Платно", n),
            (Some("paid"), None) => "Платно".to_owned(),
            (Some("free"), Some(n)) => format!("{}: {}", "Безкоштовно", n),
            (Some("free"), None) => "Безкоштовно".to_owned(),
            _ => "".to_owned(),
        },
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
        contact_str: match (pi.contact_phone, pi.contact_name) {
            (Some(p), Some(n)) => Some(format!("{}{} {}",
                                               if p.starts_with("380")
                                               {"+"} else {""},
                                               p, n)),
            (Some(p), None) => Some(p),
            _ => None,
        },
        desc: get_place_short_desc(&pi.description, 300),
        desc_short: get_place_short_desc(&pi.description, 100),
        url: pi.url,
        id: pi.id,
    }
    
}

pub fn get_place_short_desc(long_desc: &str, sz: usize) -> String {
    let end = long_desc.char_indices().map(|(p, _)| p).nth(sz);
    let short_desc = &long_desc[..end.unwrap_or(long_desc.len())];
    format!("{}{}", short_desc, end.map_or("", |_| "..."))
}

pub fn get_place_text(place: &RfPlaceInfo) -> String {
    format!(r#"<b>{}</b><a href="{}">&#160;</a>
<i>{}</i>
&#x2B50;{} <a href="{}/reports">(звітів: {})</a>
{}{}
{}

{}"#,
            place.name, place.thumbnail,
            place.payment_str,
            place.rating_str, place.url, place.votes,
            match place.area_str {
                Some(ref s) => format!("&#x25FB;{} ", s),
                None => "".to_owned(),
            },
            match place.hours_str {
                Some(ref s) => format!("&#x23F0;{}", s),
                None => "".to_owned(),
            },
            match place.contact_str {
                Some(ref s) => format!("&#x1F4DE;{}", s),
                None => "".to_owned(),
            },
            place.desc)
}
