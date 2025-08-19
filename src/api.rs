use std::io::Read;

use axum::Router;
mod files;
mod login;

use crate::Context;

pub fn route(ctx: &Context,app: Router)->Router{
	let app=files::route(&ctx,app);
	let ctx0=ctx.clone();
	let app=app.route("/api/login",axum::routing::post(move|body|login::post(ctx0.clone(),body)));
	/*
	let app=app.route("/",axum::routing::get(||async{
		let mut buf=String::new();
		let mut header=axum::http::HeaderMap::new();
		header.append("Content-Type","text/html; charset=UTF-8".parse().unwrap());
		std::fs::File::open("index.html").unwrap().read_to_string(&mut buf).unwrap();
		(axum::http::StatusCode::OK,header,buf)
	}));
	*/
	app
}
