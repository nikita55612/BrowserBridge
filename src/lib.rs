
pub mod error;
pub mod utils;

pub mod core;
pub use core::*;


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_works() {
        let mut bs = BrowserSession::launch_with_default_config()
            .await
            .unwrap();

        bs.set_proxy("UeKzXo:Me1JcB@87.245.144.147:8000")
            .await
            .unwrap();

        let myip = bs.myip().await.unwrap();

        bs.close().await;

        println!("{:#?}", myip);

        assert_eq!((), ());
    }
}
