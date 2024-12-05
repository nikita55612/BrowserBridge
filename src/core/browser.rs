#![warn(missing_docs)]

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

pub use crate::error::BrowserError;
use super::extension;


/// Represents IP information retrieved from an IP lookup service
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MyIP {
    /// The IP address
    pub ip: String,
    /// The country name
    pub country: String,
    /// The country code
    pub cc: String,
}

/// Defines the headless mode for browser operation
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HeadlessMode {
    /// Browser runs with a visible UI
    False,
    /// Browser runs fully headless (no UI)
    True,
    /// Browser runs in a new headless mode
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

/// Configuration for browser session timings
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct BrowserTimings {
    /// Sleep duration after browser launch (in milliseconds)
    launch_sleep: u64,
    /// Sleep duration after setting proxy (in milliseconds)
    set_proxy_sleep: u64,
    /// Sleep duration after page load (in milliseconds)
    page_sleep: u64,
    /// Timeout for page navigation (in milliseconds)
    wait_page_timeout: u64,
}

impl BrowserTimings {
    /// New configuration for browser session timings
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
            wait_page_timeout: 500
        }
    }
}

/// Comprehensive browser session configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct BrowserSessionConfig {
    /// Path to Chrome/Chromium executable
    pub executable: Option<String>,
    /// Headless mode setting
    pub headless: HeadlessMode,
    /// Additional browser launch arguments
    pub args: Vec<String>,
    /// Browser extensions to load
    pub extensions: Vec<String>,
    /// Whether to use incognito mode
    pub incognito: bool,
    /// User data directory
    pub user_data_dir: Option<String>,
    /// Timeout for browser launch
    pub launch_timeout: u64,
    /// Timing configurations
    pub timings: BrowserTimings,
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

/// Represents a browser automation session
pub struct BrowserSession {
    /// The Chromium browser instance
    pub browser: Browser,
    /// Tokio task handle for browser management
    pub handle: JoinHandle<()>,
    /// Session timing configurations
    timings: BrowserTimings,
}

/// Parameters for page initialization
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PageParam<'a> {
    /// Optional page proxy address
    pub proxy: Option<&'a str>,
    /// Optional user agent string
    pub user_agent: Option<&'a str>,
    /// Optional cookies to set
    pub cookies: Option<Vec<CookieParam>>,
    /// Optional duration to keep the page open
    pub duration: Option<u64>
}

impl<'a> PageParam<'a> {
    /// New parameters for page initialization
    pub fn new(
        proxy: Option<&'a str>,
        user_agent: Option<&'a str>, 
        cookies: Option<Vec<CookieParam>>, 
        duration: Option<u64>
    ) -> Self {
        Self {
            proxy,
            user_agent,
            cookies,
            duration
        }
    }
}

impl BrowserSession {
    /// Launch a new browser session with custom configuration
    ///
    /// # Examples
    /// ```rust
    /// let config = BrowserSessionConfig::default();
    /// let session = BrowserSession::launch(config).await?;
    /// ```
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

    /// Launch a new browser session with default configuration
    pub async fn launch_with_default_config() -> Result<Self, BrowserError> {
        let bsc = BrowserSessionConfig::default();
        Self::launch(bsc).await
    }

    /// Update the timing configuration for the browser session
    ///
    /// # Parameters
    /// - `timings`: New `BrowserTimings` to be applied to the session
    ///
    /// # Examples
    /// ```rust
    /// session.set_timings(BrowserTimings {
    ///     page_sleep: 1000, // 1 second sleep after page load
    ///     ..session.timings
    /// }).await;
    /// ```
    pub async fn set_timings(&mut self, timings: BrowserTimings) {
        self.timings = timings;
    }

    /// Gracefully close the browser session
    ///
    /// Attempts to close the browser and clean up resources. If the initial 
    /// close fails, it will attempt to forcefully kill the browser process.
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

    /// Create a new browser page with stealth mode and random user agent
    ///
    /// # Returns
    /// A `Result` containing the new `Page` or a `BrowserError`
    pub async fn new_page(&self) -> Result<Page, BrowserError> {
        let new_page = self.browser.new_page("about:blank").await?;
        let user_agent = get_random_user_agent();
        new_page.enable_stealth_mode_with_agent(user_agent).await?;
        Ok(new_page)
    }

