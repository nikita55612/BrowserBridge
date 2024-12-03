pub use chromiumoxide::Page;

pub mod error;
pub mod utils;

pub mod core;
pub use core::*;


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_works() {
        let mut bs = browser::BrowserSession::from_default_config().await.unwrap();
        let _= bs.set_proxy("UGKzXr:oe6JcB@87.247.146.147:8000").await;
        let myip = bs.myip().await.unwrap();
        bs.close().await;
        println!("myip: {:#?}", myip);
        assert_eq!((), ());
    }
}
