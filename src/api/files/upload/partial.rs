use crate::Context;
use axum::{http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use futures::TryStreamExt;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

#[derive(Debug,Serialize, Deserialize)]
pub struct RequestParams{
	part: u32,
}
pub async fn post(
	ctx:Context,
	axum::extract::Query(parms):axum::extract::Query<RequestParams>,
	request: axum::extract::Request,
)->axum::response::Response{
	let authorization=request.headers().get("Authorization").cloned();
	let (mut session,session_id)=match ctx.upload_session(authorization.as_ref(),false).await{
		Ok(v)=>v,
		Err(e)=>return e,
	};
	let body=request.into_body();
	let body=body.into_data_stream();
	let body_with_io_error = body.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));
	let body_reader = StreamReader::new(body_with_io_error);
	futures::pin_mut!(body_reader);
	//println!("{:?}",session);
	let buf={
		let mut all_body=vec![];
		let mut buf=vec![0;4096];
		loop{
			match body_reader.read(&mut buf).await{
				Ok(len)=>{
					if len==0{
						break;
					}
					if all_body.len()+len>ctx.config.part_max_size as usize{
						return (StatusCode::PAYLOAD_TOO_LARGE).into_response();
					}
					all_body.extend_from_slice(&buf[0..len]);
				},
				Err(e)=>{
					eprintln!("{}:{} {:?}",file!(),line!(),e);
					return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
				}
			}
		}
		all_body
	};
	if let Some(v)=session.part_number.as_mut(){
		if *v+1 == parms.part{
			*v+=1;
		}else{
			return (StatusCode::BAD_REQUEST).into_response();
		}
	}else{
		if parms.part==0{
			session.part_number=Some(0);
		}else{
			return (StatusCode::BAD_REQUEST).into_response();
		}
	}
	if session.part_number==Some(0){
		//最初のオブジェクト
		let mut ext=None;
		let mut content_type="";
		if let Some(kind)=infer::get(&buf){
			content_type=kind.mime_type();
			ext=Some(format!(".{}",kind.extension()));
			//println!("known content_type:{}",content_type);
		}
		if ext.as_ref().map(|s|s.as_str()) == Some("") {
			ext=match content_type{
				"image/jpeg"=>Some(".jpg"),
				"image/png"=>Some(".png"),
				"image/webp"=>Some(".webp"),
				"image/avif"=>Some(".avif"),
				"image/apng"=>Some(".apng"),
				"image/vnd.mozilla.apng"=>Some(".apng"),
				_=>None,
			}.map(|s|s.to_owned());
		}
		if content_type == "image/apng"{
			content_type="image/png";
		}
		if !crate::browsersafe::FILE_TYPE_BROWSERSAFE.contains(&content_type){
			content_type = "application/octet-stream";
			ext = None;
		}
		session.content_type=Some(content_type.to_owned());
		session.ext=ext;
		session.upload_id=match ctx.bucket.initiate_multipart_upload(&session.s3_key,content_type).await{
			Ok(imur)=>{
				Some(imur.upload_id)
			},
			Err(e)=>{
				eprintln!("{}:{} {:?}",file!(),line!(),e);
				return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
			}
		};
	}
	session.hasher.update(&buf);
	session.content_length+=buf.len() as u64;
	let temp_id=format!("s3_wait_etag:{}-{}",session.upload_id.as_ref().unwrap(),session.part_number.unwrap());
	session.part_etag.push(temp_id.clone());
	ctx.write_upload_session(session.clone(),session_id.clone()).await;
	tokio::runtime::Handle::current().spawn(async move{
		match ctx.bucket.put_multipart_chunk(buf,&session.s3_key,parms.part+1,&session.upload_id.unwrap(),&session.content_type.unwrap()).await{
			Ok(part)=>{
				//println!("ok {}",part.part_number);
				let mut w=ctx.part_etag.write().await;
				w.insert(temp_id,part.etag.clone());
			},
			Err(e)=>{
				eprintln!("{}:{} {:?}",file!(),line!(),e);
				//空文字列は失敗
				let mut w=ctx.part_etag.write().await;
				w.insert(temp_id,"".into());
			}
		}
	});
	(StatusCode::NO_CONTENT).into_response()
}