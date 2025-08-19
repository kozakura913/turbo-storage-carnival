use axum::{http::StatusCode, response::{IntoResponse, Response}};
use futures::TryStreamExt;
use http_body_util::StreamBody;
use s3::{error::S3Error, Bucket};
use serde::{Deserialize, Serialize};

use crate::{models::file::FileEntry, Context};

#[derive(Debug,Serialize, Deserialize)]
pub struct RequestParams{
	key: Option<String>,
	thumbnail:Option<String>,
}
pub async fn get(
	ctx:Context,
	axum::extract::Path(id):axum::extract::Path<String>,
	authorization:Option<axum_extra::TypedHeader<axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>>>,
	cookie:Option<axum_extra::TypedHeader<axum_extra::headers::Cookie>>,
	range:Option<axum_extra::TypedHeader<axum_extra::headers::Range>>,
	axum::extract::Query(parms):axum::extract::Query<RequestParams>,
)->Result<(axum::http::StatusCode,axum::http::HeaderMap,axum::body::Body),Response>{
	let session=match ctx.session(authorization.as_ref(),cookie.as_ref()).await{
		Some(u)=>u,
		None=>return Err(StatusCode::FORBIDDEN.into_response())
	};
	let file=match i64::from_str_radix(&id,10){
		Ok(id)=>{
			FileEntry::load_by_id(&ctx.db,session.user_id,id).await.ok()
		},
		Err(_)=>None
	};
	if file.is_none(){
		return Err(axum::http::StatusCode::NOT_FOUND.into_response());
	}
	let file=file.unwrap();
	if file.s3_key.is_none(){
		return Err(axum::http::StatusCode::NO_CONTENT.into_response());
	}
	let mut header=axum::http::HeaderMap::new();
	let mut start=None;
	let mut end=None;
	if let Some(range)=range{
		let mut iter=range.0.satisfiable_ranges(file.size.try_into().unwrap());
		if let Some((s,e))=iter.next(){
			start=match s{
				std::ops::Bound::Included(v)=>Some(v),
				std::ops::Bound::Excluded(v)=>Some(v),
				_=>None,
			};
			end=match e{
				std::ops::Bound::Included(v)=>Some(v),
				std::ops::Bound::Excluded(v)=>Some(v),
				_=>None,
			};
		}
		if iter.next().is_some(){
			return Err(axum::http::StatusCode::RANGE_NOT_SATISFIABLE.into_response());
		}
	}
	if end.is_some()&&start.is_none(){
		return Err(axum::http::StatusCode::RANGE_NOT_SATISFIABLE.into_response());
	}
	let com=if start.is_none()&&end.is_none(){
		s3::command::Command::GetObject{}
	}else{
		s3::command::Command::GetObjectRange{start:start.unwrap_or(0),end}
	};
	println!("{:?}",com);
	let path=if parms.thumbnail.is_some(){
		if file.thumbnail_key.is_none(){
			return Err(axum::http::StatusCode::NOT_FOUND.into_response());
		}
		file.thumbnail_key.as_ref().unwrap().as_str()
	}else{
		file.s3_key.as_ref().unwrap().as_str()
	};
	println!("{}",path);
	let stream=head_and_stream(&ctx.bucket,path,com).await;
	if let Err(e)=stream.as_ref(){
		eprintln!("{:?}",e);
		if let S3Error::HttpFailWithBody(status_code,body)=e{
			let status=axum::http::StatusCode::from_u16(*status_code).unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
			if !status.is_success(){
				return Err((status,header,body.to_owned()).into_response());
			}
		}else{
			return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response());
		}
	}
	let (s3_header,stream)=stream.unwrap();
	let status=axum::http::StatusCode::from_u16(stream.status_code).unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
	if !status.is_success(){
		return Err((status,header).into_response());
	}
	let stream=stream.bytes.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));
	let stream=StreamBody::new(stream);
	let body = axum::body::Body::from_stream(stream);
	if let Some(s)=s3_header.content_type.as_ref(){
		header.append(axum::http::header::CONTENT_TYPE,s.try_into().unwrap());
	}
	if let Some(len)=s3_header.content_length{
		let s=start.unwrap_or(0);
		let object_length=if s==0{
			len.to_string()
		}else{
			file.size.to_string()
		};
		header.append(axum::http::header::CONTENT_RANGE,format!("bytes {}-{}/{}",s,s+len as u64-1,object_length).try_into().unwrap());
		header.append(axum::http::header::CONTENT_LENGTH,len.to_string().try_into().unwrap());
		header.append(axum::http::header::ACCEPT_RANGES,"bytes".to_owned().try_into().unwrap());
	}
	Ok((status,header,body))
}
pub async fn head_and_stream(bucket:&Bucket,path:&str,com:s3::command::Command<'_>)->Result<(s3::serde_types::HeadObjectResult, s3::request::ResponseDataStream),s3::error::S3Error>{
	use s3::serde_types::HeadObjectResult;
	use s3::request::Request;
	let request =s3::request::tokio_backend::HyperRequest::new(bucket, path, com).await?;
	let response=request.response().await?;
	//let response=client.get(format!("{}/{}",bucket.url(),&path).try_into().unwrap()).await?;
	let status=response.status();
	let headers=response.headers();
	let header_object = HeadObjectResult::from(headers);
	let stream = response.into_body().into_stream().map_err(|b| s3::error::S3Error::Hyper(b));
	Ok((header_object,s3::request::ResponseDataStream {
		bytes: Box::pin(stream),
		status_code: status.as_u16(),
	}))
}
