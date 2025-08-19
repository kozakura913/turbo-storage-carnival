use axum::Router;

use crate::Context;

mod preflight;
mod partial;
mod finish;

pub fn route(ctx: &Context,app: Router)->Router{
	let ctx0=ctx.clone();
	let app=app.route("/api/files/upload/preflight",axum::routing::post(move|authorization,cookie,body|preflight::post(ctx0.clone(),authorization,cookie,body)));
	let ctx0=ctx.clone();
	let app=app.route("/api/files/upload/partial",axum::routing::post(move|q,body|partial::post(ctx0.clone(),q,body)));
	let ctx0=ctx.clone();
	let app=app.route("/api/files/upload/finish",axum::routing::post(move|q|finish::post(ctx0.clone(),q)));
	app
}
