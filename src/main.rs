use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{response::IntoResponse, Router};
use base64::Engine;
use chrono::{DateTime, Utc};
use diesel_async::AsyncPgConnection;

use diesel::{delete, ExpressionMethods, QueryDsl, Selectable};
use diesel_async::{RunQueryDsl, AsyncConnection};
use diesel::SelectableHelper;
use models::user::User;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio::sync::RwLock;

mod browsersafe;
mod id_service;
mod models;
mod api;

#[derive(Clone,Debug)]
pub struct DataBase(diesel_async::pooled_connection::bb8::Pool<AsyncPgConnection>);
impl DataBase{
	pub async fn open(database_url:&str)->Result<Self,String>{
		let config = diesel_async::pooled_connection::AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
		let pool = match diesel_async::pooled_connection::bb8::Pool::builder().build(config).await{
			Ok(p) => p,
			Err(e) => return Err(e.to_string()),
		};
		Ok(Self(pool))
	}
	pub async fn get_or_err(&self)->Result<DBConnection,String>{
		self.get().await.ok_or_else(||"get connection".to_owned())
	}
	pub async fn get(&self)->Option<DBConnection>{
		match self.0.get().await{
			Ok(c)=>Some(c),
			Err(e)=>{
				eprintln!("DB Error {:?}",e);
				None
			}
		}
	}
}
pub type DBConnection<'a>=diesel_async::pooled_connection::bb8::PooledConnection<'a, AsyncPgConnection>;
#[derive(Serialize, Deserialize,Debug)]
struct ConfigFile{
	db:String,
	bind_addr:String,
	s3:S3Config,
	part_max_size:i32,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct S3Config{
	endpoint: String,
	bucket: String,
	region: String,
	access_key: String,
	secret_key: String,
	timeout:u64,
	path_style:bool,
}
#[derive(Clone,Debug)]
pub struct Context{
	config:Arc<ConfigFile>,
	db:DataBase,
	id_service:Arc<id_service::UlidService>,
	bucket:Arc<Box<Bucket>>,
	upload_session:Arc<RwLock<HashMap<String,UploadSession>>>,
	login_session:Arc<RwLock<HashMap<String,Session>>>,
	part_etag:Arc<RwLock<HashMap<String,String>>>,
}
impl Context{
	pub async fn session(&self,
		authorization:Option<&axum_extra::TypedHeader<axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>>>,
		cookie:Option<&axum_extra::TypedHeader<axum_extra::headers::Cookie>>,
	)->Option<Session>{
		if let Some(sid)=authorization{
			if let Some(s)=self.active_session(sid.token()).await{
				return Some(s);
			}
		}
		let cookie=cookie?;
		let sid=cookie.get("SID")?;
		self.active_session(sid).await
	}
	pub async fn active_session(&self,sid:&str)->Option<Session>{
		let rl=self.login_session.read().await;
		let s=rl.get(sid)?;
		if s.expires_at>Utc::now(){
			Some(s.clone())
		}else{
			drop(rl);
			let mut wl=self.login_session.write().await;
			wl.remove(sid);
			None
		}
	}
	pub async fn login(&self,user:User)->String{
		let mut wl=self.login_session.write().await;
		{
			//期限切れセッションを一掃する
			let now=Utc::now();
			let mut rem=vec![];
			for (k,v) in wl.iter(){
				if v.expires_at<now{
					rem.push(k.clone());
				}
			}
			for k in rem{
				wl.remove(&k);
			}
		}
		let session_id=base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8;32]>());
		let login_at=Utc::now();
		wl.insert(session_id.clone(),Session{
			user_id:user.user_id,
			expires_at:login_at+chrono::Duration::hours(12),//セッション有効期限12時間
			login_at,
		});
		session_id
	}
	pub async fn write_upload_session(&self,session: UploadSession,id:String){
		self.upload_session.write().await.insert(id,session);
	}
	pub async fn upload_session(&self,authorization: Option<&axum::http::HeaderValue>,del:bool)->Result<(UploadSession,String),axum::response::Response>{
		let session=match authorization.map(|v|v.to_str().map(|s|{
			if s.starts_with("Bearer "){
				Some(&s["Bearer ".len()..])
			}else{
				None
			}
		})){
			Some(Ok(Some(session_id)))=>{
				let res=if del{
					self.upload_session.write().await.remove(session_id)
				}else{
					self.upload_session.read().await.get(session_id).cloned()
				};
				match res{
					Some(s)=>Ok((s,session_id.to_owned())),
					_=>{
						return Err((axum::http::StatusCode::FORBIDDEN).into_response())
					},
				}
			},
			e=>{
				eprintln!("{}:{} {:?}",file!(),line!(),e);
				return Err((axum::http::StatusCode::BAD_REQUEST).into_response())
			}
		};
		session
	}
}
#[derive(Clone,Debug)]
pub struct Session{
	user_id:i64,
	login_at:DateTime<Utc>,
	expires_at:DateTime<Utc>,
}
#[derive(Clone,Debug,Serialize, Deserialize)]
pub struct UploadSession{
	hasher:Hasher,
	content_type:Option<String>,
	ext:Option<String>,
	s3_key:String,
	content_length:u64,
	directory:String,
	title:Option<String>,
	upload_id:Option<String>,
	part_number:Option<u32>,
	user_id:i64,
	last_modified: Option<String>,
	part_etag:Vec<String>,
}
#[derive(Debug,Clone)]
pub struct Hasher(sha2::Sha256);
impl Serialize for Hasher{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer {
		let s:String=self.clone().into();
		s.serialize(serializer)
	}
}
impl Into<String> for Hasher{
	fn into(self)->String{
		let ptr=Box::leak(Box::new(self.0));
		let s=unsafe{
			std::slice::from_raw_parts(ptr as *const _ as *const u8, std::mem::size_of::<sha2::Sha256>())
		};
		use base64::Engine;
		let s=base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s);
		unsafe{
			let _=Box::from_raw(ptr);
		}
		s
	}
}
impl <'de> Deserialize<'de> for Hasher{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de> {
		let s=String::deserialize(deserializer)?;
		Ok(Hasher::from(s))
	}
}
impl <T> From<T> for Hasher where T:AsRef<str>{
	fn from(value: T) -> Self {
		use base64::Engine;
		let raw=base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(value.as_ref()).unwrap();
		let s = unsafe {
			Box::from_raw(raw.leak() as * mut _ as *mut sha2::Sha256)
		};
		Self(*s)
	}
}
impl Hasher{
	pub fn new()->Self{
		use sha2::{Sha256, Digest};
		let hasher = Sha256::new();
		Self(hasher)
	}
	pub fn update(&mut self,b:&[u8]){
		use sha2::Digest;
		self.0.update(b);
	}
	pub fn finalize(&self)->Vec<u8>{
		self.0.clone().finalize().to_vec()
	}
}
pub enum QueryRange<T>{
	Limit(std::ops::Range<T>),
	All,
}
impl <T: PartialOrd> QueryRange<T>{
	pub fn is_empty(&self)->bool{
		match self{
			Self::Limit(v)=>v.is_empty(),
			Self::All=>false,
		}
	}
}
impl <T> From<std::ops::Range<T>> for QueryRange<T>{
	fn from(value: std::ops::Range<T>) -> Self {
		Self::Limit(value)
	}
}
impl <T> From<std::ops::RangeFull> for QueryRange<T>{
	fn from(_: std::ops::RangeFull) -> Self {
		Self::All
	}
}
fn main() {
	let mut args=std::env::args();
	let _self_path=args.next();
	let subcommand=args.next();
	let config:ConfigFile=serde_json::from_reader(std::fs::File::open("config.json").unwrap()).unwrap();
	let rt=tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
	rt.block_on(async{
		let db=DataBase::open(&config.db).await.expect("DB error");
		let bucket = s3::Bucket::new(
			&config.s3.bucket,
			s3::Region::Custom {
				region: config.s3.region.to_owned(),
				endpoint: config.s3.endpoint.to_owned(),
			},
			s3::creds::Credentials::new(Some(&config.s3.access_key),Some(&config.s3.secret_key),None,None,None).unwrap(),
		).unwrap();
		let bucket=if config.s3.path_style{
			bucket.with_path_style()
		}else{
			bucket
		};
		match subcommand.as_ref().map(|s|s.as_str()){
			Some("user")=>{
				let subcommand=args.next();
				match subcommand.as_ref().map(|s|s.as_str()){
					Some("add")=>{
						migrate(&db,&bucket).await;
						let username=args.next();
						let username=username.as_ref().expect("required username");
						println!("user add {}",username);
						let pass=base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8;32]>());
						let u=models::user::new(&db,username.into(),pass.clone()).await;
						println!("{:?}",u);
						if u.is_some(){
							println!("{}:{}",username,pass);
						}
					},
					Some("reset")=>{
						migrate(&db,&bucket).await;
						let username=args.next();
						let username=username.as_ref().expect("required username");
						let u=User::load_by_username(&db,username).await.expect("not found user");
						let pass=base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8;32]>());
						println!("{:?}",u.reset_password(&db,&pass).await);
						println!("{:?}",u);
						println!("{}:{}",username,pass);
					},
					Some("del")=>{
						migrate(&db,&bucket).await;
						let username=args.next();
						let username=username.as_ref().expect("required username");
						let password=args.next();
						let password=password.as_ref().expect("required password");
						let u=User::load_by_username(&db,username).await.expect("not found user");
						models::user::delete(&db, u.user_id, password).await;
					},
					Some("ls")=>{
						migrate(&db,&bucket).await;
						let mut offset=0;
						while let Some(users) = User::load_all(&db,offset..offset+100).await {
							if users.is_empty(){
								break;
							}
							for u in users.iter(){
								println!("{}:{}",u.user_id,u.username);
							}
							offset+=users.len() as i64;
						}
					},
					c=>{
						eprintln!("unknown command {:?}",c);
						std::process::exit(1);
					}
				}
			},
			Some("run")=>{
				migrate(&db,&bucket).await;
				let ctx=Context{
					config:Arc::new(config),
					id_service:Arc::new(id_service::UlidService::new()),
					db,
					bucket:Arc::new(bucket),
					upload_session:Arc::new(RwLock::new(HashMap::new())),
					login_session:Arc::new(RwLock::new(HashMap::new())),
					part_etag:Arc::new(RwLock::new(HashMap::new())),
				};
				server(ctx).await;
			},
			Some("drop")=>{
				let mut conn=db.get().await.expect("DB Connect Error");
				println!("{:?}",diesel::sql_query("drop table meta").execute(&mut conn).await);
				println!("{:?}",diesel::sql_query("drop table file").execute(&mut conn).await);
				println!("{:?}",diesel::sql_query("drop table users").execute(&mut conn).await);
				println!("{:?}",diesel::sql_query("drop table collection").execute(&mut conn).await);
				println!("{:?}",diesel::sql_query("drop table collection_file").execute(&mut conn).await);
				match bucket.list_multiparts_uploads(None,None).await{
					Ok(list)=>{
						for f in list{
							for u in f.uploads{
								println!("{:?}",u);
								println!("{:?}",bucket.abort_upload(&u.key,&u.id).await);
							}
						}
					},
					Err(e)=>{
						eprintln!("{:?}",e);
					}
				}
			}
			c=>{
				eprintln!("unknown command {:?}",c);
				std::process::exit(1);
			}
		}
	});
}
async fn migrate(db:&DataBase,bucket:&Bucket){
	let ver=models::meta::MetaEntry::load_by_name(&db,"db_version").await;
	let is_init=ver.is_none();
	let db_rev=ver.map(|v|i64::from_str_radix(v.value.as_str(),10).unwrap_or(0)).unwrap_or(0);
	let now=chrono::DateTime::from_timestamp_millis(1729448015982).unwrap().to_utc();
	if db_rev<now.timestamp_millis(){
		let mut conn=db.get().await.expect("DB Connect Error");
		models::file::migrate(db_rev,&mut conn).await;
		models::user::migrate(db_rev,&mut conn).await;
		models::collection::migrate(db_rev,&mut conn).await;
		let meta=models::meta::MetaEntry{
			name:"db_version".into(),
			value:now.timestamp_millis().to_string(),
			updated_at:Some(now.naive_utc()),
		};
		if is_init{
			diesel::sql_query(models::meta::migrate()).execute(&mut conn).await.expect("migrate meta table");
			meta.insert(&db).await.expect("db init error");
			println!("init db");
		}else{
			meta.update(&db).await.expect("db init error");
			println!("update db");
		}
	}else{
		println!("skip db migrate");
	}
	match bucket.list_multiparts_uploads(None,None).await{
		Ok(list)=>{
			for f in list{
				for u in f.uploads{
					println!("{:?}",u);
					println!("{:?}",bucket.abort_upload(&u.key,&u.id).await);
				}
			}
		},
		Err(e)=>{
			eprintln!("{:?}",e);
		}
	}
}
async fn server(ctx:Context){
	let http_addr:SocketAddr = ctx.config.bind_addr.parse().unwrap();
	let app = Router::new();
	let app=api::route(&ctx,app);
	let service=axum::routing::get_service(tower_http::services::ServeDir::new("frontend"));
	let app=app.route("/", service);
	let service=axum::routing::get_service(tower_http::services::ServeDir::new("frontend"));
	let app=app.route("/*path", service);
	let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
	println!("server loaded");
	axum::serve(listener,app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown_signal()).await.unwrap();
}

async fn shutdown_signal() {
	use tokio::signal;
	use futures::{future::FutureExt,pin_mut};
	let ctrl_c = async {
		signal::ctrl_c()
			.await
			.expect("failed to install Ctrl+C handler");
	}.fuse();

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(signal::unix::SignalKind::terminate())
			.expect("failed to install signal handler")
			.recv()
			.await;
	}.fuse();
	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>().fuse();
	pin_mut!(ctrl_c, terminate);
	futures::select!{
		_ = ctrl_c => {},
		_ = terminate => {},
	}
}
