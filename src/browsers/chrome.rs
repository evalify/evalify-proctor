pub fn get_flags(url: &str) -> Vec<String> {
    vec![
        // Launch target
        url.to_string(),

        // Kiosk / fullscreen
        "--kiosk".to_string(),
        "--fullscreen".to_string(),

        // Basic window / profile behaviour
        "--new-window".to_string(),
        "--no-first-run".to_string(),
        "--user-data-dir=C:\\Windows\\Temp\\EvalifyKiosk".to_string(),
        "--incognito".to_string(),

        // Extensions / plugins / PDF / print
        "--disable-extensions".to_string(),
        "--disable-pdf-extension".to_string(),
        "--disable-print-preview".to_string(),
        "--disable-save-password-bubble".to_string(),

        // Translation / background stuff / sync
        "--disable-translate".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-sync".to_string(),
        "--disable-client-side-phishing-detection".to_string(),
        "--disable-component-extensions-with-background-pages".to_string(),

        // UX / input tweaks
        "--disable-pinch".to_string(),
        "--disable-smooth-scrolling".to_string(),
        "--overscroll-history-navigation=0".to_string(),

        // Remote debugging — 0 = random port, doesn’t disable DevTools,
        "--remote-debugging-port=0".to_string(),

        // Feature toggles
        "--disable-features=Translate,TranslateUI,PrintPreview,Pay,AutofillServerCommunication".to_string(),
    ]
}
