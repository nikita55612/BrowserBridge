use std::{collections::HashSet, future::Future, time::Duration};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use tokio::{
    task::JoinHandle, 
    time::{sleep, timeout}
};
use chromiumoxide::{
    Browser, 
    BrowserConfig, 
    Page
};

use crate::error::BrowserError;
use super::extension;


#[derive(Clone, Debug, Deserialize)]
pub struct MyIP {
    pub ip: String,
    pub country: String,
    pub cc: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HeadlessMode {
    False,
    True,
    New,
}

static DEFAULT_ARGS: [&str; 16] = [
    "--disable-background-networking",
    "--enable-features=NetworkService,NetworkServiceInProcess",
    "--disable-client-side-phishing-detection",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-breakpad",
    "--disable-features=TranslateUI",
    "--disable-prompt-on-repost",
    "--no-first-run",
    "--disable-sync",
    "--force-color-profile=srgb",
    "--enable-blink-features=IdleDetection",
    "--lang=en_US",
    "--no-sandbox",
    "--disable-gpu",
    "--disable-smooth-scrolling"
];

#[derive(Deserialize, Serialize)]
pub struct BrowserSessionConfig {
    pub executable: Option<String>,
    pub headless: HeadlessMode,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub incognito: bool,
    pub user_data_dir: Option<String>,
    pub launch_sleep: u64,
    pub set_proxy_sleep: u64,
    pub open_page_sleep: u64,
    pub open_page_timeout: u64,
}

impl Default for BrowserSessionConfig {
    fn default() -> Self {
        Self {
            executable: None,
            headless: HeadlessMode::False,
            args: DEFAULT_ARGS.into_iter()
                .map(|v| v.to_string())
                .collect(),
            extensions: Vec::new(),
            incognito: false,
            user_data_dir: None,
            launch_sleep: 250,
            set_proxy_sleep: 300,
            open_page_sleep: 200,
            open_page_timeout: 1000,
        }
    }
}

impl From<BrowserSessionConfig> for BrowserConfig {
    fn from(bsc: BrowserSessionConfig) -> Self {
        let mut extensions = Vec::new();
        extensions.push(
            extension::PATH.lock()
                .as_deref()
                .map(|v| v.clone())
                .unwrap_or(String::new())
        );
        extensions.extend_from_slice(bsc.extensions.as_slice());
        let headless = match bsc.headless {
            HeadlessMode::False => chromiumoxide::browser::HeadlessMode::False,
            HeadlessMode::True=> chromiumoxide::browser::HeadlessMode::True,
            HeadlessMode::New=> chromiumoxide::browser::HeadlessMode::New,
        };
        let mut args = bsc.args.iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>();
        args.extend_from_slice(&DEFAULT_ARGS);
        let args = args.into_iter().collect::<HashSet<_>>();
        let mut builder = BrowserConfig::builder()
            .disable_default_args()
            .headless_mode(headless)
            .args(args)
            .extensions(extensions)
            .viewport(None)
            .launch_timeout(Duration::from_millis(1500));
        if bsc.incognito { builder = builder.incognito(); }
        if bsc.user_data_dir.is_some() { 
            builder = builder.user_data_dir(bsc.user_data_dir.unwrap()); 
        }
        if bsc.executable.is_some() { 
            builder = builder.chrome_executable(bsc.executable.unwrap()); 
        }
        builder.build().unwrap()
    }
    
}

pub struct BrowserTimings {
    launch_sleep: u64,
    set_proxy_sleep: u64,
    open_page_sleep: u64,
    open_page_timeout: u64,
}

impl BrowserTimings {
    pub fn new(
        launch_sleep: u64, 
        set_proxy_sleep: u64, 
        open_page_sleep: u64, 
        open_page_timeout: u64
    ) -> Self {
        Self {
            launch_sleep,
            set_proxy_sleep,
            open_page_sleep,
            open_page_timeout
        }
    }
}

pub struct BrowserSession {
    pub browser: Browser,
    pub handle: JoinHandle<()>,
    timings: BrowserTimings
}

impl BrowserSession {
    pub async fn new(bsc: BrowserSessionConfig) -> Result<Self, BrowserError> {
        let timings = BrowserTimings::new(
            bsc.launch_sleep, 
            bsc.set_proxy_sleep, 
            bsc.open_page_sleep, 
            bsc.open_page_timeout
        );
        let (browser, mut handler) = Browser::launch(
            BrowserConfig::from(bsc)
        ).await?;
        let handle = tokio::task::spawn(async move {
            while handler.next().await.is_some() {}
        });
        sleep(Duration::from_millis(timings.launch_sleep)).await;
        Ok(
            Self {
                browser,
                handle,
                timings
            }
        )
    }

