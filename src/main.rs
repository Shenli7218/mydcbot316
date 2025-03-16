use serenity::async_trait;
use serenity::model::prelude::*;
use serenity::prelude::*;
use sqlx::{SqlitePool, Row};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// 輔助結構：QueuedMessage
#[derive(Debug, Clone)]
struct QueuedMessage {
    guild_id: u64,
    channel_id: ChannelId,
    content: String,
    author_id: u64,
}

impl QueuedMessage {
    /// 使用頻道回覆訊息
    async fn reply(&self, ctx: &Context, content: &str) -> serenity::Result<Message> {
        self.channel_id.say(&ctx.http, content).await
    }
}

impl From<Message> for QueuedMessage {
    fn from(msg: Message) -> Self {
        Self {
            guild_id: msg.guild_id.unwrap().0,
            channel_id: msg.channel_id,
            content: msg.content,
            author_id: msg.author.id.0,
        }
    }
}

/// 資料庫內的 Guild 設定
struct GuildConfig {
    registration_channel: ChannelId,
    manual_channel: ChannelId,
    admin_channel: ChannelId,
    admin_role: RoleId,
    advanced_role: RoleId,
}

impl GuildConfig {
    /// 從資料庫讀取指定 guild 的配置
    async fn get(pool: &SqlitePool, guild_id: u64) -> Option<Self> {
        let row = sqlx::query(
            "SELECT registration_channel, manual_channel, admin_channel, admin_role, advanced_role 
             FROM guild_configs WHERE guild_id = ?"
        )
        .bind(guild_id as i64)
        .fetch_one(pool)
        .await
        .ok()?;

        Some(Self {
            registration_channel: ChannelId(row.get::<i64, _>(0) as u64),
            manual_channel: ChannelId(row.get::<i64, _>(1) as u64),
            admin_channel: ChannelId(row.get::<i64, _>(2) as u64),
            admin_role: RoleId(row.get::<i64, _>(3) as u64),
            advanced_role: RoleId(row.get::<i64, _>(4) as u64),
        })
    }
}

/// 使用 mpsc 來管理消息佇列
struct QueueHolder {
    tx: mpsc::Sender<QueuedMessage>,
    rx: Mutex<mpsc::Receiver<QueuedMessage>>,
}

/// Bot 的事件處理器
struct Handler {
    pool: SqlitePool,
    queue: Arc<QueueHolder>,
}

