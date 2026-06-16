create table public.chunks
(
    id          uuid                     default gen_random_uuid() not null primary key,
    user_key    uuid                                               not null,
    xxhash3_128 bytea,
    sha256      bytea,
    body_gz     bytea                                              not null,
    body_len    integer                                            not null,
    created_at  timestamp with time zone default now()             not null,
    unique (user_key, xxhash3_128),
    unique (user_key, sha256)
);

alter table public.chunks
    alter column body_gz set storage external;

alter table public.chunks
    owner to postgres;
