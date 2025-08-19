use crate::{models, Context};
use axum::{http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug,Deserialize)]
pub(crate) struct RequestBody {
	username:String,
	password:String,
}
#[derive(Serialize)]
struct ResponseData {
	session_id:String,
}
pub async fn post(
	ctx:Context,
	axum::Json(payload): axum::Json<RequestBody>,
)->axum::response::Response{
	let start=chrono::Utc::now();
	let user=models::user::User::load_by_username(&ctx.db,&payload.username).await;
	let user=if user.as_ref().map(|u|u.verify(&payload.password).unwrap_or(false)).unwrap_or(false){
		user.unwrap()
	}else{
		//認証失敗時の応答時間を10秒に均一化する
		let diff=chrono::Utc::now()-start;
		let ms=0.max(10*1000-diff.num_milliseconds()).try_into().unwrap();
		if ms>0{
			tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
		}
		return StatusCode::FORBIDDEN.into_response();
	};
	let session_id=ctx.login(user).await;
	let max_age=12*60*60;//セッション有効期限12時間
	let set_cookie=format!("SID={}; Max-Age={}; Path=/; SameSite=Strict",session_id,max_age);
	let json=serde_json::to_string(&ResponseData{
		session_id,
	});
	let mut header=axum::http::header::HeaderMap::new();
	header.insert(axum::http::header::CONTENT_TYPE,"application/json".parse().unwrap());
	header.insert(axum::http::header::SET_COOKIE,set_cookie.parse().unwrap());
	(StatusCode::OK,header,json.unwrap_or_default()).into_response()
}
