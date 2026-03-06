pub fn get_flags(url: &str) -> Vec<String> {
    vec![
        // True Edge kiosk mode
        "--kiosk".to_string(),
        url.to_string(),         

        // Lockdown style
        "--edge-kiosk-type=fullscreen".to_string(),
        "--no-first-run".to_string(),
        "--inprivate".to_string(),
        "--ignore-certificate-errors".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--disable-print-preview".to_string(),
        "--disable-pdf-extension".to_string(),
        "--disable-save-password-bubble".to_string(),
        "--disable-translate".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-client-side-phishing-detection".to_string(),
        "--disable-component-extensions-with-background-pages".to_string(),
        "--disable-pinch".to_string(),
        "--disable-smooth-scrolling".to_string(),
        "--overscroll-history-navigation=0".to_string(),
        "--no-referrers".to_string(),
    ]
}
