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
  ```
3. Run the executable:
  ```
  ./rvfish_bot
  ```
4. Tell bot to update itself with fishing places:
  ```
  GET http://localhost:<port>/reload_places
  ```
5. Set up list of places to be shown upon empty inline query:
```
POST http://localhost:<port>/set_top
Content-Type: application/json

{"ids": [20, 21, 800]}
```
6. Configure nginx to proxy_pass `/<webhookpath>` to `localhost:<port>/<webhookpath>`
7. Use `/announce` to post messages to chats via the bot:
```
POST http://localhost:<port>/announce
Content-Type: application/json

{
    "chat": <chatid>,
    "text": "*text* _with_ basic [Markdown support](https://core.telegram.org/bots/api#markdown-style)"
}
```
```