    /// Open a URL on an existing page with timeout and sleep
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    /// - `page`: The page to navigate
    ///
    /// # Returns
    /// A `Result` containing the page reference or a `BrowserError`
    ///
    /// # Examples
    /// ```rust
    /// let page = session.new_page().await?;
    /// let session_id = page.session_id();
    /// session.open_on_page("https://example.com", &page).await?;
    /// ```
    pub async fn open_on_page<'a>(
        &self, url: &str, page: &'a Page
    ) -> Result<&'a Page, BrowserError> {
        page.goto(url).await?;
        let _ = timeout(
            Duration::from_millis(self.timings.wait_page_timeout), 
            {
                page.wait_for_navigation()
            }
        ).await;
        sleep(Duration::from_millis(self.timings.page_sleep)).await;
        Ok(page)
    }

    /// Open a new page and navigate to a URL
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    ///
    /// # Returns
    /// A `Result` containing the new page or a `BrowserError`
    ///
    /// # Examples
    /// ```rust
    /// let page = session.open("https://example.com").await?;
    /// ```
    pub async fn open(&self, url: &str) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        self.open_on_page(url, &page).await?;
        Ok(page)
    }

    /// Open a page with advanced configuration options
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    /// - `param`: Page parameters including cookies, user agent, duration
    ///
    /// # Returns
    /// A `Result` containing the new page or a `BrowserError`
    ///
    /// # Examples
    /// ```rust
    /// let cookies = vec![CookieParam { ... }];
    /// let params = PageParam {
    ///     cookies: Some(cookies),
    ///     user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string()),
    ///     duration: Some(5000),
    /// };
    /// let page = session.open_with_param("https://example.com", params).await?;
    /// ```
    pub async fn open_with_param<'a>(&self, url: &str, param: PageParam<'a>) -> Result<Page, BrowserError> {
        if let Some(proxy) = param.proxy {
            self.set_proxy(proxy).await?;
        }
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

    /// Open a URL and keep the page open for a specified duration
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    /// - `millis`: Duration to keep the page open in milliseconds
    pub async fn open_with_duration(&self, url: &str, millis: u64) -> Result<Page, BrowserError> {
        let page = self.open(url).await?;
        sleep(Duration::from_millis(
            (millis as i32 - self.timings.page_sleep as i32).max(1) as u64
        )).await;
        Ok(page)
    }

    /// Open a URL with predefined cookies
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    /// - `cookies`: Vector of cookies to set
    ///
    /// # Returns
    /// A `Result` containing the page or a `BrowserError`
    ///
    /// # Examples
    /// ```rust
    /// let cookies = vec![
    ///     CookieParam { 
    ///         name: "session_id".to_string(), 
    ///         value: "abc123".to_string(), 
    ///         ..Default::default() 
    ///     }
    /// ];
    /// let page = session.open_with_cookies("https://example.com", cookies).await?;
    /// ```
    pub async fn open_with_cookies(&self, url: &str, cookies: Vec<CookieParam>) -> Result<Page, BrowserError> {
        let page = self.new_page().await?;
        page.set_cookies(cookies).await?;
        self.open_on_page(url, &page).await?;
        Ok(page)
    }

    /// Open a URL with cookies and keep the page open for a specified duration
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    /// - `cookies`: Vector of CookieParam to set
    /// - `millis`: Duration to keep the page open in milliseconds
    ///
    /// # Returns
    /// A `Result` containing the page or a `BrowserError`
    pub async fn open_with_cookies_and_duration(
        &self, url: &str, cookies: Vec<CookieParam>, millis: u64
    ) -> Result<Page, BrowserError> {
        let page = self.open_with_cookies(url, cookies).await?;
        sleep(Duration::from_millis(
            (millis as i32 - self.timings.page_sleep as i32).max(1) as u64
        )).await;
        Ok(page)
    }

    /// Execute a function on a newly opened page
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    /// - `f`: Async function to execute on the page
    ///
    /// # Returns
    /// A `Result` containing the function's return value or a `BrowserError`
    ///
    /// # Examples
    /// ```rust
    /// let result = session.with_open("https://example.com", |page| async {
    ///     let title = page.title().await?;
    ///     Ok(title)
    /// }).await?;
    /// ```
    pub async fn with_open<F, Fut, R>(&self, url: &str, f: F) -> Result<R, BrowserError>
    where
        F: FnOnce(&Page) -> Fut,
        Fut: Future<Output = R>
    {
        let page = self.open(url).await?;
        Ok(f(&page).await)
    }

    /// Execute two functions on a new page (pre-navigation and post-navigation)
    ///
    /// # Parameters
    /// - `url`: The URL to navigate to
    /// - `_f`: Function to execute before navigation
    /// - `f_`: Function to execute after navigation
    ///
    /// # Returns
    /// A `Result` containing the second function's return value or a `BrowserError`
    ///
    /// # Examples
    /// ```rust
    /// let result = session.with_new_open(
    ///     "https://example.com", 
    ///     |page| async { page.set_viewport(1920, 1080).await },
    ///     |page| async { page.screenshot().await }
    /// ).await?;
    /// ```
    pub async fn with_new_open<_F, F_, _Fut, Fut_, _R, R_>(
        &self, url: &str, _f: _F, f_: F_
    ) -> Result<R_, BrowserError>
    where
        _F: FnOnce(&Page) -> _Fut,
        _Fut: Future<Output = _R>,
        F_: FnOnce(&Page) -> Fut_,
        Fut_: Future<Output = R_>,
    {
        let page = self.new_page().await?;
        _f(&page).await;
        let page = self.open_on_page(url, &page).await?;
        Ok(f_(&page).await)
    }

    /// Set a proxy for the browser session
    ///
    /// # Parameters
    /// - `proxy`: Proxy server address (username:password@host:port or host:port)
    ///
    /// # Returns
    /// A `Result` indicating success or a `BrowserError`
    ///
    /// # Examples
    /// ```rust
    /// session.set_proxy("username:password@host:port").await?;
    /// let myip = session.myip().await?;
    /// println!("IP: {}", myip.ip);
    /// 
    /// session.reset_proxy().await?;
    /// ```
    pub async fn set_proxy(&self, proxy: &str) -> Result<(), BrowserError> {
        if let Err(e) = self.browser.new_page(format!("chrome://set_proxy/{proxy}")).await {
            let error = BrowserError::from(e);
            match error {
                BrowserError::NetworkIO => {},
                _ => { return Err(error); }
            }
        }
        sleep(Duration::from_millis(self.timings.set_proxy_sleep)).await;
        Ok(())
    }

    /// Reset the browser's proxy settings
    ///
    /// # Returns
    /// A `Result` indicating success or a `BrowserError`
    pub async fn reset_proxy(&self) -> Result<(), BrowserError> {
        if let Err(e) = self.browser.new_page("chrome://reset_proxy").await {
            let error = BrowserError::from(e);
            match error {
                BrowserError::NetworkIO => {},
                _ => { return Err(error); }
            }
        }
        sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    /// Clear all browser data
    ///
    /// # Returns
    /// A `Result` indicating success or a `BrowserError`
    pub async fn clear_data(&self) -> Result<(), BrowserError> {
        if let Err(e) = self.browser.new_page("chrome://clear_data").await {
            let error = BrowserError::from(e);
            match error {
                BrowserError::NetworkIO => {},
                _ => { return Err(error); }
            }
        }
        sleep(Duration::from_millis(150)).await;
        Ok(())  
    }

    /// Retrieve the current IP address by querying an IP information API
    ///
    /// This method opens a page to https://api.myip.com/ and attempts to parse 
    /// the JSON response containing IP address information.
    ///
    /// # Returns
    /// A `Result` containing the `MyIP` struct with IP details or a `BrowserError`
    ///
    /// # Errors
    /// Returns `BrowserError::Serialization` if:
    /// - Unable to find the body element
    /// - Unable to parse the JSON response
    ///
    /// # Examples
    /// ```rust
    /// let ip_info = session.myip().await?;
    /// println!("IP: {}", ip_info.ip);
    /// println!("Country: {}", ip_info.country);
    /// ```
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

/// List of user agents for randomization
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

/// Generate a random user agent
pub fn get_random_user_agent() -> &'static str {
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(0..USER_AGENT_LIST.len());
    USER_AGENT_LIST[index]
}