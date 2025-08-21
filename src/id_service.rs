use std::sync::Mutex;

use rand::{rngs::StdRng, SeedableRng};

// Crockford's Base32
// https://github.com/ulid/spec#encoding
const CHARS:&'static str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Debug)]
pub struct UlidService{
	rng:Mutex<StdRng>,
}
impl UlidService{
	pub fn gen_now(&self)->String {
		self.gen(chrono::Utc::now().timestamp_millis())
	}
	pub fn gen(&self,time: i64)->String {
		let mut rng=self.rng.lock().unwrap();
		let datetime=chrono::DateTime::from_timestamp_millis(time).unwrap();
		let rng:&mut StdRng=&mut rng;
		let id=ulid::Ulid::from_datetime_with_source(datetime.into(),rng);
		id.to_string()
	}
	pub fn parse(&self,id: &str)->Option<i64> {
		let timestamp = &id[0..10];
		let mut time = 0;
		for c in timestamp.chars().into_iter(){
			time = time * 32 + CHARS.find(c)? as i64;
		}
		Some(time)
	}
	pub fn new()->Self{
		Self{
			rng:Mutex::new(StdRng::from_os_rng()),
		}
	}
}
