use std::time::Duration;

use iced::widget::{Space, button, column, container, progress_bar, row, rule, scrollable, text};
use iced::{Element, Length, Subscription, Theme};

use crate::{
    app::{AppState, Page},
    config::{SettingsRepository, ThemePreference},
    domain::TaskState,
    platform::AppPaths,
};

#[derive(Debug, Clone)]
struct Launcher {
    state: AppState,
    settings_repository: SettingsRepository,
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
    };
    iced::application(move || app.clone(), Launcher::update, Launcher::view)
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
    fn update(&mut self, message: Message) {
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
        }
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
                text("No instance selected yet. Milestone 1 uses demonstration data and performs no network requests."),
                container(column![text("Vanilla demonstration instance").size(21), text("Version: not installed  •  Java: not configured"), button("Run progress demonstration").on_press(Message::StartDemo)].spacing(14)).padding(22),
            ].spacing(20).into(),
            Page::Instances => placeholder("Instances", "Create, isolate, repair, and launch installations. Instance storage arrives in Milestone 4."),
            Page::Downloads => self.downloads_page(),
            Page::Accounts => placeholder("Accounts", "Official Microsoft authentication will be implemented in Milestone 6. No credentials are collected yet."),
            Page::Logs => placeholder("Logs & console", "Structured launcher logs are stored on disk. Game-session filtering arrives with the launch engine."),
            Page::Settings => self.settings_page(),
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
        text("Milestone 1")
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
