use serenity::client::bridge::gateway::event::ShardStageUpdateEvent;
use serenity::json::Value;
use serenity::model::channel::{Channel, ChannelCategory, GuildChannel, Reaction, StageInstance};
use serenity::model::event::{
    ChannelPinsUpdateEvent, GuildMemberUpdateEvent, GuildMembersChunkEvent, InviteCreateEvent,
    InviteDeleteEvent, MessageUpdateEvent, ResumedEvent, ThreadListSyncEvent,
    ThreadMembersUpdateEvent, TypingStartEvent, VoiceServerUpdateEvent,
};
use serenity::model::gateway::Presence;
use serenity::model::guild::{
    Emoji, Guild, Integration, PartialGuild, ThreadMember, UnavailableGuild,
};
use serenity::model::id::{ApplicationId, EmojiId, IntegrationId, MessageId, StickerId};
use serenity::model::prelude::{CurrentUser, ReactionType, Sticker, User, VoiceState};
use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{
        channel::{ChannelType, Message, PartialChannel, PartialGuildChannel},
        gateway::Ready,
        guild::{Member, Role},
        id::{ChannelId, GuildId, RoleId, UserId},
        interactions::{
            application_command::{
                ApplicationCommandInteraction,
                ApplicationCommandInteractionDataOptionValue as OptionValue,
                ApplicationCommandOptionType, ApplicationCommandType,
            },
            message_component::ButtonStyle,
            Interaction, InteractionApplicationCommandCallbackDataFlags,
        },
    },
    prelude::Mentionable,
    utils::{Color, MessageBuilder},
};
use sqlx::{sqlite, FromRow, SqlitePool};
use std::collections::HashMap;

enum DiagnosisStatus {
    Formal,
    Questioning,
    SelfDiagnose,
    FriendOrFamily,
}

enum Gender {
    Male,
    Female,
    Divers,
}

struct FormAnswers {
    pub discord_tag: String,
    pub status: DiagnosisStatus,
    pub gender: Gender,
    pub is_18_plus: bool,
    pub is_30_plus: bool,
    pub is_female: bool,
}

#[derive(Debug)]
struct FormAnswersDB {
    pub message_id: i64,
    pub user_id: i64,
    pub age: Option<String>,
    pub gender: String,
    pub is_female: bool,
    pub is_18_plus: bool,
    pub is_30_plus: bool,
    pub diagnosis_status: Option<String>,
}

