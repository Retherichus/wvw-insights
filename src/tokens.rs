use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    success: bool,
    token: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValidationResponse {
    valid: bool,
}

/// Generates a new history token from the server
pub fn generate_token() -> Result<String> {
    let url = "https://parser.rethl.net/api.php?endpoint=generate-token";
    
    let response = ureq::get(url).call()?;
    let token_resp: TokenResponse = response.into_json()?;
    
    if token_resp.success {
        token_resp.token.ok_or_else(|| anyhow::anyhow!("No token in response"))
    } else {
        Err(anyhow::anyhow!(
            "Token generation failed: {}", 
            token_resp.message.unwrap_or_default()
        ))
    }
}

/// Validates a history token with the server
pub fn validate_token(api_endpoint: &str, token: &str) -> Result<bool> {
    let url = format!("{}?endpoint=nexus-validate-token", api_endpoint);
    
    let response = ureq::post(&url)
        .send_form(&[("history_token", token)])?;
    
    let validation_resp: ValidationResponse = response.into_json()?;
    
    Ok(validation_resp.valid)
}