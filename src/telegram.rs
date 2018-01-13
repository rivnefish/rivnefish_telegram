use reqwest;
use serde::ser::Serialize;
use serde::de::DeserializeOwned;
use serde_json;
use std::io::Read;
use std::fmt::Write;

#[derive(Deserialize, Debug)]
pub struct TgChat {
    pub id: i64,
    #[serde(rename = "type")] type_: String,
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
    #[serde(rename = "type")] type_: String,
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum TgChatId {
    Integer(i64),
    Username(String),
}

#[derive(Serialize)]
pub struct TgSendMsg {
    chat_id: TgChatId,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")] parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] reply_to_message_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] reply_markup: Option<TgInlineKeyboardMarkup>,
}


#[derive(Serialize)]
struct TgInputMediaPhoto {
    #[serde(rename = "type")] type_: String,
    media: String,
}

impl TgInputMediaPhoto {
    fn new(url: &str) -> Self {
        Self {
            type_: "photo".to_owned(),
            media: String::from(url),
        }
    }
}

#[derive(Serialize)]
struct TgSendMediaGroup {
    chat_id: TgChatId,
    media: Vec<TgInputMediaPhoto>,
}

#[derive(Serialize)]
pub struct TgEditMsgReplyMarkup {
    chat_id: TgChatId,
    message_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")] reply_markup: Option<TgInlineKeyboardMarkup>,
}

#[derive(Serialize)]
pub struct TgAnswerCBQ {
    callback_query_id: String,
}

#[derive(Serialize, Debug)]
pub struct TgInlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<TgInlineKeyboardButton>>,
}

