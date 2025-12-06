pub fn get_flags(url: &str) -> Vec<String> {
    vec![
        format!("--app={}",url),
        "--edge-kioski-type=fullscreen".to_string(),
        "--inprivate".to_string(),
    ]
}