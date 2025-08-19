use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::{ExpressionMethods, QueryDsl, Selectable, SelectableHelper};
use diesel_async::{scoped_futures::ScopedFutureExt, AsyncConnection, RunQueryDsl};

use crate::{id_service, DBConnection, DataBase, QueryRange};

use super::user::User;

diesel::table! {
	#[sql_name = "file"]
	file (id) {
		id -> Int8,
		user_id -> Int8,
		directory -> VarChar,
		name -> VarChar,
		created_at -> Timestamp,
		updated_at -> Timestamp,
		sha256 -> Nullable<VarChar>,
		s3_key -> Nullable<VarChar>,
		metadata -> Nullable<VarChar>,
		thumbnail_key -> Nullable<VarChar>,
		blurhash -> Nullable<VarChar>,
		content_type -> VarChar,
		size -> Int8,
	}
}

#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = file)]
pub struct FileEntry{
	pub id:i64,
	pub user_id:i64,
	pub directory:String,
	pub name:String,
	pub created_at:NaiveDateTime,
	pub updated_at:NaiveDateTime,
	pub sha256:Option<String>,
	pub s3_key:Option<String>,//ディレクトリには無い
	pub metadata:Option<String>,//json
	pub thumbnail_key:Option<String>,
	pub blurhash:Option<String>,
	pub content_type: String,
	pub size: i64,
}
#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = file)]
pub struct NewFileEntry{
	pub user_id:i64,
	pub directory:String,
	pub name:String,
	pub created_at:NaiveDateTime,
	pub updated_at:NaiveDateTime,
	pub sha256:Option<String>,
	pub s3_key:Option<String>,//ディレクトリには無い
	pub metadata:Option<String>,//json
	pub thumbnail_key:Option<String>,
	pub blurhash:Option<String>,
	pub content_type: String,
	pub size: i64,
}
impl FileEntry{
	pub async fn load(db:&DataBase,uid:i64,d:Option<String>,range:impl Into<QueryRange<i64>>,limit:i64)->Result<Vec<Self>,String>{
		let range=Into::<QueryRange<i64>>::into(range);
		if range.is_empty(){
			return Err("empty range".to_owned());
		}
		let mut conn=db.get_or_err().await?;
		use file::dsl::file;
		use self::file::*;
		let e=match range{
			QueryRange::Limit(range) =>{
				let u=file.select(Self::as_select()).filter(user_id.eq(uid));
				if let Some(d)=d{
					u.filter(directory.eq(d)).filter(id.gt(range.start)).filter(id.lt(range.end)).order(id.asc()).limit(limit).get_results(&mut conn).await
				}else{
					u.filter(id.gt(range.start)).filter(id.lt(range.end)).order(id.asc()).limit(limit).get_results(&mut conn).await
				}
			},
			QueryRange::All => {
				let u=file.select(Self::as_select()).filter(user_id.eq(uid));
				if let Some(d)=d{
					u.filter(directory.eq(d)).order(id.asc()).limit(limit).get_results(&mut conn).await
				}else{
					u.order(id.asc()).limit(limit).get_results(&mut conn).await
				}
			},
		};
		e.map_err(|e|format!("{:?}",e))
	}
	pub async fn load_by_path(db:&DataBase,uid:i64,d:&str,n:&str)->Result<Self,String>{
		let mut conn=db.get_or_err().await?;
		use file::dsl::file;
		use self::file::*;
		let e=file.select(Self::as_select()).filter(user_id.eq(uid)).filter(directory.eq(d)).filter(name.eq(n)).first(&mut conn).await;
		e.map_err(|e|format!("{:?}",e))
	}
	pub async fn count_by_hash(db:&DataBase,uid:i64,hash:String)->Result<i64,String>{
		let mut conn=db.get_or_err().await?;
		use file::dsl::file;
		use self::file::*;
		let e=file.filter(sha256.eq(hash)).filter(user_id.eq(uid)).count().first(&mut conn).await;
		e.map_err(|e|format!("{:?}",e))
	}
	pub async fn first_by_hash(db:&DataBase,uid:i64,hash:String)->Result<Self,String>{
		let mut conn=db.get_or_err().await?;
		use file::dsl::file;
		use self::file::*;
		let e=file.select(Self::as_select()).filter(sha256.eq(hash)).filter(user_id.eq(uid)).first(&mut conn).await;
		e.map_err(|e|format!("{:?}",e))
	}
	pub async fn load_by_id(db:&DataBase,uid:i64,target_id:i64)->Result<Self,String>{
		let mut conn=db.get_or_err().await?;
		use file::dsl::file;
		use self::file::*;
		let e=file.select(Self::as_select()).filter(id.eq(target_id)).filter(user_id.eq(uid)).first(&mut conn).await;
		e.map_err(|e|format!("{:?}",e))
	}
	pub async fn update_path(db:&DataBase,uid:i64,target_id:i64,d:&String,n:&String)->Result<(),String>{
		let mut conn=db.get_or_err().await?;
		mkdirs(&mut conn,uid,&d).await?;
		use file::dsl::file;
		use self::file::*;
		let res=diesel::update(file.filter(id.eq(target_id))).set((
			directory.eq(d),
			name.eq(n),
		)).execute(&mut conn).await;
		let updated_rows=res.map_err(|e|format!("{:?}",e))?;
		if updated_rows!=1{
			Err(format!("inserted_rows {}",updated_rows))
		}else{
			Ok(())
		}
	}
	pub async fn delete(db:&DataBase,target_id:&i64)->Result<(),String>{
		use self::file::dsl::file;
		use self::file::*;
		let mut conn=db.get().await.ok_or_else(||"get connection".to_owned())?;
		let res=diesel::delete(file.filter(id.eq(target_id))).execute(&mut conn).await;
		let inserted_rows=res.map_err(|e|format!("{:?}",e))?;
		if inserted_rows!=1{
			Err(format!("inserted_rows {}",inserted_rows))
		}else{
			Ok(())
		}
	}
}
pub async fn mkdirs(conn:&mut DBConnection<'_>,uid:i64,directory:&String)->Result<(),String>{
	let mut sub_string=String::from("/");
	for p in directory.split("/"){
		if p.is_empty(){
			continue;
		}
		let p=format!("{}/",p);
		use self::file::dsl::file;
		use self::file::*;
		let e=file.filter(directory.eq(sub_string.clone())).filter(name.eq(p.to_owned())).filter(user_id.eq(uid)).count().first(conn).await;
		if e==Ok(1){
		}else{
			mkdir(conn,uid,sub_string.clone(),p.to_owned()).await?;
		}
		sub_string+=&p;
	}
	Ok(())
}
async fn mkdir(conn:&mut DBConnection<'_>,user_id:i64,d:String,name:String)->Result<(),String>{
	let ent=NewFileEntry{
		user_id,
		directory: d,
		name,
		created_at: chrono::Utc::now().naive_utc(),
		updated_at: chrono::Utc::now().naive_utc(),
		sha256: None,
		s3_key: None,
		metadata: None,
		thumbnail_key: None,
		blurhash: None,
		content_type: "application/x-directory".into(),
		size: 0,
	};
	{
		use self::file::dsl::file;
		use self::file::*;
		let res=diesel::insert_into(file).values(ent).execute(conn).await;
		let inserted_rows=res.map_err(|e|format!("{:?}",e))?;
		if inserted_rows!=1{
			Err(format!("inserted_rows {}",inserted_rows))
		}else{
			Ok(())
		}
	}
}
impl NewFileEntry{
	pub async fn new(&self,db:&DataBase)->Result<(),String>{
		let mut conn=db.get().await.ok_or_else(||"get connection".to_owned())?;
		mkdirs(&mut conn,self.user_id,&self.directory).await?;
		use self::file::dsl::file;
		use self::file::*;
		let e:Result<i64, _>=file.filter(user_id.eq(self.user_id)).filter(directory.eq(&self.directory)).filter(name.eq(&self.name)).count().first(&mut conn).await;
		let inserted_rows=if e.unwrap_or(0)<1{
			diesel::insert_into(file).values(self).execute(&mut conn).await.map_err(|e|format!("{:?}",e))?
		}else{
			return Err("target path exists".into())
		};
		if inserted_rows!=1{
			Err(format!("inserted_rows {}",inserted_rows))
		}else{
			Ok(())
		}
	}
}
pub async fn migrate(time:i64,conn:&mut DBConnection<'_>){
	if time>1729446621257{
		return;
	}
	let sql=r###"
	create table file
	(
		id BIGSERIAL UNIQUE,
		user_id BIGSERIAL NOT NULL,
		directory TEXT NOT NULL,
		name TEXT NOT NULL,
		created_at TIMESTAMP WITH TIME ZONE NOT NULL,
		updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
		sha256 TEXT,
		s3_key TEXT,
		metadata TEXT,
		thumbnail_key TEXT,
		blurhash TEXT,
		content_type TEXT NOT NULL,
		size BIGSERIAL NOT NULL
	);
	"###;
	println!("{}",sql);
	diesel::sql_query(sql).execute(conn).await.expect("migrate file table");
}
