//! Custom selection routing and browser transport, included in the hint binary.
pub mod bridge;
pub mod install;
pub mod navigation;
pub mod support;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const USAGE: &str = "\
Custom commands:
  herdr-fingers open-selection < selection
  herdr-fingers browser serve|start|status
  herdr-fingers browser relay --client ID
  herdr-fingers install --role local|remote [--host SSH_ALIAS] [--herdr PATH]
  herdr-fingers install-browser-service
  herdr-fingers register-existing";

pub fn handles(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some(
            "open-selection"
                | "focus"
                | "browser"
                | "install"
                | "install-browser-service"
                | "register-existing"
        )
    )
}

pub fn run(args: &[String]) -> Result<()> {
    use std::io::Read;
    match args.first().map(String::as_str) {
        Some("open-selection") => {
            let mut text = String::new();
            std::io::stdin()
                .take(1024 * 1024 + 1)
                .read_to_string(&mut text)?;
            if text.len() > 1024 * 1024 {
                return Err("Selection is too large".into());
            }
            let result = navigation::open_selections(&text);
            if let Err(error) = &result
                && let Ok(host) = crate::adapters::herdr::HerdrClient::from_env()
            {
                let _ = host.notify("Fingers Open", &error.to_string());
            }
            result
        }
        Some("focus") if args.len() == 5 => {
            navigation::focus_after_close(&args[1], &args[2], &args[3], &args[4])
        }
        Some("browser") => bridge::run(&args[1..]),
        Some("install") => install::run(&args[1..]),
        Some("install-browser-service") => install::browser_service(),
        Some("register-existing") => install::register_existing(),
        _ => Err(USAGE.into()),
    }
}
