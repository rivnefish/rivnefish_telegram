extern crate bodyparser;
extern crate iron;
extern crate persistent;
extern crate router;
extern crate time;

extern crate env_logger;
#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

use iron::prelude::*;
use iron::typemap::Key;

use persistent::{Read, State};

use time::PreciseTime;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;

#[allow(dead_code)]
mod telegram;
use telegram::{TgAnswerInlineQuery, TgBotApi, TgChosenInlineResult, TgInlineKeyboardButton,
               TgInlineKeyboardMarkup, TgInlineQuery, TgInlineQueryResult, TgInputMessageContent,
               TgResponse, TgUpdate};

mod fish;
use fish::{RfApi, RfPlace, RfPlaceInfo};

fn get_info_for(st: &SafeBotState, rfapi: &RfApi, id: i32) -> Option<RfPlaceInfo> {
    match st.read() {
        Ok(guard) => {
            let state = &*guard;
            let cache = &state.cache;

            if let Some(info) = cache.get(&id) {
                return info.clone();
            }
        }
        Err(_) => return None,
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

fn process_update(st: &SafeBotState, upd: TgUpdate, updstr: &str, cfg: &Config) {
    let tg = TgBotApi::new(&cfg.bottoken);
    match upd {
        TgUpdate {
            chosen_inline_result: Some(TgChosenInlineResult {
                result_id,
                inline_message_id: Some(imi),
                ..
            }),
            ..
        } => {
            info!("CIR: resuldid: {}, inline msg id: {}", result_id, imi);
        }
        TgUpdate {
            message: None,
            callback_query: None,
            inline_query: Some(TgInlineQuery {
                id: iq_id,
                from: user,
                query: query_str,
                ..
            }),
            ..
        } => {
            let t0 = PreciseTime::now();

            let matching_ids: Vec<i32> = match st.read() {
                Ok(guard) => {
                    let state = &*guard;
                    let places = &state.places;

                    let query_upper = query_str.to_uppercase();

                    if query_upper.is_empty() {
                        state.top_ids.clone()
                    } else {
                        places
                            .iter()
                            .filter(|p| p.name.to_uppercase().contains(&query_upper))
                            .map(|p| p.id)
                            .take(10)
                            .collect()
                    }
                }
                Err(_) => Vec::new(),
            };

            let rfapi = fish::RfApi::new();

            let infos = matching_ids
                .iter()
                .map(|i| get_info_for(st, &rfapi, *i))
                .filter(|ci| ci.is_some())
                .map(|ci| ci.unwrap())
                .map(|pi| {
                    let txt = fish::get_place_text(&pi);
                    TgInlineQueryResult {
                        type_: "article".to_owned(),
                        id: format!("iqid_{}", pi.id),
                        title: pi.name,
                        description: pi.desc_short,
                        url: pi.url.clone(),
                        hide_url: true,
                        thumb_url: pi.thumbnail,
                        input_message_content: TgInputMessageContent {
                            message_text: txt,
                            parse_mode: "HTML".to_owned(),
                            disable_web_page_preview: false,
                        },
                        reply_markup: Some(TgInlineKeyboardMarkup {
                            inline_keyboard: vec![
                                vec![
                                    TgInlineKeyboardButton::Url {
                                        text: "детальніше на вебсайті"
                                            .to_owned(),
                                        url: pi.url,
                                    },
                                ],
                            ],
                        }),
                    }
                })
                .collect::<Vec<_>>();

            let t1 = PreciseTime::now();

            info!(
                "IQ id {}, from user '{}' ({}), query: `{}`, took {}",
                iq_id,
                telegram::make_name(&user),
                user.id,
                query_str,
                t0.to(t1)
            );

            let resp: Result<TgResponse<bool>, String> = tg.send_json_recv_json(
                "/answerInlineQuery",
                TgAnswerInlineQuery {
                    inline_query_id: iq_id,
                    results: infos,
                },
            );

            if resp.is_err() {
                error!("error answering IQ: {:#?}", resp);
            }
        }
        _ => {
            warn!("received unsupported update: {:#?}", &upd);
            debug!("original text: {}", updstr);
        }
    }
}

struct BotState {
    places: Vec<RfPlace>,
    cache: HashMap<i32, Option<RfPlaceInfo>>,
    top_ids: Vec<i32>,
}

impl Key for BotState {
    type Value = BotState;
}

fn modify_bot_state<F>(req: &mut Request, f: F)
where
    F: FnOnce(&mut BotState),
{
    if let Ok(arc_st) = req.get::<State<BotState>>() {
        if let Ok(mut guard) = arc_st.write() {
            let bs = &mut *guard;

            f(bs);
        }
    }
}

fn reload_places(req: &mut Request) -> IronResult<Response> {
    let rfapi = fish::RfApi::new();
    let new_places = rfapi.fetch_all_places();

    modify_bot_state(req, |bs: &mut BotState| {
        bs.places = new_places;
        bs.cache.clear();
        info!("reloaded place list and invalidated cache");
    });

    Ok(Response::with(iron::status::Ok))
}

#[derive(Deserialize, Clone)]
struct TopIds {
    ids: Vec<i32>,
}

fn set_top(req: &mut Request) -> IronResult<Response> {
    match req.get::<bodyparser::Struct<TopIds>>() {
        Ok(Some(s)) => modify_bot_state(req, |bs: &mut BotState| {
            bs.top_ids = s.ids.clone();
            info!("updated top fishing places with {} items", bs.top_ids.len());
        }),
        Ok(None) => info!("/set_top request has empty body"),
        Err(err) => error!("/set_top: {:?}", err),
    }

    Ok(Response::with(iron::status::Ok))
}

#[allow(dead_code)]
struct Config {
    botname: String,
    bottoken: String,
}

fn main() {
    let format_fn = |record: &log::LogRecord| {
        let t = time::now();
        format!(
            "{} {} [{}] {}",
            time::strftime("%Y-%m-%d %H:%M:%S", &t).unwrap(),
            record.level(),
            record.location().module_path(),
            record.args()
        )
    };

    let mut log_builder = env_logger::LogBuilder::new();
    log_builder
        .format(format_fn)
        .filter(None, log::LogLevelFilter::Info);

    if let Ok(ref lcfg) = std::env::var("RUST_LOG") {
        log_builder.parse(lcfg);
    }

    if let Err(e) = log_builder.init() {
        panic!("unable to start {}", e);
    }

    let mut router = router::Router::new();

    let config = Config {
        botname: std::env::var("RVFISH_BOTNAME").unwrap_or_else(|_| "@".to_owned()),
        bottoken: std::env::var("RVFISH_BOTTOKEN").unwrap_or_else(|_| "".to_owned()),
    };

    fn bot(req: &mut Request, cfg: &Config) -> IronResult<Response> {
        match telegram::read_update(&mut req.body) {
            Ok((upd, updstr)) => if let Ok(arc_st) = req.get::<State<BotState>>() {
                process_update(&arc_st, upd, &updstr, cfg);
            },
            Err(err) => error!("read_update error: {}", err),
        }

        Ok(Response::with(iron::status::Ok))
    }

    let bot_handler = move |req: &mut Request| bot(req, &config);

    let listenpath: &str =
        &std::env::var("RVFISH_LISTENPATH").unwrap_or_else(|_| "/bot".to_owned());

    let reload_handler = |req: &mut Request| reload_places(req);
    let set_top_handler = |req: &mut Request| set_top(req);

    router.post(listenpath, bot_handler, "bot");
    router.get("/reload_places", reload_handler, "reload");
    router.post("/set_top", set_top_handler, "set_top");

    let botstate = BotState {
        places: Vec::new(),
        cache: HashMap::new(),
        top_ids: Vec::new(),
    };

    let mut chain = Chain::new(router);
    chain.link(State::<BotState>::both(botstate));
    chain.link_before(Read::<bodyparser::MaxBodyLength>::one(1024 * 1024));

    let listenaddr: &str =
        &std::env::var("RVFISH_LISTENADDR").unwrap_or_else(|_| "localhost:2358".to_owned());

    match Iron::new(chain).http(listenaddr) {
        Ok(_) => {}
        Err(e) => error!("iron http failure {}", e.to_string()),
    }
}
