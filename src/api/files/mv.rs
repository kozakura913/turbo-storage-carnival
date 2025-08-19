use crate::{models::file::FileEntry, Context};
use axum::{http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct Files {
	id: i64,
	name:Option<String>,
	directory:Option<String>,
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
	if let Some(d)=&payload.name{
		//TODO ディレクトリをmvする操作をサポートする
		if d.contains("/")||d.contains("\0"){
			return StatusCode::BAD_REQUEST.into_response();
		}
	}
	if let Some(d)=&payload.directory{
		if !d.starts_with("/")||!d.ends_with("/")||d.contains("\0"){
			return StatusCode::BAD_REQUEST.into_response();
		}
	}
	let file=crate::models::file::FileEntry::load_by_id(&ctx.db,session.user_id,payload.id).await;
	if let Ok(file)=file{
		let directory=payload.directory.as_ref().unwrap_or(&file.directory);
		let name=payload.name.as_ref().unwrap_or(&file.name);
		if directory==&file.directory&&name==&file.name{
			(StatusCode::BAD_REQUEST).into_response()
		}else if crate::models::file::FileEntry::update_path(&ctx.db,file.user_id,file.id,directory,name).await.is_ok(){
			(StatusCode::NO_CONTENT).into_response()
		}else{
			(StatusCode::INTERNAL_SERVER_ERROR).into_response()
		}
	}else{
		(StatusCode::BAD_REQUEST).into_response()
	}
}
