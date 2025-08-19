
use chrono::NaiveDateTime;
use diesel::Selectable;
use diesel_async::RunQueryDsl;

use crate::{DBConnection, DataBase, QueryRange};

diesel::table! {
	#[sql_name = "collection"]
	collection (id) {
		id -> VarChar,
		user_id -> Int8,
		title -> Nullable<VarChar>,
		updated_at -> Timestamp,
		path -> VarChar,
	}
}

#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = collection)]
pub struct Collection{
	pub id:String,
	pub user_id:i64,
	pub title:Option<String>,
	pub updated_at:NaiveDateTime,
	pub path:String,
}
diesel::table! {
	#[sql_name = "collection_file"]
	collection_file (id) {
		id -> VarChar,
		collection_id -> VarChar,
		file_id -> VarChar,
	}
}
#[derive(Debug,Clone,diesel::Insertable,diesel::Queryable,Selectable,diesel::QueryableByName)]
#[diesel(table_name = collection_file)]
pub struct CollectionFile{
	pub id:String,
	pub file_id:String,
	pub collection_id:String,
}

pub async fn migrate(time:i64,conn:&mut DBConnection<'_>){
	if time>1729446621257{
		return;
	}
	let sql=r###"
	create table collection
	(
		id TEXT NOT NULL UNIQUE,
		user_id BIGSERIAL UNIQUE,
		title TEXT,
		updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
		path TEXT NOT NULL
	);
	"###;
	println!("{}",sql);
	diesel::sql_query(sql).execute(conn).await.expect("migrate collection table");
	let sql=r###"
	create table collection_file
	(
		id TEXT NOT NULL UNIQUE,
		collection_id TEXT,
		file_id TEXT
	);
	"###;
	println!("{}",sql);
	diesel::sql_query(sql).execute(conn).await.expect("migrate collection_file table");
}
