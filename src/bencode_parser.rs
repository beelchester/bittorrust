const INT_PREFIX: &str = "i";
const LIST_PREFIX: &str = "l";
const DICT_PREFIX: &str = "d";
const SUFFIX: &str = "e";

pub fn decode(val: &str) -> (serde_json::Value, &str) {
    match val.split_at(1) {
        // integer
        (INT_PREFIX, rest) => {
            if let Some((int, rest)) = rest.split_once(SUFFIX) {
                if let Ok(val) = int.parse::<i64>() {
                    return (serde_json::Value::Number(val.into()), rest);
                }
            }
        }
        // list
        (LIST_PREFIX, rest) => {
            let (list, rest) = get_list_of_values(rest);
            return (serde_json::Value::Array(list), rest);
        }
        // dictionary
        (DICT_PREFIX, rest) => {
            if let Some(val) = rest.strip_suffix(SUFFIX) {
                let (list, rest) = get_list_of_values(val);
                let mut dict = serde_json::Map::new();
                list.iter().enumerate().for_each(|(i, value)| {
                    if i % 2 == 0 {
                        if let serde_json::Value::String(key) = value {
                            if list.len() > i + 1 {
                                dict.insert(key.to_string(), list[i + 1].clone());
                            } else {
                                panic!("Value not provided");
                            }
                        } else {
                            panic!("Key should be a string");
                        }
                    }
                });
                return (serde_json::Value::Object(dict), rest);
            }
        }
        (_, _) => {
            // byte string
            if let Some((len, rest)) = val.split_once(':') {
                if let Ok(len) = len.parse::<usize>() {
                    return (
                        serde_json::Value::String(rest[..len].to_string()),
                        &rest[len..],
                    );
                }
            }
        }
    }
    panic!("Unknown argument");
}

pub fn get_list_of_values(val: &str) -> (Vec<serde_json::Value>, &str) {
    let mut list = vec![];
    let mut rest = val;
    while !rest.is_empty() && !rest.starts_with('e') {
        let (decoded, r) = decode(rest);
        list.push(decoded.clone());
        rest = r;
    }
    if rest.starts_with('e') {
        rest = &rest[1..];
    }
    (list, rest)
}
