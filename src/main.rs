extern crate bodyparser;
extern crate iron;
extern crate persistent;
extern crate router;
extern crate time;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate reqwest;
#[macro_use]
extern crate lazy_static;

use iron::prelude::*;
use iron::typemap::Key;

use persistent::{Read, State};

use time::PreciseTime;

use std::sync::{Arc, RwLock};
use std::io::Write;
use std::collections::hash_map::{HashMap, Entry};

#[allow(dead_code)]
mod telegram;
use telegram::*;

mod fish;
use fish::{RfApi, RfPlace, RfPlaceInfo, RfFish};

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

fn make_report_kb(kbdata: &KbData) -> TgInlineKeyboardMarkup {
    TgInlineKeyboardMarkup { inline_keyboard: vec![vec![
        TgInlineKeyboardButton::Cb {
            text: format!("\u{1F44D} {}", kbdata.votes.len()),
            callback_data: "u".to_owned(),
        },
        TgInlineKeyboardButton::Url {
            text: "переглянути на вебсайті".to_owned(),
            url: kbdata.url.clone(),
        },
    ]]}
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
        },
        TgUpdate {
            message: None,
            callback_query: Some(TgCallbackQuery {
                id: cbq_id,
                from: user,
                message: Some(TgMessageLite{message_id, chat}),
                data: Some(d)
            }),
            inline_query: None,
            ..
        } => {
            info!("CBQ: id: {} from: {} msgid: {} chat: {:?} data: {}", cbq_id, user.id, message_id, chat.username, d);
            if let Some(username) = chat.username {
                if username == &cfg.channel[1..] && d == "u" { // NOTE: username is not prefixed with @
                    if let Ok(mut guard) = st.write() {
                        let bs = &mut *guard;
                        match bs.kbdata.entry(message_id) {
                            Entry::Occupied(mut e) => {
                                let kbdata = e.get_mut();
                                if let Some(i) = kbdata.votes.iter().position(|x| *x == user.id) {
                                    kbdata.votes.swap_remove(i);
                                } else {
                                    kbdata.votes.push(user.id);
                                }
                                tg.update_kb(
                                    message_id,
                                    make_report_kb(kbdata),
                                    TgChatId::Username(format!("@{}", username)), // NOTE: should be prefixed with @
                                );
                                tg.answer_cbq(cbq_id, Some("ваш голос враховано".to_owned()));
                            },
                            Entry::Vacant(_) => {
                                error!("no kbdata for this message_id");
                                tg.answer_cbq(cbq_id, None);
                            },
                        }
                    }
                } else {
                    info!("ignore unknown CBQ");
                }
            } else {
                info!("ignore this chat");
            }
        },
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
                        reply_markup: Some(TgInlineKeyboardMarkup::url_button(
                            "детальніше на вебсайті".to_owned(),
                            pi.url,
                        )),
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

type PostId = i32;
type UserId = i32;


#[derive(Serialize, Deserialize, Clone)]
struct KbData {
    url: String,
    votes: Vec<UserId>,
}

#[derive(Default)]
struct BotState {
    places: Vec<RfPlace>,
    fishes: Vec<RfFish>,
    cache: HashMap<i32, Option<RfPlaceInfo>>,
    top_ids: Vec<i32>,
    kbdata: HashMap<PostId, KbData>,
}

impl Key for BotState {
    type Value = BotState;
}

fn modify_bot_state<F>(req: &mut Request, f: F)
    where F: FnOnce(&mut BotState),
{
    if let Ok(arc_st) = req.get::<State<BotState>>() {
        if let Ok(mut guard) = arc_st.write() {
            let bs = &mut *guard;

            f(bs);
        }
    }
}

fn load_state(req: &mut Request) -> IronResult<Response> {
    let status = match req.get::<bodyparser::Struct<HashMap<PostId, KbData>>>() {
        Ok(Some(kbdata)) => {
            modify_bot_state(req, |bs: &mut BotState| {
                bs.kbdata = kbdata;
                info!("loaded state");
            });
            iron::status::Ok
        },
        Ok(None) => {
            info!("/load_state request has empty body");
            iron::status::BadRequest
        },
        Err(err) => {
            error!("/load_state: {:?}", err);
            iron::status::BadRequest
        },
    };

    Ok(Response::with(status))
}

