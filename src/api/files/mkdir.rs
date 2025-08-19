use crate::{models::file::FileEntry, Context};
use axum::{http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use super::list::ResponseFile;

#[derive(Deserialize)]
pub(crate) struct Files {
	directory:String,
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
	if payload.directory.contains("\0"){
		//使用できない文字を含む
		return StatusCode::BAD_REQUEST.into_response();
	}
	if !payload.directory.starts_with("/")||!payload.directory.ends_with("/"){
		return StatusCode::BAD_REQUEST.into_response();
	}
	let name=&payload.directory[..payload.directory.len()-1];
	let d=if let Some(pos)=name.rfind("/"){
		&payload.directory[..pos]
	}else{
		""
	};
	let d=format!("{}/",d);
	let n=&name[d.len()..];
	let n=format!("{}/",n);
	println!("{}?{}",d,n);
	let conn=ctx.db.get().await;
	if let Some(mut conn)=conn{
		if crate::models::file::FileEntry::load_by_path(&ctx.db,session.user_id,&d,&n).await.is_ok(){
			return StatusCode::CONFLICT.into_response();
		}
		if crate::models::file::mkdirs(&mut conn,session.user_id,&payload.directory).await.is_ok(){
			let e=crate::models::file::FileEntry::load_by_path(&ctx.db,session.user_id,&d,&n).await;
			match e{
				Ok(file)=>{
					let mut json=serde_json::Value::Null;
					let f=Into::<ResponseFile>::into(file);
					if let Ok(f)=serde_json::to_value(f){
						json=f;
					}
					let mut header=axum::http::header::HeaderMap::new();
					header.insert(axum::http::header::CONTENT_TYPE,"application/json".parse().unwrap());
					(StatusCode::OK,header,serde_json::to_string(&json).unwrap_or_default()).into_response()
				},
				Err(e)=>{
					eprintln!("{}:{} {:?}",file!(),line!(),e);
					(StatusCode::NO_CONTENT).into_response()
				}
			}
		}else{
			(StatusCode::INTERNAL_SERVER_ERROR).into_response()
		}
	}else{
		(StatusCode::INTERNAL_SERVER_ERROR).into_response()
	}
}
