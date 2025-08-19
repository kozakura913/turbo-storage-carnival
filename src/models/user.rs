use diesel_async::AsyncPgConnection;

use diesel::{ExpressionMethods, QueryDsl, Selectable};
use diesel_async::{RunQueryDsl, AsyncConnection};
use diesel::SelectableHelper;

use crate::{DBConnection, DataBase, QueryRange};

diesel::table! {
	#[sql_name = "users"]
	users (user_id) {
		user_id -> Int8,
		username -> VarChar,
		password -> VarChar,
		last_login -> Int8,
	}
}
#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = users)]
pub struct User{
	pub user_id:i64,//削除されてもそのidに再割り当てされる事はない
	pub username:String,
	pub password:String,
	pub last_login:i64,
}
#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = users)]
struct NewUser{
	pub username:String,
	pub password:String,
	pub last_login:i64,
}
#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = users)]
struct DeleteUser{
	pub user_id:i64,
}
impl User{
	pub async fn load_by_id(db:&DataBase,p:&i64)->Option<Self>{
		let mut conn=db.get().await?;
		use self::users::dsl::users;
		use self::users::*;
		let e=users.filter(user_id.eq(p)).select(Self::as_select()).first(&mut conn).await.ok()?;
		Some(e)
	}
	pub async fn load_by_username(db:&DataBase,p:&String)->Option<Self>{
		let mut conn=db.get().await?;
		use self::users::dsl::users;
		use self::users::*;
		let e=users.filter(username.eq(p)).select(Self::as_select()).first(&mut conn).await.ok()?;
		Some(e)
	}
	pub async fn load_all(db:&DataBase,range:impl Into<QueryRange<i64>>)->Option<Vec<Self>>{
		let range=Into::<QueryRange<i64>>::into(range);
		if range.is_empty(){
			return None;
		}
		let mut conn=db.get().await?;
		use users::dsl::users;
		use self::users::*;
		let e=match range{
			QueryRange::Limit(range) => users.select(Self::as_select()).order(last_login.desc()).offset(range.start).limit(range.end-range.start).get_results(&mut conn).await,
			QueryRange::All => users.select(Self::as_select()).get_results(&mut conn).await,
		};
		e.ok()
	}
	pub async fn reset_password(&self,db:&DataBase,new_password:&str)->Result<(),String>{
		let mut conn=db.get_or_err().await?;
		let new_password=password_hash(new_password)?;
		use users::dsl::users;
		use self::users::*;
		let res=diesel::update(users.filter(user_id.eq(&self.user_id))).set(password.eq(new_password)).execute(&mut conn).await;
		let updated_rows=res.map_err(|e|format!("{:?}",e))?;
		if updated_rows!=1{
			Err(format!("inserted_rows {}",updated_rows))
		}else{
			Ok(())
		}
	}
	pub fn verify(&self,password:&str)->Option<bool>{
		use argon2::{Argon2, PasswordHash, PasswordVerifier};
		let password_hash = PasswordHash::new(self.password.as_str()).ok()?;
		Some(Argon2::default().verify_password(password.as_bytes(), &password_hash).is_ok())
		//bcrypt::verify(password, &self.password).ok()
	}
}
fn password_hash(new_password:&str)->Result<String,String>{
	//let new_password=bcrypt::hash(new_password,16).map_err(|e|e.to_string())?;//bcryptは廃止した
	use argon2::password_hash::SaltString;
	use argon2::{Argon2, PasswordHasher, Algorithm, Version, Params};
	let salt = SaltString::generate(&mut rand::thread_rng());
	let new_password = Argon2::new(
		Algorithm::Argon2id,
		Version::V0x13,
		Params::new(256*1024,2,16, None).unwrap(),
	).hash_password(new_password.as_bytes(), &salt).map_err(|e|e.to_string())?.to_string();
	Ok(new_password)
}
impl NewUser{
	pub async fn insert(&self,db:&DataBase)->Result<i64,String>{
		use self::users::dsl::users;
		use self::users::*;
		let mut conn=db.get().await.ok_or_else(||"get connection".to_owned())?;
		let res=diesel::insert_into(users).values(self).returning(user_id).get_results(&mut conn).await;
		let inserted_rows=res.map_err(|e|format!("{:?}",e))?;
		if inserted_rows.len()!=1{
			Err(format!("inserted_rows {}",inserted_rows.len()))
		}else{
			Ok(inserted_rows.get(0).copied().unwrap())
		}
	}
}
impl DeleteUser{
	pub async fn delete(&self,db:&DataBase)->Result<(),String>{
		use self::users::dsl::users;
		use self::users::*;
		let mut conn=db.get().await.ok_or_else(||"get connection".to_owned())?;
		let res=diesel::delete(
			users.filter(user_id.eq(self.user_id))
		).execute(&mut conn).await;
		let deleted_rows=res.map_err(|e|format!("{:?}",e))?;
		if deleted_rows!=1{
			Err(format!("deleted_rows {}",deleted_rows))
		}else{
			Ok(())
		}
	}
}
pub async fn new(db:&DataBase,username:String,password:String)->Option<User>{
	let u=NewUser{
		username,
		password: password_hash(&password).ok()?,//ハッシュ化した後の値
		last_login:chrono::Utc::now().timestamp_millis(),
	};
	let id=u.insert(&db).await.unwrap();
	println!("alloc id {}",id);
	let u=User::load_by_id(&db,&id).await;
	u
}
pub async fn delete(db:&DataBase,id:i64,password:&str)->Option<bool>{
	let u=User::load_by_id(&db,&id).await?;
	if !u.verify(password).unwrap_or(false){
		return Some(false);
	}
	let del=DeleteUser{
		user_id:u.user_id,
	};
	del.delete(&db).await.ok()?;
	Some(true)
}

pub async fn migrate(time:i64,conn:&mut DBConnection<'_>){
	if time>1729446621257{
		return;
	}
	let sql=r###"
	create table users
	(
		user_id BIGSERIAL UNIQUE,
		username TEXT NOT NULL UNIQUE,
		password TEXT NOT NULL,
		last_login BIGINT NOT NULL
	);
	"###;
	println!("{}",sql);
	diesel::sql_query(sql).execute(conn).await.expect("migrate users table");
}