impl TgInlineKeyboardMarkup {
    pub fn new() -> Self {
        Self {
            inline_keyboard: vec![vec![]],
        }
    }
    pub fn url_button(text: String, url: String) -> Self {
        Self {
            inline_keyboard: vec![vec![TgInlineKeyboardButton::Url{text, url}]],
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum TgInlineKeyboardButton {
    Cb { text: String, callback_data: String },
    Url { text: String, url: String },
}

#[derive(Serialize)]
pub struct TgAnswerInlineQuery {
    pub inline_query_id: String,
    pub results: Vec<TgInlineQueryResult>,
}

#[derive(Serialize, Debug)]
pub struct TgInlineQueryResult {
    #[serde(rename = "type")] pub type_: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub hide_url: bool,
    pub thumb_url: String,
    pub reply_markup: Option<TgInlineKeyboardMarkup>,
    pub input_message_content: TgInputMessageContent,
}

#[derive(Serialize, Debug)]
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

const BASEURL: &str = "https://api.telegram.org";
const MAX_ALBUM_SIZE: usize = 10;

pub struct TgBotApi<'a> {
    api_token: &'a str,
    http_client: reqwest::Client,
}

impl<'a> TgBotApi<'a> {
    pub fn new(token: &str) -> TgBotApi {
        TgBotApi {
            api_token: token,
            http_client: reqwest::Client::new(),
        }
    }

    pub fn send_json<S: Serialize>(&self, method: &str, obj: S) {
        let mut url = String::new();
        url.push_str(BASEURL);
        url.push_str("/bot");
        url.push_str(self.api_token);
        url.push_str(method);
        if let Ok(bod) = serde_json::to_string(&obj) {
            let mut hs = reqwest::header::Headers::new();
            hs.set(reqwest::header::ContentType::json());
            if let Err(e) = self.http_client
                .post(&url)
                .headers(hs)
                .body(bod)
                .send()
            {
                error!("error sending json: {}", e);
            }
        }
    }

    pub fn send_json_recv_json<S, D>(&self, method: &str, obj: S) -> Result<D, String>
    where S: Serialize, D: DeserializeOwned
    {
        match serde_json::to_string(&obj) {
            Ok(bod) => {
                let mut url = String::new();
                url.push_str(BASEURL);
                url.push_str("/bot");
                url.push_str(self.api_token);
                url.push_str(method);

                let mut hs = reqwest::header::Headers::new();
                hs.set(reqwest::header::ContentType::json());
                match self.http_client
                    .post(&url)
                    .headers(hs)
                    .body(bod)
                    .send()
                {
                    Ok(resp) => match serde_json::from_reader(resp) {
                        Ok(d) => Ok(d),
                        Err(e) => Err(e.to_string()),
                    },
                    Err(e) => Err(e.to_string()),
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn answer_cbq(&self, id: String) {
        self.send_json(
            "/answerCallbackQuery",
            TgAnswerCBQ {
                callback_query_id: id,
            },
        );
    }

    pub fn send_text(&self, text: String, chatid: TgChatId) {
        self.send_json(
            "/sendMessage",
            TgSendMsg {
                chat_id: chatid,
                text: text,
                parse_mode: None,
                reply_to_message_id: None,
                reply_markup: None,
            },
        );
    }

    pub fn send_rich_text(&self, text: String, chatid: TgChatId, kb: Option<TgInlineKeyboardMarkup>)
    -> Result<TgResponse<TgMessageLite>, String> {
        self.send_json_recv_json(
            "/sendMessage",
            TgSendMsg {
                chat_id: chatid,
                text: text,
                parse_mode: Some("HTML".to_owned()),
                reply_to_message_id: None,
                reply_markup: kb,
            },
        )
    }

    pub fn send_md_text(&self, text: String, chatid: TgChatId, kb: Option<TgInlineKeyboardMarkup>)
    -> Result<TgResponse<TgMessageLite>, String> {
        self.send_json_recv_json(
            "/sendMessage",
            TgSendMsg {
                chat_id: chatid,
                text: text,
                parse_mode: Some("Markdown".to_owned()),
                reply_to_message_id: None,
                reply_markup: kb,
            },
        )
    }

    pub fn send_album<'u, I: Iterator<Item=&'u String>>(&self, urls: I, chatid: TgChatId)
    -> Result<TgResponse<Vec<TgMessageLite>>, String> {
        self.send_json_recv_json(
            "/sendMediaGroup",
            TgSendMediaGroup {
                chat_id: chatid,
                media: urls.map(|url| TgInputMediaPhoto::new(url)).take(MAX_ALBUM_SIZE).collect(),
            }
        )
    }

    pub fn send_reply(&self, text: String, mid: u64, chatid: TgChatId) {
        self.send_json(
            "/sendMessage",
            TgSendMsg {
                chat_id: chatid,
                text: text,
                parse_mode: None,
                reply_to_message_id: Some(mid),
                reply_markup: None,
            },
        )
    }

    pub fn send_kb(&self, text: String, kb: TgInlineKeyboardMarkup, chatid: TgChatId)
    -> Result<TgResponse<TgMessageLite>, String> {
        self.send_json_recv_json(
            "/sendMessage",
            TgSendMsg {
                chat_id: chatid,
                text: text,
                parse_mode: None,
                reply_to_message_id: None,
                reply_markup: Some(kb),
            },
        )
    }

    pub fn update_kb(&self, msgid: u64, kb: TgInlineKeyboardMarkup, chatid: TgChatId) {
        self.send_json(
            "/editMessageReplyMarkup",
            TgEditMsgReplyMarkup {
                chat_id: chatid,
                message_id: msgid,
                reply_markup: Some(kb),
            },
        );
    }
}

pub fn get_whoami(user: &TgUser, chat: &TgChat) -> String {
    let mut acc = String::new();
    write!(
        &mut acc,
        "I see you as: {:#?}, and we are currently here: {:#?}",
        user,
        chat
    ).unwrap();
    acc
}

pub fn make_name(user: &TgUser) -> String {
    let mut res = String::new();
    match *user {
        TgUser {
            username: Some(ref u),
            ..
        } => write!(&mut res, "@{}", u).unwrap(),

        TgUser {
            first_name: ref f,
            last_name: Some(ref l),
            username: None,
            ..
        } => write!(&mut res, "{} {}", f, l.chars().next().unwrap_or('?')).unwrap(),

        TgUser {
            first_name: ref f,
            username: None,
            ..
        } => res.push_str(f),
    }
    res
}

pub fn read_update(src: &mut Read) -> Result<(TgUpdate, String), String> {
    let mut body = String::new();
    src.read_to_string(&mut body).unwrap();

    match serde_json::from_str::<TgUpdate>(&body) {
        Ok(upd) => Ok((upd, body)),
        Err(err) => {
            let mut errstr = String::new();
            write!(
                &mut errstr,
                "received: {}\nparsing error: {}",
                &body,
                &err.to_string()
            ).unwrap();
            Err(errstr)
        }
    }
}
