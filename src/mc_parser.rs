
use std::collections::HashMap;
use std::sync::OnceLock;
use regex::Regex;

fn get_macro_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\$\{([^}]+)\}").unwrap())
}

pub fn resolve_argument(arg: &str, vars: &HashMap<&str, String>) -> String {
    let re = get_macro_regex();
    re.replace_all(arg, |caps: &regex::Captures| {
        let key = &caps[1];
        vars.get(key)
            .cloned()
            .unwrap_or_else(|| "".to_string()) 
    })
    .into_owned()
}


mod test {
    use crate::mc_types::McSpecificVersionDetail;
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_var_args() {
      let data = r#"${auth_player_name}"#;
      let mut vars = HashMap::new();
      vars.insert("auth_player_name", "TestPlayer".into());
      let ans = resolve_argument(data, &vars);
      assert_eq!(ans, "TestPlayer");
    }

    #[tokio::test]
    async fn test_parse_mc_specific_version_detail() {
        let data = std::fs::read_to_string("data/26.1.2.json").expect("can't read file");
        match serde_json::from_str::<McSpecificVersionDetail>(&data) {
            Ok(v) => {
                println!("解析成功！");
                println!("完整結構體: {:#?}", v.arguments.unwrap().jvm);
            }
            Err(e) => {
                println!("解析失敗: {}", e);
            }
        }
    }
}
