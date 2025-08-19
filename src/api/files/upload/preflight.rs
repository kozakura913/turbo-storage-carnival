use crate::Context;
use axum::{http::StatusCode, response::IntoResponse};
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Debug,Deserialize)]
pub(crate) struct RequestBody {
	title:Option<String>,
	last_modified:Option<String>,
	directory:Option<String>,
}
#[derive(Serialize)]
struct ResponseData {
	session_id:String,
}
pub async fn post(
	ctx:Context,
	authorization:Option<axum_extra::TypedHeader<axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>>>,
	cookie:Option<axum_extra::TypedHeader<axum_extra::headers::Cookie>>,
	axum::Json(payload): axum::Json<RequestBody>,
)->axum::response::Response{
	println!("{:?}",payload);
	let session=match ctx.session(authorization.as_ref(),cookie.as_ref()).await{
		Some(u)=>u,
		None=>return StatusCode::FORBIDDEN.into_response()
	};
	if let Some(t)=&payload.title{
		if t.contains("/")||t.contains("\0"){
			//使用できない文字を含む
			return StatusCode::BAD_REQUEST.into_response();
		}
	}
	if let Some(d)=&payload.directory{
		if !d.starts_with("/")||d.contains("\0"){
			return StatusCode::BAD_REQUEST.into_response();
		}
	}
	let session_id=base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8;32]>());
	let s3_key=ctx.id_service.gen_now();
	let session=crate::UploadSession{
		hasher: crate::Hasher::new(),
		content_type: None,
		ext: None,
		s3_key,
		directory:payload.directory.unwrap_or_else(||"/".into()),
		content_length:0,
		title:payload.title,
		upload_id:None,
		part_number: None,
		part_etag:Vec::new(),
		user_id:session.user_id,
		last_modified:payload.last_modified,
	};
	ctx.write_upload_session(session,session_id.clone()).await;
	let json=serde_json::to_string(&ResponseData{
		session_id,
	});
	let mut header=axum::http::header::HeaderMap::new();
	header.insert(axum::http::header::CONTENT_TYPE,"application/json".parse().unwrap());
	(StatusCode::OK,header,json.unwrap_or_default()).into_response()
}
