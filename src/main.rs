mod bot;
mod structs;

use std::time::Duration;
use sqlx::Pool;
use bot::Bot;
use serenity::client::bridge::gateway::ShardManager;
pub type Error = Box<dyn std::error::Error>;
use serenity::{client::ClientBuilder, prelude::GatewayIntents};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::sync::{
    Arc, Mutex
};

use serenity::prelude::Mutex as Gaytex;

const INTENTS: GatewayIntents = GatewayIntents::from_bits_truncate(
    GatewayIntents::DIRECT_MESSAGES.bits()
        | GatewayIntents::GUILD_MESSAGES.bits()
        | GatewayIntents::GUILDS.bits()
        | GatewayIntents::MESSAGE_CONTENT.bits(),
);


pub struct ShardManagerContainer;
pub struct DBPool;

impl serenity::prelude::TypeMapKey for ShardManagerContainer {
    type Value = Arc<Gaytex<ShardManager>>;
}

impl serenity::prelude::TypeMapKey for DBPool {
    type Value = sqlx::SqlitePool;
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt().init();
    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN missing");
    let appid: u64 = std::env::var("DISCORD_APPID")
        .expect("DISCORD_APPID missing").parse().expect("DISCORD_APPID invalid");

    let sql = {
        let opts = SqliteConnectOptions::new()
            .create_if_missing(true)
            .filename("bot.db")
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_lifetime(Duration::from_secs(3600))
            .max_connections(2)
            .connect_with(opts)
            .await
            .expect("failed to connect to DB");
        sqlx::migrate!("./migrations").run(&pool).await.expect("Couldn't run database migrations");
       // Bot::new(pool, guild)
       pool
    };

    let roles = structs::GuildRoleSettings {
        boomer:              serenity::model::id::RoleId(968224875175116840),
        fussvolk:            serenity::model::id::RoleId(968221117569437746),
        fussvoelkchen:       serenity::model::id::RoleId(968221338483425330),
        male_role:           serenity::model::id::RoleId(968224375260184666),
        female_role:         serenity::model::id::RoleId(968221397904134194),
        others_role:         serenity::model::id::RoleId(968224452166946868),
        asd_role:            serenity::model::id::RoleId(968224526284492861),
        non_asd_role:        serenity::model::id::RoleId(968224618391433316),
        default_member_role: serenity::model::id::RoleId(968224737589338172),
        f_adult:             serenity::model::id::RoleId(968229905642958908),
        f_child:             serenity::model::id::RoleId(968230028716408842),
    };

    let bot = bot::Bot {
        database: sql,
        roles
    };


    let mut client = serenity::Client::builder(&token, INTENTS)
        .event_handler(bot)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
    }

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Could not register ctrl+c handler");
        shard_manager.lock().await.shutdown_all().await;
    });

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }




        Ok(())
}
