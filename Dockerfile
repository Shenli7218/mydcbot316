# 第一階段：建置階段
FROM rust:1.81 as builder
WORKDIR /usr/src/app

# 安裝 musl 工具鏈
RUN apt-get update && apt-get install -y musl-tools

# 先複製所有專案檔案
COPY . .

# 設定 musl 作為編譯目標
RUN rustup target add x86_64-unknown-linux-musl

# 生成 Cargo.lock 檔案，這會在初次編譯時自動生成，並且靜態鏈接
RUN cargo build --release --target x86_64-unknown-linux-musl

# 第二階段：運行階段
FROM scratch  # 使用空映像，因為所有庫都已靜態鏈接

# 從建置階段複製靜態鏈接編譯後的二進位檔（此處假設編譯後檔案名稱為 mydcbot316）
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/mydcbot316 /usr/local/bin/mydcbot316

# 設定工作目錄（注意 Render 的檔案系統為 ephemeral，資料庫檔案可能不持久）
WORKDIR /data

# 設定運行指令
CMD ["mydcbot316"]
