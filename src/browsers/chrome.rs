pub fn get_flags(url: &str) -> Vec<String> {
    vec![
        format!("--app={}",url),
        "no-first-run".to_string(),
        "--incognito".to_string(),
    ]

}