fn save_state(req: &mut Request) -> IronResult<Response> {
    if let Ok(arc_st) = req.get::<State<BotState>>() {
        if let Ok(guard) = arc_st.read() {
            let bs = &guard;
            let resp = if let Ok(s) = serde_json::to_string(&bs.kbdata) {
                Response::with((
                    iron::status::Ok,
                    iron::modifiers::Header(iron::headers::ContentType::json()),
                    s,
                ))
            } else {
                Response::with(iron::status::InternalServerError)
            };
            return Ok(resp);
        }
    }
    
    Ok(Response::with(iron::status::InternalServerError))
}

fn reload_places(req: &mut Request) -> IronResult<Response> {
    let rfapi = fish::RfApi::new();
    let new_places = rfapi.fetch_all_places();
    let new_fish = rfapi.fetch_all_fish();

    modify_bot_state(req, |bs: &mut BotState| {
        bs.places = new_places;
        bs.fishes = new_fish;
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
    let status = match req.get::<bodyparser::Struct<TopIds>>() {
        Ok(Some(s)) => {
            modify_bot_state(req, |bs: &mut BotState| {
                bs.top_ids = s.ids.clone();
                info!("updated top fishing places with {} items", bs.top_ids.len());
            });
            iron::status::Ok
        },
        Ok(None) => {
            info!("/set_top request has empty body");
            iron::status::BadRequest
        },
        Err(err) => {
            error!("/set_top: {:?}", err);
            iron::status::BadRequest
        },
    };

    Ok(Response::with(status))
}

#[derive(Deserialize, Clone)]
struct Announcement {
    chat: TgChatId,
    text: String,
    images: Option<Vec<String>>,
}

fn announce(req: &mut Request, cfg: &Config) -> IronResult<Response> {
    let status = match req.get::<bodyparser::Struct<Announcement>>() {
        Ok(Some(s)) => {
            let tg = TgBotApi::new(&cfg.bottoken);
            match tg.send_md_text(s.text, s.chat.clone(), None) {
                Err(err) => {
                    error!("/announce: {:?}", err);
                    iron::status::InternalServerError
                },
                Ok(TgResponse {ok: false, description, ..}) => {
                    error!("/announce: Bot API error: {:?}", description);
                    iron::status::InternalServerError
                },
                Ok(_) => if let Some(is) = s.images {
                    if !is.is_empty() {
                        match tg.send_album(is.iter(), s.chat) {
                            Err(err) => {
                                error!("/announce: {:?}", err);
                                iron::status::InternalServerError
                            },
                            Ok(TgResponse {ok: false, description, ..}) => {
                                error!("/announce: Bot API error: {:?}", description);
                                iron::status::InternalServerError
                            },
                            Ok(_) => {
                                info!("/announce: message posted");
                                iron::status::Ok
                            },
                        }
                    } else {
                        iron::status::Ok
                    }
                } else {
                    iron::status::Ok
                },
            }
        },
        Ok(None) => {
            info!("/announce: request has empty body");
            iron::status::BadRequest
        },
        Err(err) => {
            error!("/announce: {:?} while parsing request body", err);
            iron::status::BadRequest
        },
    };

    Ok(Response::with(status))
}

#[derive(Deserialize, Clone)]
struct PublishReport {
    id: i32,
}

fn publish(req: &mut Request, cfg: &Config) -> IronResult<Response> {
    let status = match req.get::<bodyparser::Struct<PublishReport>>() {
        Ok(Some(p)) => {
            let fish = RfApi::new();
            if let Some(ri) = fish.fetch_report_info(p.id) {
                if let Ok(arc_st) = req.get::<State<BotState>>() {
                    let pi = ri.place.as_ref().and_then(|p| get_info_for(&arc_st, &fish, p.id));
                    let tg = TgBotApi::new(&cfg.bottoken);
                    let chat = TgChatId::Username(cfg.channel.clone());
                    let kbdata = KbData {
                        url: ri.url.clone(),
                        votes: Vec::new(),
                    };
                    let resp = if let Ok(g) = arc_st.read() {
                        tg.send_rich_text(
                            fish::get_report_text(&ri, pi.as_ref(), &g.fishes),
                            chat.clone(),
                            Some(make_report_kb(&kbdata)),
                        )
                    } else {
                        Err(String::default())
                    };
                    match resp {
                        Err(err) => {
                            error!("/publish #{}: {:?}", ri.id, err);
                            iron::status::InternalServerError
                        },
                        Ok(TgResponse {ok: false, description, ..}) => {
                            error!("/publish #{}: Bot API error: {:?}", ri.id, description);
                            iron::status::InternalServerError
                        },
                        Ok(TgResponse {ok: true, result: None, ..}) => {
                            error!("/publish #{}: Bot API did not return msg", ri.id);
                            iron::status::InternalServerError
                        },
                        Ok(TgResponse {ok: true, result: Some(TgMessageLite {message_id, ..}), ..}) => {
                            if let Ok(mut guard) = arc_st.write() {
                                let bs = &mut *guard;
                                bs.kbdata.entry(message_id).or_insert(kbdata);
                            }
                            if ri.photos.len() < 2 || !cfg.publishalbums {
                                info!("/publish #{}: message (no album) posted", ri.id);
                                iron::status::Ok
                            } else {
                                match tg.send_album(ri.photos.iter().map(|p| &p.medium_url), chat) {
                                    Err(err) => {
                                        error!("/publish #{}: {:?}", ri.id, err);
                                        iron::status::InternalServerError
                                    },
                                    Ok(TgResponse {ok: false, description, ..}) => {
                                        error!("/publish #{}: Bot API error: {:?}", ri.id, description);
                                        iron::status::InternalServerError
                                    },
                                    Ok(_) => {
                                        info!("/publish #{}: message and album posted", ri.id);
                                        iron::status::Ok
                                    },
                                }
                            }
                        }
                    }
                } else {
                    iron::status::InternalServerError // arc
                }
            } else {
                iron::status::InternalServerError // fetch report
            }
        },
        Ok(None) => {
            info!("/publish: request has empty body");
            iron::status::BadRequest
        },
        Err(err) => {
            error!("/publish: {:?} while parsing request body", err);
            iron::status::BadRequest
        },
    };

    Ok(Response::with(status))
}

struct Config {
    //botname: String,
    bottoken: String,
    channel: String,
    listenpath: String,
    listenaddr: String,
    publishalbums: bool,
}

lazy_static! {
    static ref CONFIG: Config = Config {
        //botname: std::env::var("RVFISH_BOTNAME").unwrap_or_default(),
        bottoken: std::env::var("RVFISH_BOTTOKEN").unwrap_or_default(),
        channel: std::env::var("RVFISH_CHANNEL").unwrap_or_default(),
        listenpath: std::env::var("RVFISH_LISTENPATH").unwrap_or_else(|_| "/bot".to_owned()),
        listenaddr: std::env::var("RVFISH_LISTENADDR").unwrap_or_else(|_| "localhost:2358".to_owned()),
        publishalbums: std::env::var("RVFISH_PUBLISHALBUMS").map(|s| s == "yes").unwrap_or_default(),
    };
}

fn main() {
    let mut log_builder = env_logger::Builder::new();
    log_builder.format(|buf, record| {
        writeln!(
            buf,
            "{} {} [{}] {}",
            time::strftime("%Y-%m-%d %H:%M:%S", &time::now()).unwrap(),
            record.level(),
            record.module_path().unwrap_or("?"),
            record.args()
        )
    }).filter(None, log::LevelFilter::Info);

    if let Ok(ref lcfg) = std::env::var("RUST_LOG") {
        log_builder.parse(lcfg);
    }

    log_builder.init();

    fn bot(req: &mut Request, cfg: &Config) -> IronResult<Response> {
        match telegram::read_update(&mut req.body) {
            Ok((upd, updstr)) => if let Ok(arc_st) = req.get::<State<BotState>>() {
                process_update(&arc_st, upd, &updstr, cfg);
            },
            Err(err) => error!("read_update error: {}", err),
        }

        Ok(Response::with(iron::status::Ok))
    }

    let bot_handler = |req: &mut Request| bot(req, &CONFIG);
    let announce_handler = |req: &mut Request| announce(req, &CONFIG);
    let publish_handler = |req: &mut Request| publish(req, &CONFIG);

    let mut router = router::Router::new();
    router.post(&CONFIG.listenpath, bot_handler, "bot");
    router.get("/reload_places", reload_places, "reload");
    router.post("/set_top", set_top, "set_top");
    router.post("/announce", announce_handler, "announce");
    router.post("/publish", publish_handler, "publish");
    router.post("/load_state", load_state, "load_state");
    router.get("/save_state", save_state, "save_state");

    let botstate = BotState::default();

    let mut chain = Chain::new(router);
    chain.link(State::<BotState>::both(botstate));
    chain.link_before(Read::<bodyparser::MaxBodyLength>::one(1024 * 1024));

    match Iron::new(chain).http(&CONFIG.listenaddr) {
        Ok(_) => {}
        Err(e) => error!("iron http failure {}", e.to_string()),
    }
}
