use chromiumoxide::error::CdpError;


#[derive(Debug)]
pub struct ParseProxyError;


#[derive(Debug)]
pub enum BrowserError {
    CdpError(CdpError),
    ElapsedTimeout,
    ParseMyIp,
    Any(String)
}

impl From<String> for BrowserError {
    fn from(s: String) -> Self {
        BrowserError::Any(s)
    }
}

impl From<CdpError> for BrowserError {
    fn from(e: CdpError) -> Self {
        BrowserError::CdpError(e)
    }
}

impl From<tokio::time::error::Elapsed> for BrowserError {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        BrowserError::ElapsedTimeout
    }
}

