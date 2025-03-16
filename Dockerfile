# 第一階段：建置階段
FROM rust:1.81 as builder
WORKDIR /usr/src/app

# 先複製所有專案檔案
COPY . .

# 生成 Cargo.lock 檔案，這會在初次編譯時自動生成
RUN cargo build --release

# 第二階段：運行階段
# 使用一個簡單的基礎映像來代替 'FROM scratch'
FROM busybox:1.35.0-uclibc

# 從建置階段複製編譯好的二進位檔（此處假設編譯後檔案名稱為 mydcbot316）
COPY --from=builder /usr/src/app/target/release/mydcbot316 /usr/local/bin/mydcbot316

# 設定工作目錄
WORKDIR /data

# 設定運行指令
CMD ["mydcbot316"]
