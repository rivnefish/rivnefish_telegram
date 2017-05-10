# rivnefish_telegram
Telegram bot for quick lookup of fishing places, inspired by @imdb

## How to build

0. Install some needed packages: `apt-get install build-essential pkg-config libssl-dev`

1. Install rust toolchain: `curl https://sh.rustup.rs -sSf | sh`

2. Build bot executable: `cargo build --release`. This will create an executable in `target/release/rvfish_bot`

## How to use

0. Register your bot with @BotFather: pick `<botname>`, obtain `<bottoken>` and enable inline mode for your bot.

1. Set webhook:
  ```
  POST https://api.telegram.org/bot:<bottoken>/setWebhook
  Content-Type: application/json

  {"url": "https://<host>/<webhookpath>"}
  ```
2. Configure it using environment variables:
  ```
  RVFISH_LISTENADDR=localhost:<port>
  RVFISH_LISTENPATH=/<webhookpath>
  RVFISH_BOTNAME=@<botname>
  RVFISH_BOTTOKEN=<bottoken>

  export RVFISH_LISTENADDR RVFISH_LISTENPATH RVFISH_BOTNAME RVFISH_BOTTOKEN
  ```
3. Run the executable:
  ```
  RUST_BACKTRACE=1 nohup ./rvfish_bot >>output.log 2>&1 &
  ```
4. Tell bot to update itself with fishing places:
  ```
  GET http://localhost:<port>/reload_places
  ```
5. Configure nginx to proxy_pass `/<webhookpath>` to `localhost:<port>/<webhookpath>`
