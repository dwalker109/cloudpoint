use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub struct Toml<T>(pub T);

impl<T: Serialize> IntoResponse for Toml<T> {
    fn into_response(self) -> Response {
        match toml::to_string(&self.0) {
            Ok(body) => ([(header::CONTENT_TYPE, "application/toml")], body).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }
}
