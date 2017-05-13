extern crate serde;
extern crate serde_json;

extern crate hyper;
extern crate hyper_native_tls;

use std::io::Read;
use std::fmt::Write;

#[derive(Deserialize, Debug)]
pub struct TgChat {
    pub id: i64,
    #[serde(rename = "type")]
    type_: String,
}

impl TgChat {
    pub fn is_private(&self) -> bool {
        self.type_ == "private"
    }
}

#[derive(Deserialize, Debug)]
pub struct TgResponse<R> {
    pub ok: bool,
    pub result: Option<R>,
    pub error_code: Option<i32>,
    pub description: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TgUser {
    pub id: i64,
    first_name: String,
    last_name: Option<String>,
    username: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TgMessageEntity {
    #[serde(rename = "type")]
    type_: String,
    pub offset: usize,
    pub length: usize,
}

impl TgMessageEntity {
    pub fn is_command(&self) -> bool {
        self.type_ == "bot_command"
    }
}

#[derive(Deserialize, Debug)]
pub struct TgMessage {
    message_id: u64,
    pub from: Option<TgUser>,
    date: u64,
    pub chat: TgChat,
    pub text: Option<String>,
    pub entities: Option<Vec<TgMessageEntity>>,
}

#[derive(Deserialize, Debug)]
pub struct TgMessageLite {
    pub message_id: u64,
    pub chat: TgChat,
}

#[derive(Deserialize, Debug)]
pub struct TgUpdate {
    update_id: u64,
    pub message: Option<TgMessage>,
    pub callback_query: Option<TgCallbackQuery>,
    pub inline_query: Option<TgInlineQuery>,
    pub chosen_inline_result: Option<TgChosenInlineResult>,
}

#[derive(Deserialize, Debug)]
pub struct TgInlineQuery {
    pub id: String,
    pub from: TgUser,
    pub query: String,
    pub offset: String,
}

#[derive(Deserialize, Debug)]
pub struct TgCallbackQuery {
    pub id: String,
    pub from: TgUser,
    pub message: Option<TgMessageLite>,
    pub data: Option<String>,
}

// Outgoing structs
#[derive(Serialize)]
pub struct TgSendMsg {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if="Option::is_none")]
    reply_to_message_id: Option<u64>,
    #[serde(skip_serializing_if="Option::is_none")]
    reply_markup: Option<TgInlineKeyboardMarkup>,
}

#[derive(Serialize)]
pub struct TgEditMsgReplyMarkup {
    chat_id: i64,
    message_id: u64,
    #[serde(skip_serializing_if="Option::is_none")]
    reply_markup: Option<TgInlineKeyboardMarkup>,
}

#[derive(Serialize)]
pub struct TgAnswerCBQ {
    callback_query_id: String,
}

#[derive(Serialize)]
pub struct TgInlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<TgInlineKeyboardButton>>,
}

impl TgInlineKeyboardMarkup {
    pub fn new() -> TgInlineKeyboardMarkup {
        TgInlineKeyboardMarkup{inline_keyboard: vec![vec![]]}
    }
}

#[derive(Serialize)]
pub struct TgInlineKeyboardButton {
    pub text: String,
    pub url: Option<String>,
    pub callback_data: Option<String>,
}

#[derive(Serialize)]
pub struct TgAnswerInlineQuery {
    pub inline_query_id: String,
    pub results: Vec<TgInlineQueryResult>,
}

#[derive(Serialize)]
pub struct TgInlineQueryResult {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub hide_url: bool,
    pub thumb_url: String,
    pub reply_markup: Option<TgInlineKeyboardMarkup>,
    pub input_message_content: TgInputMessageContent,
}

#[derive(Serialize)]
pub struct TgInputMessageContent {
    pub message_text: String,
    pub parse_mode: String,
    pub disable_web_page_preview: bool,
}

// NOTE: this works only if bot has inline feedback enabled
#[derive(Deserialize, Debug)]
pub struct TgChosenInlineResult {
    pub result_id: String,
    pub from: TgUser,
    pub inline_message_id: Option<String>,
    pub query: String,
}

const BASEURL: &'static str = "https://api.telegram.org";

pub struct TgBotApi<'a> {
    api_token: &'a str,
    http_client: hyper::Client,
}

