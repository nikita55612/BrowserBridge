//#![warn(missing_docs)]
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use tokio::{
    task::JoinHandle,
    time::{sleep, timeout}
};
use chromiumoxide::{
    cdp::browser_protocol::network::CookieParam,
    browser::HeadlessMode,
    Browser,
    BrowserConfig,
    Page
};
use rand::Rng;

pub use crate::error::BrowserError;
use super::extension;


#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MyIP {
    pub ip: String,
    pub country: String,
    pub cc: String,
}

pub static DEFAULT_ARGS: [&str; 16] = [
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

#[derive(Clone, Debug)]
pub struct BrowserTimings {
    pub launch_sleep: u64,
    pub set_proxy_sleep: u64,
    pub action_sleep: u64,
    pub wait_page_timeout: u64,
}

impl Default for BrowserTimings {
    fn default() -> Self {
        Self {
            launch_sleep: 280,
            set_proxy_sleep: 180,
            action_sleep: 80,
            wait_page_timeout: 700
        }
    }
}

pub struct BrowserSession {
    pub browser: Browser,
    pub handle: JoinHandle<()>,
    pub timings: BrowserTimings,
}

#[derive(Clone, Debug)]
pub struct PageParam<'a> {
    pub proxy: Option<&'a str>,
    pub wait_for_element: Option<(&'a str, u64)>,
    pub user_agent: Option<&'a str>,
    pub cookies: Vec<CookieParam>,
    pub stealth_mode: bool,
    pub duration: u64
}

impl<'a> Default for PageParam<'a> {
    fn default() -> Self {
        Self {
            proxy: None,
            wait_for_element: None,
            user_agent: None,
            cookies: Vec::new(),
            stealth_mode: false,
            duration: 0
        }
    }
}

pub struct BrowserSessionConfig {
    pub executable: Option<String>,
    pub args: Vec<String>,
    pub headless: HeadlessMode,
    pub sandbox: bool,
    pub extensions: Vec<String>,
    pub incognito: bool,
    pub user_data_dir: Option<String>,
    pub port: u16,
    pub launch_timeout: u64,
    pub request_timeout: u64,
    pub cache_enabled: bool,
    pub timings: BrowserTimings,
}

impl Default for BrowserSessionConfig {
    fn default() -> Self {
        Self {
            executable: None,
            args: DEFAULT_ARGS.into_iter()
                .map(|v| v.into())
                .collect(),
            headless: HeadlessMode::False,
            sandbox: false,
            extensions: Vec::new(),
            incognito: false,
            user_data_dir: None,
            port: 0,
            launch_timeout: 1500,
            request_timeout: 2000,
            cache_enabled: true,
            timings: BrowserTimings::default(),
        }
    }
}

pub trait FromSessionConfig {
    fn to_config(&self) -> Result<BrowserConfig, BrowserError>;
}

impl FromSessionConfig for BrowserSessionConfig {
    fn to_config(&self) -> Result<BrowserConfig, BrowserError> {
        let mut extensions = Vec::new();
        extensions.push(extension::PATH.clone());
        extensions.extend_from_slice(
            self.extensions.as_slice()
        );
        let mut builder = BrowserConfig::builder()
            .disable_default_args()
            .headless_mode(self.headless)
            .args(&self.args)
            .extensions(extensions)
            .viewport(None)
            .port(self.port)
            .launch_timeout(
                Duration::from_millis(self.launch_timeout)
            )
            .request_timeout(
                Duration::from_millis(self.request_timeout)
            );

        if self.incognito {
            builder = builder.incognito();
        }
        if !self.sandbox {
            builder = builder.no_sandbox();
        }
        if self.cache_enabled {
            builder = builder.enable_cache();
        }
        if let Some(user_data_dir) = &self.user_data_dir {
            builder = builder.user_data_dir(user_data_dir);
        }
        if let Some(executable) = &self.executable {
            builder = builder.chrome_executable(executable);
        }

        builder.build()
            .map_err(|_| BrowserError::BuildBrowserConfigError)
    }
}

impl BrowserSession {
    pub async fn launch(bsc: BrowserSessionConfig) -> Result<Self, BrowserError> {
        let timings = bsc.timings.clone();
        let (browser, mut handler) = Browser::launch(
            bsc.to_config()?
        ).await?;
        let handle = tokio::task::spawn(async move {
            while handler.next().await.is_some() {}
        });
        sleep(
            Duration::from_millis(timings.launch_sleep)
        ).await;

        Ok (
            Self {
                browser,
                handle,
                timings
            }
        )
    }

    pub async fn launch_with_default_config() -> Result<Self, BrowserError> {
        let config = BrowserSessionConfig::default();
        Self::launch(config).await
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
        Ok(new_page)
    }

    pub async fn open_on_page<'a>(&self, url: &str, page: &'a Page) -> Result<(), BrowserError> {
        page.goto(url).await?;
        let _ = timeout(
            Duration::from_millis(self.timings.wait_page_timeout),
            page.wait_for_navigation()
        ).await;

        Ok(())
    }

    pub async fn open(&self, url: &str) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        self.open_on_page(url, &page).await?;

        Ok(page)
    }

    pub async fn open_with_duration(&self, url: &str, duration: u64) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        self.open_on_page(url, &page).await?;
        sleep(
            Duration::from_millis(duration)
        ).await;

        Ok(page)
    }

    pub async fn open_with_param<'a>(&self, url: &str, param: &PageParam<'a>) -> Result<Page, BrowserError> {
        if let Some(proxy) = param.proxy {
            self.set_proxy(proxy).await?;
        }
        let page = self.new_page().await?;
        if let Some(user_agent) = param.user_agent {
            page.set_user_agent(user_agent).await?;
        }
        if !param.cookies.is_empty() {
            page.set_cookies(param.cookies.clone()).await?;
        }
        if param.stealth_mode {
            let _ = page.enable_stealth_mode().await;
        }
        self.open_on_page(url, &page).await?;
        sleep(
            Duration::from_millis(param.duration)
        ).await;
        if let Some((selector, timeout)) = param.wait_for_element {
            let _ = page.wait_for_element_with_timeout(
                selector, timeout
            ).await;
        }

        Ok(page)
    }

    pub async fn set_proxy(&self, proxy: &str) -> Result<(), BrowserError> {
        if let Err(e) = self.browser.new_page(format!("chrome://set_proxy/{proxy}")).await {
            let error = BrowserError::from(e);
            match error {
                BrowserError::NetworkIO => {},
                _ => { return Err(error); }
            }
        }
        sleep(
            Duration::from_millis(self.timings.set_proxy_sleep)
        ).await;
        Ok(())
    }

    pub async fn reset_proxy(&self) -> Result<(), BrowserError> {
        if let Err(e) = self.browser.new_page("chrome://reset_proxy").await {
            let error = BrowserError::from(e);
            match error {
                BrowserError::NetworkIO => {},
                _ => { return Err(error); }
            }
        }
        sleep(
            Duration::from_millis(self.timings.action_sleep)
        ).await;
        Ok(())
    }

    pub async fn close_tabs(&self) -> Result<(), BrowserError> {
        if let Err(e) = self.browser.new_page("chrome://close_tabs").await {
            let error = BrowserError::from(e);
            match error {
                BrowserError::NetworkIO => {},
                _ => { return Err(error); }
            }
        }
        sleep(
            Duration::from_millis(self.timings.action_sleep)
        ).await;
        Ok(())
    }

    pub async fn clear_data(&self) -> Result<(), BrowserError> {
        if let Err(e) = self.browser.new_page("chrome://clear_data").await {
            let error = BrowserError::from(e);
            match error {
                BrowserError::NetworkIO => {},
                _ => { return Err(error); }
            }
        }
        sleep(
            Duration::from_millis(self.timings.action_sleep)
        ).await;
        Ok(())
    }

    pub async fn myip(&self) -> Result<MyIP, BrowserError> {
        let page = self.open("https://api.myip.com/").await?;
        let myip = page.find_element("body").await?
            .inner_text().await?
            .ok_or(BrowserError::Serialization)
            .map(|s|
                serde_json::from_str(&s)
                .map_err(|_| BrowserError::Serialization)
            )?;
        let _ = page.close().await;
        myip
    }
}

pub trait Wait {
    const WAIT_SLEEP: u64 = 10;

    async fn wait_for_element(&self, selector: &str) -> Result<(), BrowserError>;

    async fn wait_for_element_with_timeout(&self, selector: &str, t: u64) -> Result<(), BrowserError>;
}

impl Wait for Page {
    async fn wait_for_element(
        &self, selector: &str
    ) -> Result<(), BrowserError> {
        while self.find_element(selector).await.is_err() {
            sleep(
                Duration::from_millis(Self::WAIT_SLEEP)
            ).await;
        }

        Ok(())
    }

    async fn wait_for_element_with_timeout(
        &self, selector: &str, t: u64
    ) -> Result<(), BrowserError> {
        timeout(
            Duration::from_millis(t),
            self.wait_for_element(selector)
        ).await??;

        Ok(())
    }
}


static USER_AGENT_LIST: [&str; 20] = [
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

pub fn random_user_agent() -> &'static str {
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(0..USER_AGENT_LIST.len());
    USER_AGENT_LIST[index]
}
