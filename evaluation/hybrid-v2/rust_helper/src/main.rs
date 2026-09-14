use std::io::{self, BufRead};
use yaktool::interpret::parse_move_frame;
fn main() {
 for line in io::stdin().lock().lines() {
  let request: String = serde_json::from_str::<serde_json::Value>(&line.unwrap()).unwrap()["request"].as_str().unwrap().to_owned();
  let out = parse_move_frame(&request).unwrap();
  println!("{}", serde_json::to_string(&out).unwrap());
 }
}
