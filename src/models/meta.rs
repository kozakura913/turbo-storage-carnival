use chrono::NaiveDateTime;
use diesel_async::AsyncPgConnection;

use diesel::{ExpressionMethods, QueryDsl, Selectable};
use diesel_async::{RunQueryDsl, AsyncConnection};
use diesel::SelectableHelper;

use crate::{DataBase, QueryRange};

diesel::table! {
	#[sql_name = "meta"]
	meta (name) {
		name -> VarChar,
		value -> VarChar,
		updated_at -> Nullable<Timestamp>
	}
}

#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = meta)]
pub struct MetaEntry{
	pub name:String,
	pub value:String,
	pub updated_at:Option<NaiveDateTime>,
}
impl MetaEntry{
	pub async fn load_by_name(db:&DataBase,p:&str)->Option<Self>{
		let mut conn=db.get().await?;
		use self::meta::dsl::meta;
		use self::meta::*;
		let e=meta.filter(name.eq(p)).select(Self::as_select()).first(&mut conn).await.ok()?;
		Some(e)
	}
	pub async fn update(&self,db:&DataBase)->Result<(),String>{
		use self::meta::dsl::meta;
		use self::meta::*;
		let mut conn=db.get_or_err().await?;
		let res=diesel::update(meta.filter(name.eq(&self.name))).set((
			value.eq(self.value.clone()),
			updated_at.eq(chrono::Utc::now().naive_utc())
		)).execute(&mut conn).await;
		let inserted_rows=res.map_err(|e|format!("{:?}",e))?;
		if inserted_rows!=1{
			Err(format!("inserted_rows {}",inserted_rows))
		}else{
			Ok(())
		}
	}
	pub async fn insert(&self,db:&DataBase)->Result<(),String>{
		use self::meta::dsl::meta;
		let mut conn=db.get().await.ok_or_else(||"get connection".to_owned())?;
		let res=diesel::insert_into(meta).values(self).execute(&mut conn).await;
		let inserted_rows=res.map_err(|e|format!("{:?}",e))?;
		if inserted_rows!=1{
			Err(format!("inserted_rows {}",inserted_rows))
		}else{
			Ok(())
		}
	}
}
pub fn migrate()->&'static str{
	r###"
	create table meta
	(
		name TEXT NOT NULL UNIQUE,
		value TEXT NOT NULL,
		updated_at TIMESTAMP WITH TIME ZONE NOT NULL
	);
	"###
}
