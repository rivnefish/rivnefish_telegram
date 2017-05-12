extern crate hyper;
extern crate hyper_native_tls;
extern crate serde_json;

const RIVNEFISHURL: &'static str = "https://rivnefish.com/api/v1/places";

#[derive(Deserialize, Debug)]
pub struct RfPlace {
    pub name: String,
    pub id: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RfPlaceInfo {
    pub name: String,
    pub url: String,
    pub description: String,
    pub rating_avg: String,
    pub thumbnail: String,
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
                                                        RfPlaceInfo>(resp) {
                Ok(pi) => Some(pi),
                Err(_) => None,
            },
            Err(_) => None,
        }
    }
}

pub fn get_place_text(place: &RfPlaceInfo) -> String {
    format!("{}\nRating: {}\n{}\n{}",
            place.name, place.rating_avg, place.description, place.url)
}
