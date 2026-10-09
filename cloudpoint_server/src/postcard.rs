use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub struct Postcard<T: Serialize>(pub T);

impl<T: Serialize> IntoResponse for Postcard<T> {
    fn into_response(self) -> Response {
        match postcard::to_allocvec(&self.0) {
            Ok(data) => data.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }
}
