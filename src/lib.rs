pub mod error;

mod core;
pub use core::{
    DEFAULT_ARGS,
    BrowserSession,
    BrowserSessionConfig,
    BrowserError,
    BrowserTimings,
    MyIP,
    PageParam,
    random_user_agent,
};
pub use core::extension;
pub use chromiumoxide;


#[cfg(test)]
mod tests {
    // use super::*;

    #[tokio::test]
    async fn benchmark() {
        assert_eq!(true, true);
    }
}
