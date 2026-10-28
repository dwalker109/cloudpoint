use crate::config::APP_VER;
use cloudpoint_lib::http::CurlHttpClient;
use std::sync::LazyLock;

static HTTP_CLIENT: LazyLock<CurlHttpClient> =
    LazyLock::new(|| CurlHttpClient::new(&*APP_VER).expect("should init shared libcurl client"));

pub(super) mod connect;
pub(super) mod self_update;
pub(super) mod sync;
