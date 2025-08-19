use axum::Router;

use crate::Context;

mod list;
mod upload;
mod show;
mod delete;
mod meta;
mod mv;
mod mkdir;

pub fn route(ctx: &Context,app: Router)->Router{
	let app=upload::route(ctx, app);
	let ctx0=ctx.clone();
	let app=app.route("/api/files/list",axum::routing::post(move|authorization,cookie,body|list::post(ctx0.clone(),authorization,cookie,body)));
	let ctx0=ctx.clone();
	let app=app.route("/api/files/meta",axum::routing::post(move|authorization,cookie,body|meta::post(ctx0.clone(),authorization,cookie,body)));
	let ctx0=ctx.clone();
	let app=app.route("/api/files/delete",axum::routing::post(move|authorization,cookie,body|delete::post(ctx0.clone(),authorization,cookie,body)));
	let ctx0=ctx.clone();
	let app=app.route("/api/files/mv",axum::routing::post(move|authorization,cookie,body|mv::post(ctx0.clone(),authorization,cookie,body)));
	let ctx0=ctx.clone();
	let app=app.route("/api/files/mkdir",axum::routing::post(move|authorization,cookie,body|mkdir::post(ctx0.clone(),authorization,cookie,body)));
	let ctx0=ctx.clone();
	let app=app.route("/api/files/show/:id",axum::routing::get(move|id,q,authorization,cookie,range|show::get(ctx0.clone(),id,authorization,cookie,range,q)));
	app
}
