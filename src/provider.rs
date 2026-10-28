#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Provider {
    #[default]
    Gitlab,
    Github,
}

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Self::Gitlab => "GitLab",
            Self::Github => "GitHub",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Gitlab => "gitlab",
            Self::Github => "github",
        }
    }
    pub fn cli(self) -> &'static str {
        match self {
            Self::Gitlab => "glab",
            Self::Github => "gh",
        }
    }
    pub fn default_host(self) -> &'static str {
        match self {
            Self::Gitlab => "gitlab.com",
            Self::Github => "github.com",
        }
    }
}
