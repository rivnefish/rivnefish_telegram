# rivnefish_telegram [![Build Status](https://travis-ci.org/rivnefish/rivnefish_telegram.svg?branch=master)](https://travis-ci.org/rivnefish/rivnefish_telegram) [![Download](https://api.bintray.com/packages/chyvonomys/rivnefish_telegram/rvfish_bot/images/download.svg) ](https://bintray.com/chyvonomys/rivnefish_telegram/rvfish_bot/_latestVersion)
Telegram bot for quick lookup of fishing places, inspired by @imdb

All binaries can be found [here](https://bintray.com/chyvonomys/rivnefish_telegram/rvfish_bot)

## How to build

0. Install some needed packages: `apt-get install build-essential pkg-config libssl-dev`

1. Install rust toolchain: `curl https://sh.rustup.rs -sSf | sh`

2. Build bot executable: `cargo build --release`. This will create an executable in `target/release/rvfish_bot`

## How to use

0. Register your bot with @BotFather: pick `<botname>`, obtain `<bottoken>`, enable inline mode with `/setinline` and optionally enable inline feedback by changing `/setinlinefeedback` to `Enabled` for your bot.

1. Set webhook:
  ```
  POST https://api.telegram.org/bot<bottoken>/setWebhook
  Content-Type: application/json

  {"url": "https://<host>/<webhookpath>"}
  ```
2. Configure it using environment variables:
  ```
  export RVFISH_LISTENADDR=localhost:<port>
  export RVFISH_LISTENPATH=/<webhookpath>
  export RVFISH_BOTNAME=@<botname>
  export RVFISH_BOTTOKEN=<bottoken>
  export RVFISH_CHANNEL=@<channel>
  export RVFISH_PUBLISHALBUMS=yes
  ```
3. Run the executable:
  ```
  ./rvfish_bot
  ```
4. Tell bot to update itself with fishing places:
  ```
  GET http://localhost:<port>/reload_places
  ```
5. Configure nginx to proxy_pass `/<webhookpath>` to `localhost:<port>/<webhookpath>`

## Extras

### Set up list of places to be shown upon empty inline query:
```
POST http://localhost:<port>/set_top
Content-Type: application/json

{"ids": [20, 21, 800]}
```
### Use `/announce` to post messages to chats via the bot:
```
POST http://localhost:<port>/announce
Content-Type: application/json

{
    "chat": <chatid>,
    "text": "*text* _with_ basic [Markdown support](https://core.telegram.org/bots/api#markdown-style)",
    "images": [<url1>, <url2>, ...]
}
```
   `images` list is optional, if present, bot will post an album of given photos.
   `<chatid>` could be either numeric id or `@<channelname>` for channels.
### Use `/publish` to publish fishing report in configured channel.
```
POST http://localhost:<port>/publish
Content-Type: application/json

{"id": <reportid>}
```
   This will post a nice card with report details and photos in channel configured by `RVFISH_CHANNEL` env variable.
   Intended usage: automatically notify channel subscribers when new report appears (webhook for site).
   If `RVFISH_PUBLISHALBUMS` is set to `yes` then it will also post album of photos (if any). By default this option is turned off.

These requests return:
- `200 OK` on success
- `400 Bad Request` if request is malformed (bad JSON, etc.)
- `500 Internal Server Error` if unable to fulfill request (bad chat id, bad report id, etc.)

## Logging
By default logging level is `INFO`, bot will write some meaningful information on any action taken both success and error.
Whenever request returns error, log is the first place to check for details.
