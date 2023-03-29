drop table if exists posts cascade;
drop table if exists user_channel cascade ;
drop table if exists users cascade ;
drop table if exists channels cascade;

CREATE TABLE posts (
                       id serial primary key,
                       title text,
                       link text not null,
                       telegram_id bigint not null,
                       pub_date integer not null,
                       content text not null,
                       chat_id bigint not null
);

create table channels (
                          id bigint not null primary key,
                          title text not null,
                          username text not null unique
);

create table users (
    id bigint not null primary key,
    enabled bool not null default true,
    chat_id bigint not null
);

create table user_channel (
    id serial primary key,
    user_id bigint not null references users(id),
    channel_id bigint not null references channels(id),
    unique (user_id, channel_id)
);
