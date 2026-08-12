#![allow(dead_code)]

pub fn complete(mut value: i32, enabled: bool) -> i32 {
    let mapped = simple_target(value);
    if enabled {
        value = mapped;
    } else {
        value = value + 1;
    }
    value
}

pub fn partial_syntax(value: i32) -> i32 {
    let direct = simple_target(value);
    let mixed = consume(value, || value);
    let chained = factory("https://fixture.invalid").send(value);
    let opaque = client_factory!().send(value);
    let chosen = match value {
        0 => direct,
        _ => chained,
    };
    direct + mixed + chained + opaque + chosen
}

#[cfg(test)]
pub fn test_only(value: i32) -> i32 {
    simple_target(value)
}