impl Handler {
    /// 處理佇列內所有待處理訊息（利用當前取得的 Context）
    async fn process_queue(&self, ctx: &Context) {
        // 鎖定接收端並排乾所有訊息
        let mut rx = self.queue.rx.lock().await;
        while let Some(qmsg) = rx.try_recv().ok() {
            process_message_worker(ctx, &self.pool, qmsg).await;
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // 忽略機器人訊息與非 guild 訊息
        if msg.author.bot || msg.guild_id.is_none() {
            return;
        }
        let guild_id = msg.guild_id.unwrap().0;

        // 處理設定指令（僅管理員使用）
        if msg.content.starts_with("!setconfig") {
            if let Err(e) = handle_setconfig(&self.pool, &ctx, &msg, guild_id).await {
                println!("設定錯誤: {:?}", e);
            }
            return;
        }

        // 將訊息轉換成 QueuedMessage，並發送到 mpsc 佇列
        let qmsg: QueuedMessage = msg.into();
        if let Err(e) = self.queue.tx.send(qmsg).await {
            println!("推入佇列失敗: {:?}", e);
        }

        // 立刻嘗試處理佇列內所有訊息
        self.process_queue(&ctx).await;
    }
}

/// 解析註冊表單（預期格式 "Name: xxx, Age: xxx"）
async fn parse_form(content: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = content.split(',').collect();
    if parts.len() == 2 {
        let name = parts[0].trim_start_matches("Name:").trim().to_string();
        let age = parts[1].trim_start_matches("Age:").trim().to_string();
        Some((name, age))
    } else {
        None
    }
}

/// 解析人工審核表單（預期格式 "Manual: <申請內容>"）
fn parse_manual_form(content: &str) -> Option<String> {
    if content.starts_with("Manual:") {
        let data = content.trim_start_matches("Manual:").trim();
        if !data.is_empty() {
            Some(data.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

/// 寫入註冊資料到資料庫
async fn save_registration(
    pool: &SqlitePool,
    guild_id: u64,
    form_data: (String, String),
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO registrations (guild_id, name, age) VALUES (?, ?, ?)"
    )
    .bind(guild_id as i64)
    .bind(form_data.0)
    .bind(form_data.1)
    .execute(pool)
    .await?; 
    Ok(())
}

/// 為 QueuedMessage 分配進階身份組（與原有邏輯相同）
async fn assign_role_for_queued(
    ctx: &Context,
    qmsg: &QueuedMessage,
    advanced_role: RoleId,
) -> Result<(), serenity::Error> {
    let guild = GuildId(qmsg.guild_id);
    let mut member = guild.member(ctx, qmsg.author_id).await?; // 必須為 mutable
    member.add_role(ctx, advanced_role).await?; 
    Ok(())
}

/// 處理從佇列中取出的單筆訊息
async fn process_message_worker(ctx: &Context, pool: &SqlitePool, qmsg: QueuedMessage) {
    if let Some(config) = GuildConfig::get(pool, qmsg.guild_id).await {
        if qmsg.channel_id == config.registration_channel {
            if let Some(form_data) = parse_form(&qmsg.content).await {
                if let Err(e) = save_registration(pool, qmsg.guild_id, form_data).await {
                    println!("儲存資料失敗: {:?}", e);
                    let _ = qmsg.reply(ctx, "儲存資料時發生錯誤。").await;
                    return;
                }
                if let Err(why) = assign_role_for_queued(ctx, &qmsg, config.advanced_role).await {
                    println!("分配身份組失敗: {:?}", why);
                } else {
                    let _ = qmsg.reply(ctx, "已完成申請，分配進階身份組。").await;
                }
            }
        } else if qmsg.channel_id == config.manual_channel {
            if let Some(manual_data) = parse_manual_form(&qmsg.content) {
                let alert = format!("{} 有新的人工審核申請：{}", config.admin_role.mention(), manual_data);
                if let Err(why) = config.admin_channel.say(&ctx.http, alert).await {
                    println!("發送審核提醒失敗: {:?}", why);
                } else {
                    let _ = qmsg.reply(ctx, "申請已送出，請等待管理員審核。").await;
                }
            }
        }
    }
}

/// 處理 !setconfig 指令，格式：
/// !setconfig <registration_channel> <manual_channel> <admin_channel> <admin_role> <advanced_role>
async fn handle_setconfig(
    pool: &SqlitePool,
    ctx: &Context,
    msg: &Message,
    guild_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let member = msg.guild_id.unwrap().member(ctx, msg.author.id).await?;
    if !member.permissions(ctx)?.administrator() {
        let _ = msg.reply(ctx, "你沒有權限執行此命令。").await?;
        return Ok(());
    }
    let parts: Vec<&str> = msg.content.split_whitespace().collect();
    if parts.len() != 6 {
        let _ = msg.reply(
            ctx,
            "用法: !setconfig <registration_channel> <manual_channel> <admin_channel> <admin_role> <advanced_role>"
        ).await?;
        return Ok(());
    }

    let registration_channel = parts[1].parse::<u64>()?;
    let manual_channel = parts[2].parse::<u64>()?;
    let admin_channel = parts[3].parse::<u64>()?;
    let admin_role = parts[4].parse::<u64>()?;
    let advanced_role = parts[5].parse::<u64>()?;

    // 儲存設定至資料庫
    sqlx::query(
        "REPLACE INTO guild_configs (guild_id, registration_channel, manual_channel, admin_channel, admin_role, advanced_role)
         VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(guild_id as i64)
    .bind(registration_channel as i64)
    .bind(manual_channel as i64)
    .bind(admin_channel as i64)
    .bind(admin_role as i64)
    .bind(advanced_role as i64)
    .execute(pool)
    .await?;

    let _ = msg.reply(ctx, "成功設置配置信息！").await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT; // Example intents

    // 使用絕對路徑來設置 SQLite 連接
    let pool = SqlitePool::connect("sqlite:////data/bot.db").await.unwrap(); // 使用絕對路徑
    let (tx, rx) = mpsc::channel(100);
    let queue = Arc::new(QueueHolder { tx, rx: Mutex::new(rx) });

    let handler = Handler { pool, queue };
    let mut client = Client::builder("YOUR_BOT_TOKEN_HERE", intents)
        .event_handler(handler)
        .await
        .expect("創建客戶端失敗");

    if let Err(e) = client.start().await {
        println!("啟動失敗: {:?}", e);
    }
}
