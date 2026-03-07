#![forbid(unsafe_code)]

use serde_json::Value;

/// Abstraction for HTTP compatibility adapters.
pub trait HttpCompatAdapter {
    fn adapt_request(&self, input: &str) -> String;
    fn adapt_response(&self, input: &str) -> String;
}

/// Converts a `snake_case` string to `camelCase`.
pub fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Converts a `camelCase` string to `snake_case`.
pub fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(ch.to_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

fn transform_keys(value: &Value, f: fn(&str) -> String) -> Value {
    match value {
        Value::Object(map) => {
            let new_map = map
                .iter()
                .map(|(k, v)| (f(k), transform_keys(v, f)))
                .collect();
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(|v| transform_keys(v, f)).collect()),
        other => other.clone(),
    }
}

fn transform_json(input: &str, f: fn(&str) -> String) -> String {
    match serde_json::from_str::<Value>(input) {
        Ok(val) => serde_json::to_string(&transform_keys(&val, f)).unwrap_or_else(|_| input.to_string()),
        Err(_) => input.to_string(),
    }
}

/// Adapter that converts `snake_case` JSON keys to `camelCase`.
pub struct SnakeToCamelAdapter;

impl HttpCompatAdapter for SnakeToCamelAdapter {
    fn adapt_request(&self, input: &str) -> String {
        transform_json(input, snake_to_camel)
    }

    fn adapt_response(&self, input: &str) -> String {
        transform_json(input, snake_to_camel)
    }
}

/// Adapter that converts `camelCase` JSON keys to `snake_case`.
pub struct CamelToSnakeAdapter;

impl HttpCompatAdapter for CamelToSnakeAdapter {
    fn adapt_request(&self, input: &str) -> String {
        transform_json(input, camel_to_snake)
    }

    fn adapt_response(&self, input: &str) -> String {
        transform_json(input, camel_to_snake)
    }
}

/// Adapter that returns the input unchanged.
pub struct PassthroughAdapter;

impl HttpCompatAdapter for PassthroughAdapter {
    fn adapt_request(&self, input: &str) -> String {
        input.to_string()
    }

    fn adapt_response(&self, input: &str) -> String {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_to_camel() {
        assert_eq!(snake_to_camel("hello_world"), "helloWorld");
        assert_eq!(snake_to_camel("user_id"), "userId");
        assert_eq!(snake_to_camel("already"), "already");
    }

    #[test]
    fn test_camel_to_snake() {
        assert_eq!(camel_to_snake("helloWorld"), "hello_world");
        assert_eq!(camel_to_snake("userId"), "user_id");
        assert_eq!(camel_to_snake("already"), "already");
    }

    #[test]
    fn test_snake_to_camel_adapter_request() {
        let adapter = SnakeToCamelAdapter;
        let input = r#"{"user_name":"alice","account_id":42}"#;
        let output = adapter.adapt_request(input);
        let val: Value = serde_json::from_str(&output).unwrap();
        assert!(val.get("userName").is_some());
        assert!(val.get("accountId").is_some());
    }

    #[test]
    fn test_camel_to_snake_adapter_response() {
        let adapter = CamelToSnakeAdapter;
        let input = r#"{"userName":"alice","accountId":42}"#;
        let output = adapter.adapt_response(input);
        let val: Value = serde_json::from_str(&output).unwrap();
        assert!(val.get("user_name").is_some());
        assert!(val.get("account_id").is_some());
    }

    #[test]
    fn test_passthrough_adapter() {
        let adapter = PassthroughAdapter;
        let input = r#"{"any_key":"value"}"#;
        assert_eq!(adapter.adapt_request(input), input);
        assert_eq!(adapter.adapt_response(input), input);
    }

    #[test]
    fn test_non_json_input_passthrough() {
        let adapter = SnakeToCamelAdapter;
        let input = "not json at all";
        assert_eq!(adapter.adapt_request(input), input);
    }

    #[test]
    fn test_nested_key_conversion() {
        let adapter = SnakeToCamelAdapter;
        let input = r#"{"outer_key":{"inner_key":"value"}}"#;
        let output = adapter.adapt_request(input);
        let val: Value = serde_json::from_str(&output).unwrap();
        assert!(val.get("outerKey").is_some());
        let inner = val.get("outerKey").unwrap();
        assert!(inner.get("innerKey").is_some());
    }

    #[test]
    fn test_array_key_conversion() {
        let adapter = CamelToSnakeAdapter;
        let input = r#"[{"firstName":"a"},{"lastName":"b"}]"#;
        let output = adapter.adapt_request(input);
        let val: Value = serde_json::from_str(&output).unwrap();
        assert!(val[0].get("first_name").is_some());
        assert!(val[1].get("last_name").is_some());
    }
}