    pub async fn from_default_config() -> Result<Self, BrowserError> {
        let bsc = BrowserSessionConfig::default();
        Self::new(bsc).await
    }

    pub async fn set_timings(&mut self, timings: BrowserTimings) {
        self.timings = timings;
    }

    pub async fn close(&mut self) {
        if self.browser.close().await.is_err() {
            self.browser.kill().await;
        }
        if self.browser.wait().await.is_err() {
            let mut attempts = 0;
            while self.browser.try_wait().is_err() && attempts < 4 {
                attempts += 1;
            }
        }
        self.handle.abort();
    }

    pub async fn new_page(&self) -> Result<Page, BrowserError> {
        Ok(self.browser.new_page("about:blank").await?)
    }

    pub async fn open_on_page<'a>(
        &self, url: &str, page: &'a Page
    ) -> Result<&'a Page, BrowserError> {
        timeout(
            Duration::from_millis(self.timings.open_page_timeout), 
            {
                page.goto(url).await?;
                page.wait_for_navigation()
            }
        ).await??;
        sleep(Duration::from_millis(self.timings.open_page_sleep)).await;
        Ok(page)
    }

    pub async fn open(&self, url: &str) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        self.open_on_page(url, &page).await?;
        Ok(page)
    }

    pub async fn open_with_duration(&self, url: &str, millis: u64) -> Result<Page, BrowserError> {
        let page = self.open(url).await?;
        sleep(Duration::from_millis(millis)).await;
        Ok(page)
    }

    pub async fn with_open<F, Fut, R>(&self, url: &str, f: F) -> Result<R, BrowserError>
    where
        F: FnOnce(&Page) -> Fut,
        Fut: Future<Output = Result<R, BrowserError>>
    {
        let page = self.open(url).await?;
        Ok(f(&page).await?)
    }

    pub async fn with_new_open<_F, F_, _Fut, Fut_, _R, R_>(
        &self, url: &str, _f: _F, f_: F_
    ) -> Result<R_, BrowserError>
    where
        _F: FnOnce(&Page) -> _Fut,
        _Fut: Future<Output = Result<_R, BrowserError>>,
        F_: FnOnce(&Page) -> Fut_,
        Fut_: Future<Output = Result<R_, BrowserError>>,
    {
        let page = self.new_page().await?;
        _f(&page).await?;
        let page = self.open_on_page(url, &page).await?;
        Ok(f_(&page).await?)
    }

    pub async fn set_proxy(&self, proxy: &str) -> Result<(), BrowserError> {
        self.browser.new_page(format!("chrome://set_proxy/{proxy}")).await?;
        sleep(Duration::from_millis(self.timings.set_proxy_sleep)).await;
        Ok(())
    }

    pub async fn reset_proxy(&self) -> Result<(), BrowserError> {
        self.browser.new_page("chrome://reset_proxy").await?;
        sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    pub async fn clear_data(&self) -> Result<(), BrowserError> {
        self.browser.new_page("chrome://clear_data").await?;
        sleep(Duration::from_millis(100)).await;
        Ok(())  
    }

    pub async fn myip(&self) -> Result<MyIP, BrowserError> {
        let page = self.open("https://api.myip.com/").await?;
        page.find_element("body").await?
            .inner_text().await?
            .ok_or(BrowserError::ParseMyIp)
            .map(|s| 
                serde_json::from_str(&s)
                .map_err(|_| BrowserError::ParseMyIp)
            )?
    }
}