impl<'a> TgBotApi<'a> {

pub fn new(token: &str) -> TgBotApi {
    let ssl = hyper_native_tls::NativeTlsClient::new().unwrap();
    let connector = hyper::net::HttpsConnector::new(ssl);
    let client = hyper::Client::with_connector(connector);
    TgBotApi {
        api_token: token,
        http_client: client,
    }
}

pub fn send_json<S: serde::ser::Serialize>(&self, method: &str, obj: S) {
    let mut url = String::new();
    url.push_str(BASEURL);
    url.push_str("/bot");
    url.push_str(self.api_token);
    url.push_str(method);
    if let Ok(bod) = serde_json::to_string(&obj) {
        let mut hs = hyper::header::Headers::new();
        hs.set(hyper::header::ContentType::json());
        self.http_client.post(&url).headers(hs).body(&bod).send().unwrap();
    }
}

pub fn send_json_recv_json<S, D>(&self, method: &str, obj: S) -> Result<D, String>
    where S: serde::ser::Serialize,
          D: serde::de::Deserialize
{
    match serde_json::to_string(&obj) {
        Ok(bod) => {
            let mut url = String::new();
            url.push_str(BASEURL);
            url.push_str("/bot");
            url.push_str(self.api_token);
            url.push_str(method);

            let mut hs = hyper::header::Headers::new();
            hs.set(hyper::header::ContentType::json());
            match self.http_client.post(&url).headers(hs).body(&bod).send() {
                Ok(resp) => {
                    match serde_json::from_reader(resp) {
                        Ok(d) => Ok(d),
                        Err(e) => Err(e.to_string()),
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn answer_cbq(&self, id: String) {
    self.send_json("/answerCallbackQuery",
                 TgAnswerCBQ { callback_query_id: id });
}

pub fn send_text(&self, text: String, chatid: i64) {
    self.send_json("/sendMessage",
        TgSendMsg {
            chat_id: chatid,
            text: text,
            reply_to_message_id: None,
            reply_markup: None,
        });
}

pub fn send_reply(&self, text: String, mid: u64, chatid: i64) {
    self.send_json("/sendMessage",
        TgSendMsg {
            chat_id: chatid,
            text: text,
            reply_to_message_id: Some(mid),
            reply_markup: None,
        })
}

pub fn send_kb(&self,
               text: String,
               kb: TgInlineKeyboardMarkup,
               chatid: i64) -> Result<TgResponse<TgMessageLite>, String> {
    self.send_json_recv_json("/sendMessage",
        TgSendMsg {
            chat_id: chatid,
            text: text,
            reply_to_message_id: None,
            reply_markup: Some(kb),
        })
}

pub fn update_kb(&self, msgid: u64, chatid: i64, kb: TgInlineKeyboardMarkup) {
    self.send_json("/editMessageReplyMarkup",
        TgEditMsgReplyMarkup {
            chat_id: chatid,
            message_id: msgid,
            reply_markup: Some(kb),
        });
}
}

pub fn get_whoami(user: &TgUser, chat: &TgChat) -> String {
    let mut acc = String::new();
    write!(&mut acc,
           "I see you as: {:#?}, and we are currently here: {:#?}",
           user,
           chat)
        .unwrap();
    acc
}

pub fn make_name(user: &TgUser) -> String {
    let mut res = String::new();
    match *user {
        TgUser { username: Some(ref u), .. } => {
            write!(&mut res, "@{}", u).unwrap()
        }

        TgUser { first_name: ref f, last_name: Some(ref l), username: None, .. } => {
            write!(&mut res, "{} {}", f, l.chars().next().unwrap_or('?')).unwrap()
        }

        TgUser { first_name: ref f, username: None, .. } => res.push_str(f),
    }
    res
}

pub fn read_update(src: &mut Read) -> Result<(TgUpdate, String), String> {
    let mut body = String::new();
    src.read_to_string(&mut body).unwrap();

    match serde_json::from_str::<TgUpdate>(&body) {
        Ok(upd) => {
            Ok((upd, body))
        }
        Err(err) => {
            let mut errstr = String::new();
            write!(&mut errstr, "received: {}\nparsing error: {}", &body, &err.to_string()).unwrap();
            Err(errstr)
        }
    }
}
