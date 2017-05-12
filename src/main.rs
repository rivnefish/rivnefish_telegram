extern crate iron;
extern crate persistent;
extern crate router;

#[macro_use]
extern crate serde_derive;

use iron::prelude::*;
use iron::typemap::Key;

use persistent::State;

use std::sync::{Arc, RwLock};

#[allow(dead_code)]
mod telegram;
use telegram::{TgBotApi, TgUpdate,
               TgInlineQuery, TgInlineQueryResult, TgAnswerInlineQuery,
               TgInputMessageContent};

mod fish;

type SafeBotState = Arc<RwLock<<BotState as Key>::Value>>;

fn process_update(st: &SafeBotState, upd: TgUpdate, cfg: &Config) {
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
            println!("IQ id {}, from user {}, query: {}",
                     iq_id, user.id, query_str);

            let matching_places: Vec<fish::RfPlace> = match st.read() {
                Ok(guard) => {
                    let bs = &*guard;

                    let places = &bs.places;

                    let query_upper = query_str.to_uppercase();

                    places.iter()
                        .filter(|p| p.name.to_uppercase().contains(&query_upper))
                        .take(10).cloned().collect()
                },
                Err(_) => Vec::new()
            };

            let infos = matching_places.iter()
                .map(|p| fish::fetch_place_info(p))
                .filter(|opi| opi.is_some())
                .map(|opi| opi.unwrap())
                .map(|ref pi| TgInlineQueryResult {
                    type_: String::from("article"),
                    id: format!("iqid_{}", pi.id),
                    title: pi.name.clone(),
                    description: pi.description.clone(),
                    thumb_url: pi.thumbnail.clone(),
                    input_message_content: TgInputMessageContent {
                        message_text: fish::get_place_text(pi),
                    },
                })
                .collect::<Vec<_>>();

            tg.send_json("/answerInlineQuery", TgAnswerInlineQuery {
                inline_query_id: iq_id,
                results: infos,
            });
        }
        _ => {
            println!("received unsupported update: {:#?}", &upd);
        }
    }
}

struct BotState {
    places: Vec<fish::RfPlace>,
}

impl Key for BotState {
    type Value = BotState;
}

fn reload_places(req: &mut Request) -> IronResult<Response> {
    let new_places = fish::fetch_all_places();

    if let Ok(arc_st) = req.get::<State<BotState>>() {
        if let Ok(mut guard) = arc_st.write() {
            let bs = &mut *guard;

            let places = &mut bs.places;
            *places = new_places;
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
            Ok(upd) => if let Ok(arc_st) = req.get::<State<BotState>>() {
                process_update(&arc_st, upd, cfg);
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
