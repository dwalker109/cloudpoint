create table public.versions
(
    id          uuid                     default gen_random_uuid() not null primary key,
    user_key    uuid                                               not null,
    sync_item   varchar                                            not null,
    xxhash3_128 bytea,
    sha256      bytea,
    body        bytea                                              not null,
    created_at  timestamp with time zone default now()             not null,
    unique (user_key, sync_item, xxhash3_128),
    unique (user_key, sync_item, sha256)
);

alter table public.versions
    owner to postgres;
