use std::str::FromStr;

use crate::{models::{self, file::{FileEntry, NewFileEntry}}, Context, UploadSession};
use axum::{http::StatusCode, response::IntoResponse};
use image::{DynamicImage, GenericImageView};
use tokio::io::AsyncReadExt;

pub async fn post(
	ctx:Context,
	headers:axum::http::HeaderMap,
)->axum::response::Response{
	let authorization=headers.get("Authorization").cloned();
	let (mut session,_session_id)=match ctx.upload_session(authorization.as_ref(),false).await{
		Ok(v)=>v,
		Err(e)=>return e,
	};
	let user=models::user::User::load_by_id(&ctx.db,&session.user_id).await;
	if user.is_none(){
		return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
	}
	async fn err_handle(ctx: &Context,session: &UploadSession)->axum::response::Response{
		if let Some(upload_id)=session.upload_id.as_ref(){
			let _=ctx.bucket.abort_upload(&session.s3_key,upload_id).await;
		}
		(StatusCode::INTERNAL_SERVER_ERROR).into_response()
	}
	{
		let mut error_count=0;
		'outer: loop{
			let r_lock=ctx.part_etag.read().await;
			for etag in session.part_etag.iter(){
				let etag=r_lock.get(etag);
				if etag.is_none(){
					error_count+=1;
					if error_count>60{//1分間毎秒確認
						eprintln!("{}:{}",file!(),line!());
						return err_handle(&ctx,&session).await;
					}else{
						println!("{}",session.part_number.unwrap_or_default());
						tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
						continue 'outer;
					}
				}else{
					if etag.unwrap().is_empty(){
						return err_handle(&ctx,&session).await;
					}
				}
			}
			break;
		}
	}
	let mut parts=vec![];
	let mut part_number=1;
	{
		let r_lock=ctx.part_etag.read().await;
		for etag in session.part_etag.iter(){
			parts.push(s3::serde_types::Part{
				part_number,
				etag:r_lock.get(etag).unwrap().to_owned(),
			});
			part_number+=1;
		}
	}
	if let Some(n)=session.part_number{
		if part_number!=n+2{
			eprintln!("{}:{} {}!={}",file!(),line!(),part_number,n+2);
			return (StatusCode::BAD_REQUEST).into_response();
		}
	}else{
		eprintln!("{}:{}",file!(),line!());
		return (StatusCode::BAD_REQUEST).into_response();
	}
	let sha256sum=session.hasher.finalize();
	let sha256sum = sha256sum.iter().map(|n| format!("{:02x}", n)).collect::<String>();
	let cache_control="max-age=31536000, immutable";
	let title=session.title.as_ref().map(|s|s.as_str()).unwrap_or_else(||"no title");
	let detected_name=percent_encoding::percent_encode(title.as_bytes(), percent_encoding::NON_ALPHANUMERIC);
	let content_disposition=format!("inline; filename=\"{}\"",detected_name);
	if session.upload_id.is_none(){
		return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
	}
	let mut skip_data_body=None;
	if let Ok(e)=FileEntry::first_by_hash(&ctx.db,session.user_id,sha256sum.clone()).await{
		skip_data_body=Some(e);
	}
	let (mut metadata,mut thumbnail_key,mut blurhash)=(None,None,None);
	if let Some(e)=skip_data_body{
		let _=ctx.bucket.abort_upload(&session.s3_key,session.upload_id.as_ref().unwrap()).await;
		metadata=e.metadata;
		thumbnail_key=e.thumbnail_key;
		blurhash=e.blurhash;
		session.s3_key=e.s3_key.unwrap();
	}else{
		match ctx.bucket.complete_multipart_upload_with_metadata(&session.s3_key,&session.upload_id.unwrap(),parts,Some(&cache_control),Some(&content_disposition)).await{
			Ok(_resp) => {},
			Err(e) =>{
				println!("{:?} \n{}",session.part_etag,session.content_length);
				eprintln!("{}:{} {:?}",file!(),line!(),e);
				return (axum::http::StatusCode::INTERNAL_SERVER_ERROR).into_response();
			},
		}
		let content_type=session.content_type.as_ref().map(|s|s.as_str()).unwrap_or_default();
		if content_type.starts_with("image/")||content_type.starts_with("video/"){
			println!("ffmpeg");
			(metadata,thumbnail_key,blurhash)=ffmpeg_metadata(&ctx,&session.s3_key).await;
		}
	}
	let now=chrono::Utc::now();
	let basename=session.title.unwrap_or_else(||"no_title".into());
	let mut file_name=basename.clone();
	for file_number in 0..100{
		let f=crate::models::file::FileEntry::load_by_path(&ctx.db,session.user_id, &session.directory,&file_name).await;
		if let Ok(_f)=f{
			file_name=format!("{}({})",basename,file_number+1);
			continue;
		}else{
			break;
		}
	}
	let last_modified=session.last_modified.as_ref().map(|s|chrono::DateTime::parse_from_rfc3339(s).map(|c|c.to_utc()).ok()).unwrap_or_default();
	let e=NewFileEntry{
		user_id:session.user_id,
		name:file_name,
		directory:session.directory.clone(),
		created_at:now.naive_utc(),
		updated_at:last_modified.unwrap_or(now).naive_utc(),
		sha256:Some(sha256sum),
		s3_key:Some(session.s3_key),
		metadata,
		thumbnail_key,
		blurhash,
		content_type:session.content_type.unwrap_or_else(||"application/octet-stream".into()),
		size:session.content_length as i64,
	};
	let file=e.new(&ctx.db).await;
	if let Err(e)=file{
		eprintln!("{}",e);
		return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
	}
	return (StatusCode::OK).into_response();
}
pub async fn ffmpeg_metadata(ctx:&Context,access_key:&String)->(Option<String>,Option<String>,Option<String>){
	let url=if ctx.config.s3.path_style{
		format!("{}/{}/{}",ctx.config.s3.endpoint,ctx.config.s3.bucket,access_key)
	}else{
		let url=axum::http::Uri::from_str(ctx.config.s3.endpoint.as_str()).expect("bad s3 url");
		let scheme=url.scheme_str().unwrap();
		let port=match url.port_u16(){
			Some(p)=>p,
			None=>{
				if scheme=="http"{
					80
				}else{
					443
				}
			}
		};
		format!("{scheme}://{}.{}:{port}/{}",ctx.config.s3.bucket,url.host().unwrap(),access_key)
	};
	println!("url:{}",url);
	if let Ok(mut process)=tokio::process::Command::new("ffmpeg").stdout(std::process::Stdio::piped()).args(["-loglevel","quiet","-i",url.as_str(),"-frames:v","1","-f","image2pipe","-"]).spawn(){
		if let Some(mut stdout)=process.stdout.take(){
			let mut img=vec![];
			if let Err(e)=stdout.read_to_end(&mut img).await{
				println!("{:?}",e);
			}else{
				if let Ok(img)=image::load_from_memory(&img){
					let size=img.dimensions();
					println!("load image {}x{}",size.0,size.1);
					let (thumbnail_content,blurhash)=metadata(img).await;
					let _=process.start_kill();
					let mut thumbnail_key=None;
					if let Some(content)=thumbnail_content.as_ref(){
						let key=format!("{}-thumbnail.webp",access_key);
						if let Ok(_)=ctx.bucket.put_object_with_content_type(&key,content,"image/webp").await{
							thumbnail_key=Some(key);
						}
					}
					let mut map=serde_json::Map::new();
					map.insert("width".into(),size.0.into());
					map.insert("height".into(),size.1.into());
					return (serde_json::to_string(&map).ok(),thumbnail_key,blurhash);
				}
			}
		}
		let _=process.wait().await;
	}
	println!("ffmpeg error");
	return (None,None,None);
}
pub async fn metadata(img:DynamicImage)->(Option<Vec<u8>>,Option<String>){
	let thumbnail_quality=75f32;
	let thumbnail_size=512;
	let filter=fast_image_resize::FilterType::Bilinear;
	let (rgba,cp) = tokio::task::spawn_blocking(move||{
		let cp=img.clone();
		let rgba=resize(img, 224, 224, fast_image_resize::FilterType::Bilinear);
		(rgba,cp)
	}).await.unwrap_or_default();
	if rgba.is_none(){
		return (None,None);
	}
	let rgba=rgba.unwrap();
	let (blurhash,thumbnail)=futures_util::join!(tokio::task::spawn_blocking(move||{
		//misskeyでは5,5で生成してたから変えない
		blurhash::encode(5,5,rgba.width(),rgba.height(),rgba.as_raw()).ok()
	}),tokio::task::spawn_blocking(move||{
		let size=cp.dimensions();
		let rgba=resize(cp, thumbnail_size.min(size.0), thumbnail_size.min(size.1), filter)?;
		let size=rgba.dimensions();
		let binding = rgba.into_raw();
		let encoder=webp::Encoder::from_rgba(&binding,size.0,size.1);
		let mem=encoder.encode_simple(false,thumbnail_quality).ok()?;
		Some(mem.to_vec())
	}));
	(thumbnail.ok().unwrap_or_default(),blurhash.ok().unwrap_or_default())
}
pub fn resize(img:DynamicImage,max_width:u32,max_height:u32,filter:fast_image_resize::FilterType)->Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>>{
	let scale = f32::min(max_width as f32 / img.width() as f32,max_height as f32 / img.height() as f32);
	let dst_width=1.max((img.width() as f32 * scale).round() as u32);
	let dst_height=1.max((img.height() as f32 * scale).round() as u32);
	use std::num::NonZeroU32;
	let width=NonZeroU32::new(img.width())?;
	let height=NonZeroU32::new(img.height())?;
	let src_image=fast_image_resize::Image::from_vec_u8(width,height,img.into_rgba8().into_raw(),fast_image_resize::PixelType::U8x4);
	let mut src_image=src_image.ok()?;
	let alpha_mul_div=fast_image_resize::MulDiv::default();
	alpha_mul_div.multiply_alpha_inplace(&mut src_image.view_mut()).ok()?;
	let dst_width=NonZeroU32::new(dst_width)?;
	let dst_height=NonZeroU32::new(dst_height)?;
	let mut dst_image = fast_image_resize::Image::new(dst_width,dst_height,src_image.pixel_type());
	let mut dst_view = dst_image.view_mut();
	let mut resizer = fast_image_resize::Resizer::new(
		fast_image_resize::ResizeAlg::Convolution(filter),
	);
	resizer.resize(&src_image.view(), &mut dst_view).ok()?;
	alpha_mul_div.divide_alpha_inplace(&mut dst_view).ok()?;
	let rgba=image::RgbaImage::from_raw(dst_image.width().get(),dst_image.height().get(),dst_image.into_vec());
	rgba
}
