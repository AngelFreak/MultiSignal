//! Signal links (`sgnl://`, `signalcaptcha://`) and which profile gets them.
//! A captcha solved in the browser comes back as such a link and must reach
//! the Signal instance that asked for it.

use crate::profiles::Profile;

/// The link schemes Signal Desktop handles.
pub const SCHEMES: [&str; 2] = ["sgnl", "signalcaptcha"];

pub fn is_signal_link(uri: &str) -> bool {
    uri.split_once("://")
        .is_some_and(|(scheme, _)| SCHEMES.iter().any(|s| s.eq_ignore_ascii_case(scheme)))
}

/// Where a link goes: straight to one profile, or ask among these (by key).
#[derive(Debug, PartialEq)]
pub enum Route {
    To(String),
    Ask(Vec<String>),
}

/// The single running profile gets the link (it's the one that asked);
/// otherwise the user picks among the running ones, or all if none runs.
/// A locked default is never offered.
pub fn route(profiles: &[Profile]) -> Route {
    let usable: Vec<&Profile> = profiles.iter().filter(|p| !p.locked).collect();
    let running: Vec<&Profile> = usable.iter().copied().filter(|p| p.running).collect();
    match running.as_slice() {
        [only] => Route::To(only.name.clone()),
        [] => Route::Ask(usable.iter().map(|p| p.name.clone()).collect()),
        several => Route::Ask(several.iter().map(|p| p.name.clone()).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn profile(name: &str, running: bool) -> Profile {
        Profile {
            name: name.into(),
            title: name.into(),
            dir: PathBuf::from("/x").join(name),
            size_bytes: 0,
            running,
            launchers: Vec::new(),
            is_default: false,
            locked: false,
        }
    }

    #[test]
    fn recognises_signal_links() {
        assert!(is_signal_link("signalcaptcha://signal-hcaptcha.abc"));
        assert!(is_signal_link("sgnl://linkdevice?uuid=x"));
        assert!(is_signal_link("SGNL://x"));
        assert!(!is_signal_link("https://signal.org"));
        assert!(!is_signal_link("file:///tmp/x"));
    }

    #[test]
    fn the_only_running_profile_gets_the_link() {
        let ps = [profile("A", false), profile("B", true)];
        assert_eq!(route(&ps), Route::To("B".into()));
    }

    #[test]
    fn several_running_profiles_mean_asking_among_them() {
        let ps = [profile("A", true), profile("B", false), profile("C", true)];
        assert_eq!(route(&ps), Route::Ask(vec!["A".into(), "C".into()]));
    }

    #[test]
    fn nothing_running_means_asking_among_all() {
        let ps = [profile("A", false), profile("B", false)];
        assert_eq!(route(&ps), Route::Ask(vec!["A".into(), "B".into()]));
    }

    #[test]
    fn a_locked_default_never_gets_links() {
        let mut default = profile("Signal (default)", true);
        default.is_default = true;
        default.locked = true;
        let ps = [default, profile("A", false)];
        assert_eq!(route(&ps), Route::Ask(vec!["A".into()]));
    }
}
