use tauri::{Manager, Url};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Navigation {
    App,
    Blocked,
}

pub(crate) fn allowed_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "mailto")
}

pub(crate) fn navigation(url: &Url, dev_url: Option<&Url>) -> Navigation {
    let same_origin = |app: &Url| {
        url.scheme() == app.scheme()
            && url.host_str() == app.host_str()
            && url.port_or_known_default() == app.port_or_known_default()
    };
    let is_app = match dev_url {
        Some(app) => same_origin(app),
        None => same_origin(&Url::parse(crate::PACKAGED_APP_ORIGIN).expect("packaged app origin")),
    };
    if is_app {
        Navigation::App
    } else {
        Navigation::Blocked
    }
}

pub(crate) fn open(url: &str) -> Result<(), String> {
    open_with(url, |url| {
        tauri_plugin_opener::open_url(url, None::<&str>).map_err(|error| error.to_string())
    })?;
    println!("url opener launched=true");
    Ok(())
}

fn open_with(url: &str, opener: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|_| "The URL isn't valid.".to_owned())?;
    if !allowed_url(&parsed) {
        return Err("Only http, https, and mailto links can open.".into());
    }
    opener(parsed.as_str())
}

pub(crate) fn guard() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|webview, url| {
            let dev_url = tauri::is_dev()
                .then_some(webview.app_handle().config().build.dev_url.as_ref())
                .flatten();
            match navigation(url, dev_url) {
                Navigation::App => true,
                Navigation::Blocked => {
                    println!("url navigation refused=true");
                    false
                }
            }
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_disables_webkit_link_previews() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["app"]["windows"][0]["allowLinkPreview"], false);
    }

    #[test]
    fn the_opener_receives_the_normalized_url() {
        for (input, expected) in [
            ("\thttps://example.com", "https://example.com/"),
            ("https://exa\nmple.com/a b", "https://example.com/a%20b"),
        ] {
            let mut received = String::new();
            open_with(input, |url| {
                received = url.to_owned();
                Ok(())
            })
            .unwrap();
            assert_eq!(received, expected);
        }
    }

    #[test]
    fn only_web_and_mail_schemes_can_open() {
        for input in [
            "http://example.com",
            "https://example.com",
            "mailto:hello@example.com",
        ] {
            assert!(allowed_url(&Url::parse(input).unwrap()), "{input}");
        }
        for input in [
            "file:///tmp/private",
            "javascript:alert(1)",
            "data:text/html,test",
            "ftp://example.com",
            "tauri://localhost/page",
        ] {
            assert!(!allowed_url(&Url::parse(input).unwrap()), "{input}");
        }
        assert_eq!(
            open("file:///tmp/private"),
            Err("Only http, https, and mailto links can open.".into())
        );
        assert_eq!(open("not a url"), Err("The URL isn't valid.".into()));
    }

    #[test]
    fn navigation_permits_only_the_app_origin() {
        let decide = |input: &str| navigation(&Url::parse(input).unwrap(), None);
        assert_eq!(
            decide(&format!(
                "{}/page/page-a?block=a#b",
                crate::PACKAGED_APP_ORIGIN
            )),
            Navigation::App
        );
        for input in [
            "https://example.com",
            "http://localhost:9999",
            "mailto:hello@example.com",
            "http://tauri.localhost",
        ] {
            assert_eq!(decide(input), Navigation::Blocked, "{input}");
        }
        for input in [
            "tauri://example.com",
            "file:///tmp/page",
            "javascript:alert(1)",
            "about:blank",
            "data:text/html,test",
        ] {
            assert_eq!(decide(input), Navigation::Blocked, "{input}");
        }
    }

    #[test]
    fn development_allows_only_the_configured_origin() {
        let app = Url::parse("http://localhost:1420").unwrap();
        let decide = |input: &str| navigation(&Url::parse(input).unwrap(), Some(&app));
        assert_eq!(decide("http://localhost:1420/page/page-a"), Navigation::App);
        for input in [
            "http://localhost:1421",
            "https://localhost:1420",
            "http://127.0.0.1:1420",
            "http://localhost.example.com:1420",
        ] {
            assert_eq!(decide(input), Navigation::Blocked, "{input}");
        }
        assert_eq!(decide("tauri://localhost"), Navigation::Blocked);
    }
}
