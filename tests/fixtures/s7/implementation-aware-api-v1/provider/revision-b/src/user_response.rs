use serde_json::{Map, Value};

pub fn user_response(id: &str, nickname: &str, private_account: bool, user: &User) -> Value {
    let mut body = Map::new();
    body.insert("id".into(), Value::String(id.into()));
    if !private_account {
        body.insert("nickname".into(), Value::String(nickname.into()));
    }
    body.extend(custom_profile_fields(user));
    Value::Object(body)
}
