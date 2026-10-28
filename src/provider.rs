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

impl Provider {
    pub fn toggle(self) -> Self {
        match self {
            Self::Gitlab => Self::Github,
            Self::Github => Self::Gitlab,
        }
    }
    pub fn demo_host(self) -> &'static str {
        match self {
            Self::Gitlab => "gitlab.example.org",
            Self::Github => "github.example.org",
        }
    }
    pub fn title(self, view: crate::model::View, language: crate::i18n::Language) -> &'static str {
        use crate::model::View;
        if self == Self::Github {
            match view {
                View::Projects => return language.text("Repositories", "Репозитории"),
                View::Groups => return language.text("Organizations", "Организации"),
                View::MergeRequests => return "Pull requests",
                View::MrDetails => return "Pull request",
                View::MrNotes => return language.text("Comments", "Комментарии"),
                View::Pipelines => return "Actions",
                _ => (),
            }
        }
        view.title_in(language)
    }
    pub fn mr_prefix(self) -> &'static str {
        if self == Self::Github { "#" } else { "!" }
    }
}
