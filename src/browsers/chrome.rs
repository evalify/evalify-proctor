#[cfg(target_os = "windows")]
const KIOSK_DATA_DIR: &str = "C:\\Windows\\Temp\\EvalifyKiosk";
#[cfg(not(target_os = "windows"))]
const KIOSK_DATA_DIR: &str = "/tmp/EvalifyKiosk";

pub fn get_flags(url: &str) -> Vec<String> {
    vec![
        url.to_string(),
        "--kiosk".to_string(),
        "--fullscreen".to_string(),
        "--new-window".to_string(),
        "--no-first-run".to_string(),
        format!("--user-data-dir={}", KIOSK_DATA_DIR),
        "--incognito".to_string(),
        "--disable-extensions".to_string(),
        "--disable-pdf-extension".to_string(),
        "--disable-print-preview".to_string(),
        "--disable-save-password-bubble".to_string(),
        "--disable-translate".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-sync".to_string(),
        "--disable-client-side-phishing-detection".to_string(),
        "--disable-component-extensions-with-background-pages".to_string(),
        "--disable-pinch".to_string(),
        "--disable-smooth-scrolling".to_string(),
        "--overscroll-history-navigation=0".to_string(),
        "--remote-debugging-port=0".to_string(),
        "--disable-features=Translate,TranslateUI,PrintPreview,Pay,AutofillServerCommunication".to_string(),
    ]
}
