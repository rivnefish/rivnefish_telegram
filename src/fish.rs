extern crate hyper;
extern crate hyper_native_tls;
extern crate serde_json;

const RIVNEFISHURL: &'static str = "https://rivnefish.com/api/v1/places";

#[derive(Deserialize, Debug, Clone)]
pub struct RfPlace {
    pub name: String,
    id: i32,
}

pub fn fetch_all_places() -> Vec<RfPlace> {
    let ssl = hyper_native_tls::NativeTlsClient::new().unwrap();
    let connector = hyper::net::HttpsConnector::new(ssl);
    let client = hyper::Client::with_connector(connector);

    match client.get(RIVNEFISHURL).send() {
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

#[derive(Deserialize, Debug)]
pub struct RfPlaceInfo {
    pub name: String,
    pub url: String,
    pub description: String,
    pub rating_avg: String,
    pub thumbnail: String,
    pub id: i32,
}

pub fn fetch_place_info(place: &RfPlace) -> Option<RfPlaceInfo> {
    let ssl = hyper_native_tls::NativeTlsClient::new().unwrap();
    let connector = hyper::net::HttpsConnector::new(ssl);
    let client = hyper::Client::with_connector(connector);
    let url = format!("{}/{}", RIVNEFISHURL, place.id);

    match client.get(&url).send(){
        Ok(resp) => match serde_json::from_reader::<hyper::client::Response,
                                                    RfPlaceInfo>(resp) {
            Ok(pi) => Some(pi),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

pub fn get_place_text(place: &RfPlaceInfo) -> String {
    format!("{}\nRating: {}\n{}\n{}",
            place.name, place.rating_avg, place.description, place.url)
}
