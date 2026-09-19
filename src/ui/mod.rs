use std::{path::PathBuf, time::Duration};

use iced::widget::{Space, button, column, container, progress_bar, row, rule, scrollable, text};
use iced::{Element, Length, Subscription, Theme};

use crate::{
    app::{AppState, CatalogState, Page},
    config::{SettingsRepository, ThemePreference},
    domain::TaskState,
    minecraft::{CatalogSource, CatalogUpdate, VersionCatalogService, VersionKind},
    platform::AppPaths,
};

#[derive(Debug, Clone)]
struct Launcher {
    state: AppState,
    settings_repository: SettingsRepository,
    cache_root: PathBuf,
}

#[derive(Debug, Clone)]
enum Message {
    Navigate(Page),
    StartDemo,
    CancelDemo,
    Tick,
    DismissNotice,
    SetTheme(ThemePreference),
    ToggleSnapshots,
    RefreshCatalog,
    CatalogLoaded(Result<CatalogUpdate, String>),
}

pub fn run(paths: AppPaths) -> iced::Result {
    let repository = SettingsRepository::new(paths.config.join("settings.json"));
    let outcome = repository.load().unwrap_or_else(|error| {
        tracing::error!(%error, "could not load settings");
        crate::config::LoadOutcome {
            settings: Default::default(),
            recovery_notice: Some(format!("Settings could not be loaded: {error}")),
        }
    });
    let app = Launcher {
        state: AppState::new(outcome.settings, outcome.recovery_notice),
        settings_repository: repository,
        cache_root: paths.cache,
    };
    iced::application(
        move || {
            let mut initial = app.clone();
            initial.state.catalog = CatalogState::Loading;
            let task = refresh_catalog(initial.cache_root.clone());
            (initial, task)
        }, Launcher::update, Launcher::view)
        .title(Launcher::title)
        .subscription(Launcher::subscription)
        .theme(Launcher::theme)
        .window(iced::window::Settings {
            size: iced::Size::new(1100.0, 720.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            ..Default::default()
        })
        .run()
}

impl Launcher {
    fn title(&self) -> String {
        format!("Andromeda Client — {}", page_name(self.state.page))
    }
    fn theme(&self) -> Theme {
        match self.state.settings.theme {
            ThemePreference::Light => Theme::Light,
            ThemePreference::Dark => Theme::Dark,
            ThemePreference::System => Theme::TokyoNight,
        }
    }
    fn subscription(&self) -> Subscription<Message> {
        if self
            .state
            .task
            .as_ref()
            .is_some_and(|task| task.state == TaskState::Running)
        {
            iced::time::every(Duration::from_millis(200)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Navigate(page) => self.state.page = page,
            Message::StartDemo => {
                self.state.start_demo();
                self.state.page = Page::Downloads;
            }
            Message::CancelDemo => self.state.cancel_demo(),
            Message::Tick => self.state.tick(),
            Message::DismissNotice => self.state.recovery_notice = None,
            Message::SetTheme(theme) => {
                self.state.settings.theme = theme;
                self.persist_settings();
            }
            Message::ToggleSnapshots => {
                self.state.settings.show_snapshots = !self.state.settings.show_snapshots;
                self.persist_settings();
            }
            Message::RefreshCatalog => {
                self.state.catalog = CatalogState::Loading;
                return refresh_catalog(self.cache_root.clone());
            }
            Message::CatalogLoaded(result) => {
                self.state.catalog = match result {
                    Ok(update) => CatalogState::Ready {
                        manifest: update.manifest,
                        source: update.source,
                        warning: update.warning,
                    },
                    Err(error) => CatalogState::Failed(error),
                };
            }
        }
        iced::Task::none()
    }
    fn persist_settings(&mut self) {
        if let Err(error) = self.settings_repository.save(&self.state.settings) {
            tracing::error!(%error, "could not save settings");
            self.state.recovery_notice = Some(format!("Your settings could not be saved: {error}"));
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let nav = column![text("ANDROMEDA").size(24)]
            .spacing(10)
            .padding(18)
            .width(Length::Fixed(190.0))
            .push(nav_button("Home", Page::Home, self.state.page))
            .push(nav_button("Instances", Page::Instances, self.state.page))
            .push(nav_button("Downloads", Page::Downloads, self.state.page))
            .push(nav_button("Accounts", Page::Accounts, self.state.page))
            .push(nav_button("Logs", Page::Logs, self.state.page))
            .push(nav_button("Settings", Page::Settings, self.state.page))
            .push(Space::new().height(Length::Fill))
            .push(text("● Services ready").size(13));
        let mut content = column![page_header(page_name(self.state.page)), self.page_content()]
            .spacing(18)
            .padding(28);
        if let Some(notice) = &self.state.recovery_notice {
            content = content.push(
                container(
                    row![
                        text(notice).width(Length::Fill),
                        button("Dismiss").on_press(Message::DismissNotice)
                    ]
                    .spacing(12),
                )
                .padding(12),
            );
        }
        row![
            container(nav).height(Length::Fill),
            scrollable(content).width(Length::Fill).height(Length::Fill)
        ]
        .into()
    }
    fn page_content(&self) -> Element<'_, Message> {
        match self.state.page {
            Page::Home => column![
                text("Welcome to your Minecraft library").size(26),
                self.catalog_summary(),
                container(column![text("Vanilla demonstration instance").size(21), text("Version: not installed  •  Java: not configured"), button("Run progress demonstration").on_press(Message::StartDemo)].spacing(14)).padding(22),
            ].spacing(20).into(),
            Page::Instances => self.versions_page(),
            Page::Downloads => self.downloads_page(),
            Page::Accounts => placeholder("Accounts", "Official Microsoft authentication will be implemented in Milestone 6. No credentials are collected yet."),
            Page::Logs => placeholder("Logs & console", "Structured launcher logs are stored on disk. Game-session filtering arrives with the launch engine."),
            Page::Settings => self.settings_page(),
        }
    }
    fn versions_page(&self) -> Element<'_, Message> {
        let CatalogState::Ready { manifest, .. } = &self.state.catalog else {
            return column![
                text("Available Minecraft versions").size(22),
                text("The official catalog must load before versions can be displayed."),
                button("Refresh catalog").on_press(Message::RefreshCatalog),
            ]
            .spacing(12)
            .into();
        };

        let mut versions = column![
            text("Available Minecraft versions").size(22),
            text("Live entries from Mojang's manifest. Installation is implemented in Milestone 5."),
        ]
        .spacing(8);
        for version in manifest
            .versions
            .iter()
            .filter(|version| self.state.settings.show_snapshots || version.kind == VersionKind::Release)
            .take(40)
        {
            versions = versions.push(
                row![
                    text(&version.id).width(Length::Fixed(160.0)),
                    text(version.kind.to_string()).width(Length::Fixed(100.0)),
                    text(&version.release_time),
                ]
                .spacing(12),
            );
        }
        versions.into()
    }

    fn catalog_summary(&self) -> Element<'_, Message> {
        match &self.state.catalog {
            CatalogState::NotLoaded => text("Version catalog has not been loaded.").into(),
            CatalogState::Loading => row![text("Refreshing the official Minecraft catalog…"), button("Refresh").on_press(Message::RefreshCatalog)].spacing(12).into(),
            CatalogState::Failed(error) => container(column![text("Could not load Minecraft versions").size(20), text(error), button("Try again").on_press(Message::RefreshCatalog)].spacing(10)).padding(16).into(),
            CatalogState::Ready { manifest, source, warning } => {
                let releases = manifest.versions.iter().filter(|version| version.kind == VersionKind::Release).count();
                let snapshots = manifest.versions.iter().filter(|version| version.kind == VersionKind::Snapshot).count();
                let source = match source { CatalogSource::Network => "updated from Mojang", CatalogSource::Cache => "loaded from cache", CatalogSource::NotModified => "already current" };
                let mut details = column![
                    text(format!("Minecraft {} is the latest release", manifest.latest.release)).size(20),
                    text(format!("{} versions available ({} releases, {} snapshots) • {}", manifest.versions.len(), releases, snapshots, source)),
                    button("Refresh catalog").on_press(Message::RefreshCatalog),
                ].spacing(8);
                if let Some(warning) = warning { details = details.push(text(format!("Offline warning: {warning}"))); }
                container(details).padding(16).into()
            }
        }
    }

    fn downloads_page(&self) -> Element<'_, Message> {
        if let Some(task) = &self.state.task {
            let action = if task.state == TaskState::Running {
                button("Cancel").on_press(Message::CancelDemo)
            } else {
                button("Run again").on_press(Message::StartDemo)
            };
            column![
                text("Demonstration task").size(22),
                text(&task.phase),
                progress_bar(0.0..=1.0, task.percentage()),
                text(format!(
                    "{}% complete — {:?}",
                    (task.percentage() * 100.0) as u8,
                    task.state
                )),
                action
            ]
            .spacing(14)
            .into()
        } else {
            column![text("No active downloads").size(22), text("Use this bounded background-task simulation to validate responsive progress and cancellation."), button("Start demonstration").on_press(Message::StartDemo)].spacing(14).into()
        }
    }
    fn settings_page(&self) -> Element<'_, Message> {
        let snapshots = if self.state.settings.show_snapshots {
            "Snapshots: shown"
        } else {
            "Snapshots: hidden"
        };
        column![
            text("Appearance").size(21),
            row![button("Light").on_press(Message::SetTheme(ThemePreference::Light)), button("Dark").on_press(Message::SetTheme(ThemePreference::Dark)), button("System").on_press(Message::SetTheme(ThemePreference::System))].spacing(8),
            rule::horizontal(1), text("Minecraft catalog").size(21),
            text("All versions will be obtained from Mojang's live version manifest. New releases appear after automatic catalog refresh."),
            button(snapshots).on_press(Message::ToggleSnapshots),
            text(format!("Download concurrency: {}", self.state.settings.download_concurrency)),
        ].spacing(14).into()
    }
}

fn refresh_catalog(cache_root: PathBuf) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let service = VersionCatalogService::new(&cache_root)
                .map_err(|error| format!("could not initialize the Mojang HTTP client: {error}"))?;
            service.refresh().await
        },
        Message::CatalogLoaded,
    )
}

fn nav_button(label: &'static str, page: Page, selected: Page) -> Element<'static, Message> {
    let label = if page == selected {
        format!("› {label}")
    } else {
        label.into()
    };
    button(text(label).width(Length::Fill))
        .on_press(Message::Navigate(page))
        .width(Length::Fill)
        .into()
}
fn page_header(name: &str) -> Element<'_, Message> {
    row![
        text(name).size(32),
        Space::new().width(Length::Fill),
        text("Milestone 2")
    ]
    .align_y(iced::Alignment::Center)
    .into()
}
fn placeholder<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    container(column![text(title).size(22), text(body)].spacing(10))
        .padding(22)
        .into()
}
fn page_name(page: Page) -> &'static str {
    match page {
        Page::Home => "Home",
        Page::Instances => "Instances",
        Page::Downloads => "Downloads",
        Page::Accounts => "Accounts",
        Page::Logs => "Logs",
        Page::Settings => "Settings",
    }
}
