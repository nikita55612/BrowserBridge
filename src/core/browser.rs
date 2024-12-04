use std::{collections::HashSet, future::Future, time::Duration};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use tokio::{
    task::JoinHandle, 
    time::{sleep, timeout}
};
use chromiumoxide::{
    Browser, 
    BrowserConfig
};
pub use chromiumoxide::{
    cdp::browser_protocol::network::CookieParam, 
    Page
};
use rand::Rng;

use crate::error::BrowserError;
use super::extension;


#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MyIP {
    pub ip: String,
    pub country: String,
    pub cc: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct BrowserTimings {
    launch_sleep: u64,
    set_proxy_sleep: u64,
    page_sleep: u64,
    wait_page_timeout: u64,
}

impl BrowserTimings {
    pub fn new(
        launch_sleep: u64, 
        set_proxy_sleep: u64, 
        page_sleep: u64, 
        wait_page_timeout: u64
    ) -> Self {
        Self {
            launch_sleep,
            set_proxy_sleep,
            page_sleep,
            wait_page_timeout
        }
    }
}

impl Default for BrowserTimings {
    fn default() -> Self {
        Self {
            launch_sleep: 200, 
            set_proxy_sleep: 300, 
            page_sleep: 250, 
            wait_page_timeout: 1000
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct BrowserSessionConfig {
    pub executable: Option<String>,
    pub headless: HeadlessMode,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub incognito: bool,
    pub user_data_dir: Option<String>,
    pub launch_timeout: u64,
    pub timings: BrowserTimings
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
            launch_timeout: 1500,
            timings: BrowserTimings::default(),
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
            .launch_timeout(Duration::from_millis(bsc.launch_timeout));

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

pub struct BrowserSession {
    pub browser: Browser,
    pub handle: JoinHandle<()>,
    timings: BrowserTimings
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PageParam<'a> {
    pub duration: Option<u64>,
    pub user_agent: Option<&'a str>,
    pub cookies: Option<Vec<CookieParam>>
}

impl BrowserSession {
    pub async fn launch(bsc: BrowserSessionConfig) -> Result<Self, BrowserError> {
        let timings = bsc.timings.clone();
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

    pub async fn launch_with_default_config() -> Result<Self, BrowserError> {
        let bsc = BrowserSessionConfig::default();
        Self::launch(bsc).await
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
        let new_page = self.browser.new_page("about:blank").await?;
        let user_agent = get_random_user_agent();
        new_page.enable_stealth_mode_with_agent(user_agent).await?;
        Ok(new_page)
    }

    pub async fn open_on_page<'a>(
        &self, url: &str, page: &'a Page
    ) -> Result<&'a Page, BrowserError> {
        timeout(
            Duration::from_millis(self.timings.wait_page_timeout), 
            {
                page.goto(url).await?;
                page.wait_for_navigation()
            }
        ).await??;
        sleep(Duration::from_millis(self.timings.page_sleep)).await;
        Ok(page)
    }

    pub async fn open(&self, url: &str) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        self.open_on_page(url, &page).await?;
        Ok(page)
    }

    pub async fn open_with_param<'a>(&self, url: &str, param: PageParam<'a>) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        if let Some(cookies) = param.cookies {
            page.set_cookies(cookies).await?;
        }
        if let Some(user_agent) = param.user_agent {
            page.set_user_agent(user_agent).await?;
        }
        self.open_on_page(url, &page).await?;
        if let Some(duration) = param.duration {
            sleep(Duration::from_millis(
                (duration as i32 - self.timings.page_sleep as i32).max(1) as u64
            )).await;
        }
        Ok(page)
    }

    pub async fn open_with_duration(&self, url: &str, millis: u64) -> Result<Page, BrowserError> {
        let page = self.open(url).await?;
        sleep(Duration::from_millis(
            (millis as i32 - self.timings.page_sleep as i32).max(1) as u64
        )).await;
        Ok(page)
    }

    pub async fn open_with_cookies(&self, url: &str, cookies: Vec<CookieParam>) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        page.set_cookies(cookies).await?;
        self.open_on_page(url, &page).await?;
        Ok(page)
    }

    pub async fn open_with_cookies_and_duration(
        &self, url: &str, cookies: Vec<CookieParam>, millis: u64
    ) -> Result<Page, BrowserError> {
        let page = self.open_with_cookies(url, cookies).await?;
        sleep(Duration::from_millis(
            (millis as i32 - self.timings.page_sleep as i32).max(1) as u64
        )).await;
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
        sleep(Duration::from_millis(150)).await;
        Ok(())  
    }

    pub async fn myip(&self) -> Result<MyIP, BrowserError> {
        let page = self.open("https://api.myip.com/").await?;
        page.find_element("body").await?
            .inner_text().await?
            .ok_or(BrowserError::Serialization)
            .map(|s| 
                serde_json::from_str(&s)
                .map_err(|_| BrowserError::Serialization)
            )?
    }
}

pub static USER_AGENT_LIST: [&str; 20] = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Edge/117.0.2045.60 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; WOW64; rv:102.0) Gecko/20100101 Firefox/102.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 12.6; rv:116.0) Gecko/20100101 Firefox/116.0",
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:118.0) Gecko/20100101 Firefox/118.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Safari/605.1.15",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 16_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (iPad; CPU OS 16_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 12; SM-A515F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (Windows NT 11.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_0_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Linux; U; Android 12; en-US; SM-T870 Build/SP1A.210812.016) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/100.0.4896.127 Safari/537.36",
    "Mozilla/5.0 (Linux; Android 11; Mi 10T Pro Build/RKQ1.200826.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.41 Mobile Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; rv:110.0) Gecko/20100101 Firefox/110.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:91.0) Gecko/20100101 Firefox/91.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.1.2 Safari/605.1.15",
];

pub fn get_random_user_agent() -> &'static str {
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(0..USER_AGENT_LIST.len());
    USER_AGENT_LIST[index]
}