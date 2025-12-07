use lazy_static::lazy_static;
use std::env;

pub struct AppConfig {
    pub target_url: String,
    pub proxy_port: u16,
    pub allowed_domains: Vec<String>,
}

lazy_static! {
    pub static ref CONFIG: AppConfig = {
        dotenv::dotenv().ok();

        let target_url = env::var("TARGET_URL")
            .unwrap_or_else(|_| "http://evalify.amritanet.edu".to_string());
        
        let proxy_port = env::var("PROXY_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
            
        let allowed_domains_str = env::var("ALLOWED_DOMAINS")
            .unwrap_or_else(|_| "evalify.amritanet.edu,localhost:3000".to_string());
            
        let allowed_domains = allowed_domains_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        AppConfig {
            target_url,
            proxy_port,
            allowed_domains,
        }
    };
}
