# 第一階段：建置階段
FROM rust:1.70 as builder
WORKDIR /usr/src/app
# 先複製 Cargo 設定檔方便快取相依性
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
# 預先編譯依賴
RUN cargo build --release
# 複製完整專案並重新編譯
COPY . .
RUN cargo build --release

# 第二階段：運行階段
FROM debian:buster-slim
# 安裝必須的動態連結庫（例如 SQLite）
RUN apt-get update && apt-get install -y libsqlite3-0 ca-certificates && rm -rf /var/lib/apt/lists/*
# 從建置階段複製編譯好的二進位檔（此處假設編譯後檔案名稱為 discord_bot）
COPY --from=builder /usr/src/app/target/release/discord_bot /usr/local/bin/discord_bot

# 指定工作目錄（注意 Render 的檔案系統為 ephemeral，資料庫檔案可能不持久）
WORKDIR /data

# 設定運行指令
CMD ["mydcbot316"]
