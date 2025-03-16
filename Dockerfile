# 第一階段：建置階段
FROM rust:1.81 as builder

# 設定工作目錄
WORKDIR /usr/src/app

# 安裝依賴（比如 pkg-config，這有時候需要用來編譯依賴）
RUN apt-get update && apt-get install -y libssl-dev pkg-config libsqlite3-dev

# 複製所有專案檔案
COPY . .

# 更新 Rust 工具鏈
RUN rustup update

# 生成 Cargo.lock 檔案，這會在初次編譯時自動生成
RUN cargo build --release

# 第二階段：運行階段
FROM debian:bookworm-slim

# 安裝所需的依賴庫：SQLite 和 OpenSSL
RUN apt-get update && \
    apt-get install -y libsqlite3-0 libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# 創建資料夾 /t 並設置權限，這對資料庫至關重要
RUN mkdir -p /t && chmod 777 /t

# 創建空的資料庫檔案（如果資料庫檔案不存在）
RUN touch /t/bot.db && chmod 777 /t/bot.db

# 從建置階段複製編譯好的二進位檔
COPY --from=builder /usr/src/app/target/release/mydcbot316 /usr/local/bin/mydcbot316

# 設定工作目錄為 /t，這樣資料庫將儲存在此目錄中
WORKDIR /t

# 開放必要的端口（如果 Render 要求開放端口）
EXPOSE 8080

# 初始化資料庫結構（如果資料庫文件存在，會跳過創建）
CMD ["sh", "-c", "sqlite3 /t/bot.db 'CREATE TABLE IF NOT EXISTS guild_configs (guild_id INTEGER PRIMARY KEY, registration_channel INTEGER, manual_channel INTEGER, admin_channel INTEGER, admin_role INTEGER, advanced_role INTEGER); CREATE TABLE IF NOT EXISTS registrations (guild_id INTEGER, name TEXT, age TEXT, PRIMARY KEY(guild_id, name)); mydcbot316"]
