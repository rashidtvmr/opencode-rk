#![forbid(unsafe_code)]
//! Home full flow: route plus render count.
//! TS truth `packages/tui/src/routes/home.tsx` (prompt-first layout).

use crate::home_footer::HomeFooter;
use crate::home_route::HomeRoute;
use crate::home_screen::home_lines;

/// Home route with a saturating render counter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeFlow {
    pub route: HomeRoute,
    views: u64,
}

impl HomeFlow {
    /// New flow wrapping `route`; views start at zero.
    #[must_use]
    pub fn new(route: HomeRoute) -> Self {
        Self { route, views: 0 }
    }

    /// Frame via [`home_lines`]; bumps views (saturating).
    pub fn render(&mut self, footer: &HomeFooter, w: usize, h: usize) -> Vec<String> {
        self.views = self.views.saturating_add(1);
        home_lines(&self.route, footer, w, h)
    }

    /// Renders served so far.
    #[must_use]
    pub fn views(&self) -> u64 {
        self.views
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow() -> (HomeFlow, HomeFooter) {
        let mut r = HomeRoute::new("/x");
        r.add_recent("/a".into());
        (HomeFlow::new(r), HomeFooter::new(vec!["tip".into()]))
    }

    #[test]
    fn new_zero_views() {
        let (f, _) = flow();
        assert_eq!(f.views(), 0);
    }

    #[test]
    fn render_matches_home_lines() {
        let (mut f, ft) = flow();
        let got = f.render(&ft, 40, 6);
        assert_eq!(got, home_lines(&f.route, &ft, 40, 6));
    }

    #[test]
    fn views_bump_per_render() {
        let (mut f, ft) = flow();
        f.render(&ft, 40, 6);
        f.render(&ft, 40, 6);
        assert_eq!(f.views(), 2);
    }

    #[test]
    fn route_mutation_reflected() {
        let (mut f, ft) = flow();
        f.route.set_cwd("/y");
        let out = f.render(&ft, 40, 6);
        assert!(out.contains(&"cwd /y".to_string()));
    }
}
