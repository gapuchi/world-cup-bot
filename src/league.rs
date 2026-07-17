/// Compile-time league types that this binary can run.
///
/// Adding a league is a code change: new variant, league module, and `match` arms.
/// Runtime guild setup creates **seasons** for a compiled-in league; it does not
/// register new leagues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum League {
    Wc,
}

impl League {
    pub const ALL: &[League] = &[League::Wc];

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "wc" => Some(Self::Wc),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Wc => "wc",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wc => "FIFA World Cup",
        }
    }

    /// Whether `/config season` (and related setup) may target this slug.
    pub fn supports_season(slug: &str) -> bool {
        Self::from_slug(slug).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::League;

    #[test]
    fn from_slug_resolves_compiled_leagues() {
        assert_eq!(League::from_slug("wc"), Some(League::Wc));
        assert_eq!(League::from_slug("wc").unwrap().slug(), "wc");
        assert_eq!(
            League::from_slug("wc").unwrap().display_name(),
            "FIFA World Cup"
        );
    }

    #[test]
    fn from_slug_rejects_unknown_and_catalog_only_slugs() {
        assert_eq!(League::from_slug("nfl"), None);
        assert_eq!(League::from_slug("nba"), None);
        assert_eq!(League::from_slug("unknown"), None);
        assert!(!League::supports_season("nfl"));
        assert!(League::supports_season("wc"));
    }

    #[test]
    fn all_lists_every_variant() {
        assert_eq!(League::ALL, &[League::Wc]);
    }
}
