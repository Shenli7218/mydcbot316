# 第一階段：建置階段
FROM rust:1.81 as builder

# 設定工作目錄
WORKDIR /usr/src/app

# 複製所有專案檔案
COPY . .

# 設定環境變數來優化編譯
ENV CARGO_REGISTRY_INDEX="https://github.com/rust-lang/crates.io-index"
ENV CARGO_HOME=/usr/src/cargo

# 生成 Cargo.lock 檔案，這會在初次編譯時自動生成
RUN cargo build --release

# 第二階段：運行階段
FROM debian:bullseye-slim

# 安裝所需的依賴庫：SQLite 和 OpenSSL
RUN apt-get update && \
    apt-get install -y libsqlite3-0 libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# 從建置階段複製編譯好的二進位檔
COPY --from=builder /usr/src/app/target/release/mydcbot316 /usr/local/bin/mydcbot316

# 設定工作目錄
WORKDIR /data

# 開放必要的端口（如果 Render 要求開放端口）
EXPOSE 8080

# 設定運行指令
CMD ["mydcbot316"]
