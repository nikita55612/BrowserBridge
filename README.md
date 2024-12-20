# BrowserBridge

A crate written in Rust for automating browser interactions. It serves as a wrapper around the [chromiumoxide](https://github.com/mattsse/chromiumoxide) crate, which utilizes the [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/) to control and launch Chromium or Chrome browsers (including headless mode).

*BrowserBridge* enhances functionality with its custom Google Chrome extension, enabling features such as proxy switching and complete browser data removal.

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
        //proxy: 
        Some("username:password@host:port"),
        //user_agent: 
        Some("Mozilla/5.0 (Windows NT 10.0; rv:110.0) Gecko/20100101 Firefox/110.0"),
        //cookies: 
        Some(vec![CookieParam::new("Api-Token", "r.Wd34dO5pgmfcc4Moe94Fvdf431")]),
        //duration: 
        Some(2000)
    );
    bs.open_with_param("https://www.google.com/", param).await?;
    let _ = bs.reset_proxy().await;
    
    bs.close().await;
}
```
