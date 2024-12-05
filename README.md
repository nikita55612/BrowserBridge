# BrowserBridge

## Examples
```rust
use browser_bridge::*;


#[tokio::main]
async fn main() -> Result<(), BrowserError> {
    let config = BrowserSessionConfig::default();
    let bs = BrowserSession::launch(bsc).await?;
    
    let page = bs.open("https://www.google.com/").await?;
    let page_content = page.content().await?;
    page.close().await;
    
    let _ = bs.set_proxy("username:password@host:port").await;
    let myip = bs.myip().await?;
    
    println!("{:#?}", myip);
    
    let _ = bs.reset_proxy().await;
    let _ = bs.clear_data().await;
    
    let param = PageParam::new(
        Some("username:password@host:port"), 
        Some("Mozilla/5.0 (Windows NT 10.0; rv:110.0) Gecko/20100101 Firefox/110.0"), 
        Some(vec![CookieParam::new("Api-Token", "r.Wd34dO5pgmfcc4Moe94Fvdf431")]), 
        Some(2000)
    );
    bs.open_with_param("https://www.google.com/", param).await?;
    let _ = bs.reset_proxy().await;
    
    let title = bs.with_open("https://www.google.com/", |p| async move {
        let title = match p.get_title().await {
            Ok(t) => Ok(t.unwrap_or_default()),
            Err(e) => Err(e)
        };
        p.close().await;
        title
    }).await??;
    
    bs.close().await;
}
```
