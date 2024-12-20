pub mod error;
mod utils;

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
    use super::*;
    use tokio::time::sleep;
    use std::time::Duration;
    //use std::path::Path;


    #[tokio::test]
    async fn benchmark() {
        let task1 = tokio::spawn(async move {
            println!("Start task #1");

            let bsc = BrowserSessionConfig{
                user_data_dir: Some(r"C:\Users\Nikita\Projects\browser_bridge\temp_user_data_dir".into()),
                port: 1365,
                ..Default::default()
            };

            let mut bs = BrowserSession::launch(bsc)
                .await
                .map_err(|e| println!("Task #1 error: {e}"))
                .unwrap();

            sleep(Duration::from_millis(600)).await;

            for i in 0..20 {
                let page = bs.open(
                    "https://en.wikipedia.org/wiki/Main_Page"
                ).await.unwrap();
                let _ = page.close().await;
                println!("Iteration numder {i}");
            }

            bs.close().await;
        });

        let task2 = tokio::spawn(async move {
            println!("Start task #2");

            let bsc = BrowserSessionConfig{
                user_data_dir: Some(r"C:\Users\Nikita\Projects\browser_bridge\temp_user_data_dir2".into()),
                port: 1366,
                ..Default::default()
            };

            let mut bs = BrowserSession::launch(bsc)
                .await
                .map_err(|e| println!("Task #2 error: {e}"))
                .unwrap();

            sleep(Duration::from_millis(1000)).await;

            for i in 0..20 {
                let page = bs.open(
                    "https://en.wikipedia.org/wiki/Main_Page"
                ).await.unwrap();
                let _ = page.close().await;
                println!("Iteration numder {i}");
            }

            bs.close().await;
        });

        let task3 = tokio::spawn(async move {
            println!("Start task #3");

            let bsc = BrowserSessionConfig{
                user_data_dir: Some(r"C:\Users\Nikita\Projects\browser_bridge\temp_user_data_dir3".into()),
                port: 1368,
                ..Default::default()
            };

            let mut bs = BrowserSession::launch(bsc)
                .await
                .map_err(|e| println!("Task #3 error: {e}"))
                .unwrap();

                sleep(Duration::from_millis(2100)).await;

            for i in 0..20 {
                let page = bs.open(
                    "https://en.wikipedia.org/wiki/Main_Page"
                ).await.unwrap();
                let _ = page.close().await;
                println!("Iteration numder {i}");
            }

            bs.close().await;
        });

        let _ = tokio::join!(task1, task2, task3);

        assert_eq!(true, true);
    }
}
