use crate::{models::file::FileEntry, Context};
use axum::{http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct Files {
	directory:Option<String>,
	since_id: Option<i64>,
	until_id: Option<i64>,
	limit:Option<i16>,
}
#[derive(Serialize)]
pub(super) struct ResponseFile {
	id:i64,
	directory:String,
	name:String,
	updated_at:String,
	sha256:Option<String>,
	content_type:String,
	metadata:serde_json::Value,
	blurhash:Option<String>,
	size:i64,
}
impl From<FileEntry> for ResponseFile{
	fn from(value: FileEntry) -> Self {
		Self{
			id:value.id,
			directory:value.directory,
			name:value.name,
			updated_at:value.updated_at.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
			sha256:value.sha256,
			metadata:value.metadata.as_ref().map(|s|serde_json::from_str(s).ok()).unwrap_or_default().unwrap_or_default(),
			blurhash:value.blurhash,
			content_type:value.content_type,
			size:value.size,
		}
	}
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
	let since_id=payload.since_id.unwrap_or(0);
	let until_id=payload.until_id.unwrap_or(i64::MAX);
	let mut resp_files=Vec::new();
	if let Ok(files)=crate::models::file::FileEntry::load(&ctx.db,session.user_id,payload.directory,since_id..until_id,payload.limit.unwrap_or(10).min(50).max(1).into()).await{
		for f in files{
			let f=Into::<ResponseFile>::into(f);
			if let Ok(f)=serde_json::to_value(f){
				resp_files.push(f);
			}
		}
	}
	let json=serde_json::Value::Array(resp_files);
	let mut header=axum::http::header::HeaderMap::new();
	header.insert(axum::http::header::CONTENT_TYPE,"application/json".parse().unwrap());
	(StatusCode::OK,header,serde_json::to_string(&json).unwrap_or_default()).into_response()
}
