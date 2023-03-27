drop table if exists posts;
drop table if exists channels;

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
                          id serial primary key,
                          title text not null,
                          username text not null unique,
                          telegram_id bigint not null
);