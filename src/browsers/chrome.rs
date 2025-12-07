pub fn get_flags(url: &str) -> Vec<String> {
    vec![
        url.to_string(),
        "--new-window".to_string(),
        "--no-first-run".to_string(),
        "--incognito".to_string(),
    ]

}
