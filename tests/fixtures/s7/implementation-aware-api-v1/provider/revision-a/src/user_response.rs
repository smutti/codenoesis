use serde_json::{Map, Value};

pub fn user_response(id: &str, nickname: &str, user: &User) -> Value {
    let mut body = Map::new();
    body.insert("id".into(), Value::String(id.into()));
    body.insert("nickname".into(), Value::String(nickname.into()));
    body.extend(custom_profile_fields(user));
    Value::Object(body)
}
