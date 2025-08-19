use crate::Context;
use axum::{http::StatusCode, response::IntoResponse};
use serde::Deserialize;

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
	let file=crate::models::file::FileEntry::load_by_id(&ctx.db,session.user_id,payload.id).await;
	if let Ok(file)=file{
		if crate::models::file::FileEntry::delete(&ctx.db,&file.id).await.is_ok(){
			//TODO フォルダを削除したら中身を消す
			let c=crate::models::file::FileEntry::count_by_hash(&ctx.db,session.user_id,file.sha256.unwrap()).await;
			if let Ok(c)=c{
				if c>0{
					//skip delete body
					return (StatusCode::NO_CONTENT).into_response();
				}
			}
			let (s3,thumbnail)=futures_util::join!(async{
				if let Some(s3_key)=file.s3_key{
					ctx.bucket.delete_object(s3_key).await.err()
				}else{
					None
				}
			},async{
				if let Some(thumbnail)=file.thumbnail_key{
					ctx.bucket.delete_object(thumbnail).await.err()
				}else{
					None
				}
			});
			if let Some(e)=s3.as_ref(){
				eprintln!("{:?}",e);
			}
			if let Some(e)=thumbnail.as_ref(){
				eprintln!("{:?}",e);
			}
			if s3.is_some() || thumbnail.is_some(){
				return StatusCode::INTERNAL_SERVER_ERROR.into_response();
			}
			(StatusCode::NO_CONTENT).into_response()
		}else{
			(StatusCode::INTERNAL_SERVER_ERROR).into_response()
		}
	}else{
		(StatusCode::BAD_REQUEST).into_response()
	}
}
