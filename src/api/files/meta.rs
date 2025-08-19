use crate::Context;
use axum::{http::StatusCode, response::IntoResponse};
use serde::Deserialize;

use super::list::ResponseFile;

#[derive(Deserialize)]
pub(crate) struct Files {
	id: i64,
}
pub async fn post(
	ctx:Context,
	authorization:Option<axum_extra::TypedHeader<axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>>>,
	cookie:Option<axum_extra::TypedHeader<axum_extra::headers::Cookie>>,
	axum::Json(payload): axum::Json<Files>,
)->axum::response::Response{
	let session=match ctx.session(authorization.as_ref(),cookie.as_ref()).await{
		Some(u)=>u,
		None=>return StatusCode::FORBIDDEN.into_response()
	};
	let mut json=serde_json::Value::Null;
	if let Ok(file)=crate::models::file::FileEntry::load_by_id(&ctx.db,session.user_id, payload.id).await{
		let f=Into::<ResponseFile>::into(file);
		if let Ok(f)=serde_json::to_value(f){
			json=f;
		}
	}
	let mut header=axum::http::header::HeaderMap::new();
	header.insert(axum::http::header::CONTENT_TYPE,"application/json".parse().unwrap());
	(StatusCode::OK,header,serde_json::to_string(&json).unwrap_or_default()).into_response()
}