pub struct Bot {
    pub database: sqlx::SqlitePool,
    pub roles: crate::structs::GuildRoleSettings,
    pub responses_channel: ChannelId

}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }

    async fn guild_member_removal(
        &self,
        ctx: Context,
        guild_id: GuildId,
        user: User,
        member_data_if_available: Option<Member>,
    ) {
        // lookup form answers if available
        // get message from db
        let uid = user.id.0 as i64;
        let ee = sqlx::query_as!(
            FormAnswersDB,
            "SELECT * FROM formanswers WHERE user_id = ?",
            uid
        )
        .fetch_one(&self.database)
        .await;

        match ee {
            Ok(usr) => {
                let mut msg = ctx.http.get_message(self.responses_channel.0, usr.message_id as u64).await.unwrap();

                msg.edit(&ctx, |f| {
                    f.components(|c| {
                        c.create_action_row(|a| {
                            a.create_button(|b| {
                                b.custom_id("left_user");
                                b.disabled(true);
                                b.style(ButtonStyle::Secondary);
                                b.label("User left Server")
                            })
                        })
                    })
                }).await.unwrap();
            
            },
            Err(_) => {
                println!("User didnt fill out form");
                return;
            }
        }

    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.id != serenity::model::prelude::UserId(968523052247818382) {
            return; // only listen to our webhook
        }

        // clone embed
        let embed = &msg.embeds[0];

        // get fields
        let mut fields = Vec::new();
        for field in embed.fields.iter() {
            fields.push(field.clone());
        }

        let answers = parse_form_answers(fields.clone()).await.unwrap();

        let users_matching_user = msg
            .guild(&ctx)
            .unwrap()
            .search_members(&ctx, &answers.discord_tag, Some(100))
            .await
            .unwrap();

        // find correct user
        let mut user_id = None;
        for member in users_matching_user.iter() {
            if answers.discord_tag.contains(&member.user.name.as_str())
                && !member.roles.contains(&self.roles.default_member_role)
            {
                user_id = Some(member.user.id);
                break;
            }
        }

        let uid = match user_id {
            Some(id) => id,
            None => {
                msg.channel_id.send_message(&ctx, |f| {
                    f.embed(|e| {
                        e.title("New Submission");
                        e.description(format!("New Submission - However, the user {} could not be found in the server", answers.discord_tag));
                        e.color(Color::DARK_RED);
                        e
                    })
                }).await.unwrap();

                msg.delete(&ctx).await.unwrap();

                return;
            }
        };

        let new_msg = msg
            .channel_id
            .send_message(&ctx, |f| {
                f.content(format!("User Mention: <@{}>", uid));
                f.embed(|e| {
                    e.title("New Form Submission");
                    e.color(Color::BLURPLE);
                    e.fields(
                        fields
                            .iter()
                            .map(|f| (f.name.clone(), f.value.clone(), false)),
                    );
                    e.footer(|f| {
                        f.text(format!("Gotten UserId {}", uid));
                        f
                    });
                    e
                });
                f.components(|c| {
                    c.create_action_row(|a| {
                        a.create_button(|b| {
                            b.label("Accept");
                            b.style(ButtonStyle::Success);
                            b.custom_id("approve_user");
                            b
                        });
                        a.create_button(|b| {
                            b.label("Deny & Ban");
                            b.style(ButtonStyle::Danger);
                            b.custom_id("reject_user_and_ban");
                            b
                        });
                        a.create_button(|b| {
                            b.label("Deny & Kick");
                            b.style(ButtonStyle::Danger);
                            b.custom_id("reject_user_and_kick");
                            b
                        })
                    })
                })
            })
            .await
            .unwrap();

        // delete trigger message
        msg.delete(&ctx).await.unwrap();

        let g = match answers.gender {
            Gender::Male => "Male",
            Gender::Female => "Female",
            Gender::Divers => "Other",
        };

        let d = match answers.status {
            DiagnosisStatus::Formal => "Formal",
            DiagnosisStatus::Questioning => "Questioning",
            DiagnosisStatus::SelfDiagnose => "Self Diagnosed",
            DiagnosisStatus::FriendOrFamily => "Family Member or Friend of an Autistic Individual.",
            _ => unimplemented!("Sag wallah"),
        };

        let n_msgid = new_msg.id.0 as i64;
        let n_uid = uid.0 as i64;

        // save to db
        let _ = sqlx::query!(
            "INSERT INTO formanswers (message_id, user_id, gender, is_female, is_18_plus, is_30_plus, diagnosis_status) VALUES (?, ?, ?, ?, ?, ?, ?)",
            n_msgid, n_uid, g, answers.is_female, answers.is_18_plus, answers.is_30_plus, d

        )
        .execute(&self.database)
        .await.unwrap();
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::MessageComponent(mut msgc) = interaction {
            let intaraction_message_id = msgc.message.id.0 as i64;

            if msgc.data.custom_id == "approve_user" {
                let _ = msgc.create_interaction_response(&ctx, |f| {
                    f.kind(serenity::model::interactions::InteractionResponseType::DeferredChannelMessageWithSource);
                    f.interaction_response_data(|f| f.flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))
                }).await;

                // get message from db
                let ee = sqlx::query_as!(
                    FormAnswersDB,
                    "SELECT * FROM formanswers WHERE message_id = ?",
                    intaraction_message_id
                )
                .fetch_one(&self.database)
                .await;

                let frm = match ee {
                    Ok(m) => m,
                    Err(e) => {
                        msgc.edit_original_interaction_response(&ctx, |f| {
                            f.embed(|e| {
                                e.title("Error");
                                e.description("Could not find message in database");
                                e.color(Color::DARK_RED);
                                e
                            });
                            f
                        })
                        .await
                        .unwrap();
                        return;
                    }
                };

                // match roles
                let mut roles = Vec::new();

                roles.push(self.roles.default_member_role);

                if frm.is_18_plus {
                    roles.push(self.roles.fussvolk);
                }
                if frm.is_30_plus {
                    roles.push(self.roles.boomer);
                }

                if !frm.is_18_plus && !frm.is_30_plus {
                    roles.push(self.roles.fussvoelkchen);
                }

                if frm.is_female && !frm.is_18_plus && !frm.is_30_plus {
                    roles.push(self.roles.f_child);
                }

                if frm.is_female && frm.is_18_plus {
                    roles.push(self.roles.f_adult);
                }

                if frm.is_female && frm.is_30_plus {
                    roles.push(self.roles.f_adult);
                }

                match frm.diagnosis_status.unwrap().as_str() {
                    "Family Member or Friend of an Autistic Individual." => {
                        roles.push(self.roles.non_asd_role);
                    }
                    _ => {
                        roles.push(self.roles.asd_role);
                    }
                }

                // add user to roles
                let usr = UserId(frm.user_id as u64);
                let mut mem = ctx
                    .http
                    .get_member(msgc.guild_id.unwrap().0, usr.0)
                    .await
                    .unwrap();
                for role in roles {
                    let _ = mem.add_role(&ctx, role).await;
                }

                let _ = msgc
                    .edit_original_interaction_response(&ctx, |f| {
                        f.embed(|e| {
                            e.title("Approved");
                            e.description("User has been approved");
                            e.color(Color::DARK_GREEN);
                            e
                        });
                        f
                    })
                    .await;

                // edit out buttons from og message
                let _ = msgc
                    .message
                    .edit(&ctx, |f| {
                        f.components(|f| {
                            f.create_action_row(|a| {
                                a.create_button(|b| {
                                    b.label("Approved");
                                    b.style(ButtonStyle::Success);
                                    b.custom_id("approved");
                                    b.disabled(true);
                                    b.emoji(ReactionType::Custom {
                                        animated: false,
                                        id: EmojiId(969988145259102258),
                                        name: Some("greentick".to_string()),
                                    });
                                    b
                                });
                                a.create_button(|b| {
                                    b.label(format!("Action performed by {}", msgc.user.tag()));
                                    b.style(ButtonStyle::Secondary);
                                    b.custom_id("moderator_action");
                                    b.disabled(true);
                                    b.emoji(ReactionType::Custom {
                                        animated: false,
                                        id: EmojiId(900453862702469150),
                                        name: Some("LogoModSystem".to_string()),
                                    });
                                    b
                                })
                            })
                        })
                    })
                    .await;
            } else if msgc.data.custom_id == "reject_user_and_ban" {
                let _ = msgc.create_interaction_response(&ctx, |f| {
                    f.kind(serenity::model::interactions::InteractionResponseType::DeferredChannelMessageWithSource);
                    f.interaction_response_data(|f| f.flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))
                }).await;

                // get message from db
                let ee = sqlx::query_as!(
                    FormAnswersDB,
                    "SELECT * FROM formanswers WHERE message_id = ?",
                    intaraction_message_id
                )
                .fetch_one(&self.database)
                .await;

                let frm = match ee {
                    Ok(m) => m,
                    Err(e) => {
                        msgc.edit_original_interaction_response(&ctx, |f| {
                            f.embed(|e| {
                                e.title("Error");
                                e.description("Could not find message in database");
                                e.color(Color::DARK_RED);
                                e
                            });
                            f
                        })
                        .await
                        .unwrap();
                        return;
                    }
                };

                let usr = UserId(frm.user_id as u64);
                let mem = ctx
                    .http
                    .get_member(msgc.guild_id.unwrap().0, usr.0)
                    .await
                    .unwrap();
                mem.ban(&ctx, 0).await.unwrap();

                let _ = msgc
                    .message
                    .edit(&ctx, |f| {
                        f.components(|f| {
                            f.create_action_row(|a| {
                                a.create_button(|b| {
                                    b.label("Banned");
                                    b.style(ButtonStyle::Danger);
                                    b.custom_id("approved");
                                    b.disabled(true);
                                    b.emoji(ReactionType::Custom {
                                        animated: false,
                                        id: EmojiId(567088349484023818),
                                        name: Some("redtick".to_string()),
                                    });
                                    b
                                });
                                a.create_button(|b| {
                                    b.label(format!("Action performed by {}", msgc.user.tag()));
                                    b.style(ButtonStyle::Secondary);
                                    b.custom_id("moderator_action");
                                    b.disabled(true);
                                    b.emoji(ReactionType::Custom {
                                        animated: false,
                                        id: EmojiId(900453862702469150),
                                        name: Some("LogoModSystem".to_string()),
                                    });
                                    b
                                })
                            })
                        })
                    })
                    .await;

                let _ = msgc
                    .edit_original_interaction_response(&ctx, |f| {
                        f.embed(|e| {
                            e.title("Rejected");
                            e.description("User has been banned");
                            e.color(Color::DARK_RED);
                            e
                        });
                        f
                    })
                    .await
                    .unwrap();
            } else if msgc.data.custom_id == "reject_user_and_kick" {
                let _ = msgc.create_interaction_response(&ctx, |f| {
                    f.kind(serenity::model::interactions::InteractionResponseType::DeferredChannelMessageWithSource);
                    f.interaction_response_data(|f| f.flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))
                }).await;

                // get message from db
                let ee = sqlx::query_as!(
                    FormAnswersDB,
                    "SELECT * FROM formanswers WHERE message_id = ?",
                    intaraction_message_id
                )
                .fetch_one(&self.database)
                .await;

                let frm = match ee {
                    Ok(m) => m,
                    Err(e) => {
                        msgc.edit_original_interaction_response(&ctx, |f| {
                            f.embed(|e| {
                                e.title("Error");
                                e.description("Could not find message in database");
                                e.color(Color::DARK_RED);
                                e
                            });
                            f
                        })
                        .await
                        .unwrap();
                        return;
                    }
                };

                let usr = UserId(frm.user_id as u64);
                let mem = ctx
                    .http
                    .get_member(msgc.guild_id.unwrap().0, usr.0)
                    .await
                    .unwrap();
                mem.kick(&ctx).await.unwrap();

                let _ = msgc
                    .message
                    .edit(&ctx, |f| {
                        f.components(|f| {
                            f.create_action_row(|a| {
                                a.create_button(|b| {
                                    b.label("Kicked");
                                    b.style(ButtonStyle::Danger);
                                    b.custom_id("approved");
                                    b.disabled(true);
                                    b.emoji(ReactionType::Custom {
                                        animated: false,
                                        id: EmojiId(567088349484023818),
                                        name: Some("redtick".to_string()),
                                    });
                                    b
                                });
                                a.create_button(|b| {
                                    b.label(format!("Action performed by {}", msgc.user.tag()));
                                    b.style(ButtonStyle::Secondary);
                                    b.custom_id("moderator_action");
                                    b.disabled(true);
                                    b.emoji(ReactionType::Custom {
                                        animated: false,
                                        id: EmojiId(900453862702469150),
                                        name: Some("LogoModSystem".to_string()),
                                    });
                                    b
                                })
                            })
                        })
                    })
                    .await;

                let _ = msgc
                    .edit_original_interaction_response(&ctx, |f| {
                        f.embed(|e| {
                            e.title("Rejected");
                            e.description("User has been kicked");
                            e.color(Color::DARK_RED);
                            e
                        });
                        f
                    })
                    .await
                    .unwrap();
            }
        }
    }
}

