extern crate iron;
extern crate persistent;
extern crate router;
extern crate time;

#[macro_use]
extern crate serde_derive;

use iron::prelude::*;
use iron::typemap::Key;

use persistent::State;

use time::PreciseTime;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;

#[allow(dead_code)]
mod telegram;
use telegram::{TgBotApi, TgUpdate,
               TgInlineQuery, TgInlineQueryResult, TgAnswerInlineQuery,
               TgInlineKeyboardMarkup, TgInlineKeyboardButton,
               TgInputMessageContent};

mod fish;
use fish::{RfApi, RfPlace, RfPlaceInfo};

fn get_info_for(st: &SafeBotState, rfapi: &RfApi, id: i32) -> Option<RfPlaceInfo> {
    match st.read() {
        Ok(guard) => {
            let state = &*guard;
            let cache = &state.cache;

            if let Some(info) = cache.get(&id) {
                return info.clone()
            }
        },
        Err(_) => return None
    }

    let fetched = rfapi.fetch_place_info(id);

    if let Ok(mut guard) = st.write() {
        let state = &mut *guard;
        let cache = &mut state.cache;

        cache.insert(id, fetched.clone());
    }

    fetched
}

type SafeBotState = Arc<RwLock<<BotState as Key>::Value>>;

fn process_update(st: &SafeBotState, upd: TgUpdate, updstr: String, cfg: &Config) {
    let tg = TgBotApi::new(&cfg.bottoken);
    match upd {
        TgUpdate {
            message: None,
            callback_query: None,
            inline_query: Some(TgInlineQuery {
                id: iq_id,
                from: user,
                query: query_str,
                offset: _,
            }),
            ..
        } => {
            let t0 = PreciseTime::now();

            let matching_ids: Vec<i32> = match st.read() {
                Ok(guard) => {
                    let state = &*guard;
                    let places = &state.places;

                    let query_upper = query_str.to_uppercase();

                    places.iter()
                        .filter(|p| p.name.to_uppercase().contains(&query_upper))
                        .map(|p| p.id).take(10).collect()
                },
                Err(_) => Vec::new()
            };

            let rfapi = fish::RfApi::new();

            let infos = matching_ids.iter()
                .map(|i| get_info_for(st, &rfapi, *i))
                .filter(|ci| ci.is_some())
                .map(|ci| ci.unwrap())
                .map(|pi| TgInlineQueryResult {
                    type_: "article".to_owned(),
                    id: format!("iqid_{}", pi.id),
                    title: pi.name.clone(),
                    description: fish::get_place_short_desc(&pi, 100),
                    url: pi.url.clone(),
                    hide_url: true,
                    thumb_url: pi.thumbnail.clone().unwrap_or("".to_owned()),
                    input_message_content: TgInputMessageContent {
                        message_text: fish::get_place_text(&pi),
                        parse_mode: "HTML".to_owned(),
                        disable_web_page_preview: false,
                    },
                    reply_markup: Some(TgInlineKeyboardMarkup {
                        inline_keyboard: vec![vec![TgInlineKeyboardButton {
                            text: "детальніше".to_owned(),
                            url: Some(pi.url.clone()),
                            callback_data: None,
                        }]]
                    }),
                })
                .collect::<Vec<_>>();

            let t1 = PreciseTime::now();

            println!("IQ id {}, from user {}, query: {}, took {}",
                     iq_id, user.id, query_str, t0.to(t1));

            tg.send_json("/answerInlineQuery", TgAnswerInlineQuery {
                inline_query_id: iq_id,
                results: infos,
            });
        }
        _ => {
            println!("received unsupported update: {:#?}", &upd);
            println!("original text: {}", &updstr);
        }
    }
}

struct BotState {
    places: Vec<RfPlace>,
    cache: HashMap<i32, Option<RfPlaceInfo>>,
}

impl Key for BotState {
    type Value = BotState;
}

fn reload_places(req: &mut Request) -> IronResult<Response> {
    let rfapi = fish::RfApi::new();
    let new_places = rfapi.fetch_all_places();

    if let Ok(arc_st) = req.get::<State<BotState>>() {
        if let Ok(mut guard) = arc_st.write() {
            let bs = &mut *guard;

            let places = &mut bs.places;
            *places = new_places;

            let cache = &mut bs.cache;
            cache.clear();
        }
    }

    Ok(Response::with(iron::status::Ok))
}

#[allow(dead_code)]
struct Config {
    botname: String,
    bottoken: String,
}

fn main() {

    let mut router = router::Router::new();

    let config = Config {
        botname: std::env::var("RVFISH_BOTNAME").unwrap_or(String::from("@")),
        bottoken: std::env::var("RVFISH_BOTTOKEN").unwrap_or(String::from("")),
    };

    fn bot(req: &mut Request, cfg: &Config) -> IronResult<Response> {
        match telegram::read_update(&mut req.body) {
            Ok((upd, updstr)) => if let Ok(arc_st) = req.get::<State<BotState>>() {
                process_update(&arc_st, upd, updstr, cfg);
            },
            Err(err) => println!("read_update error: {}", err),
        }

        Ok(Response::with(iron::status::Ok))
    }

    let bot_handler = move |req: &mut Request| bot(req, &config);

    let listenpath: &str = &std::env::var("RVFISH_LISTENPATH")
        .unwrap_or(String::from("/bot"));

    let reload_handler = |req: &mut Request| reload_places(req);

    router.post(listenpath, bot_handler, "bot");
    router.get("/reload_places", reload_handler, "reload");

    let botstate = BotState {
        places: Vec::new(),
        cache: HashMap::new(),
    };

    let mut chain = Chain::new(router);
    chain.link(State::<BotState>::both(botstate));

    let listenaddr: &str = &std::env::var("RVFISH_LISTENADDR")
        .unwrap_or(String::from("localhost:2358"));

    match Iron::new(chain).http(listenaddr) {
        Ok(_) => {}
        Err(e) => println!("iron http failure {}", e.to_string())
    }
}