async fn parse_form_answers(
    s: Vec<serenity::model::prelude::EmbedField>,
) -> Result<FormAnswers, Box<dyn std::error::Error>> {
    let discord_tag = &s[0].value;
    let status = match s[1].value.as_str() {
        "Formally diagnosed with ASD (Autism spectrum Disorder)" => DiagnosisStatus::Formal,
        "Questioning ASD" => DiagnosisStatus::Questioning,
        "Self Diagnosed" => DiagnosisStatus::SelfDiagnose,
        "Family Member or Friend of an Autistic Individual." => DiagnosisStatus::FriendOrFamily,
        _ => panic!("Sir, what the fuck"),
    };
    let gender = match s[2].value.as_str() {
        "Male" => Gender::Male,
        "Female" => Gender::Female,
        "Other (Non-Binary, Transgender, ETC...)" => Gender::Divers,
        _ => panic!("sir please"),
    };
    let is_over_18 = match s[3].value.as_str() {
        "Yes" => true,
        "No" => false,
        _ => false,
    };

    let is_over_30 = match s[4].value.as_str() {
        "Yes" => true,
        "No" => false,
        _ => false,
    };

    let is_female = match gender {
        Gender::Female => true,
        _ => false,
    };

    Ok(FormAnswers {
        discord_tag: discord_tag.to_string(),
        status,
        gender,
        is_18_plus: is_over_18,
        is_30_plus: is_over_30,
        is_female,
    })
}
