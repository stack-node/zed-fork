use crate::{TerminalView, build_terminal_icon_context_menu};
use anyhow::Result;
use editor::{Editor, actions::SelectAll};
use gpui::{
    App, AsyncWindowContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, ParentElement, Pixels, Render, SharedString, Styled, Subscription, WeakEntity,
    Window, actions, px,
};
use menu;
use project::Project;
use terminal::Terminal;
use ui::{
    Color, ContextMenu, FluentBuilder, Icon, IconButton, IconName, IconSize, Label, LabelSize,
    ListItem, Tooltip, prelude::*, right_click_menu,
};
use workspace::{
    Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};

const SERVICES_PANEL_KEY: &str = "ServicesPanel";
const MINI_TERMINAL_HEIGHT: Pixels = px(200.);

actions!(
    services_panel,
    [
        /// Toggles the services panel.
        Toggle,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _window, _cx| {
        workspace.register_action(|workspace, _: &Toggle, window, cx| {
            workspace.toggle_panel_focus::<ServicesPanel>(window, cx);
        });
    })
    .detach();
}

struct Service {
    name: SharedString,
    custom_icon: Option<IconName>,
    terminal: Entity<Terminal>,
    terminal_view: Option<Entity<TerminalView>>,
    is_expanded: bool,
    rename_editor: Option<Entity<Editor>>,
    rename_editor_subscription: Option<Subscription>,
}

pub struct ServicesPanel {
    services: Vec<Service>,
    workspace: WeakEntity<Workspace>,
    project: WeakEntity<Project>,
    position: DockPosition,
    focus_handle: FocusHandle,
}

impl EventEmitter<PanelEvent> for ServicesPanel {}

impl ServicesPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<Entity<Self>> {
        workspace.update_in(&mut cx, |workspace, window, cx| {
            cx.new(|cx| ServicesPanel::new(workspace, window, cx))
        })
    }

    pub fn new(workspace: &Workspace, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        Self {
            services: Vec::new(),
            workspace: workspace.weak_handle(),
            project: workspace.project().downgrade(),
            position: DockPosition::Right,
            focus_handle,
        }
    }

    pub fn add_service(
        &mut self,
        name: String,
        custom_icon: Option<IconName>,
        terminal: Entity<Terminal>,
        cx: &mut Context<Self>,
    ) {
        self.services.push(Service {
            name: name.into(),
            custom_icon,
            terminal,
            terminal_view: None,
            is_expanded: false,
            rename_editor: None,
            rename_editor_subscription: None,
        });
        cx.notify();
    }

    fn toggle_expand(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(service) = self.services.get_mut(index) {
            service.is_expanded = !service.is_expanded;
            // If collapsed, keep the terminal_view alive so it can be reused.
            cx.notify();
        }
    }

    fn remove_service(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.services.len() {
            self.services.remove(index);
            cx.notify();
        }
    }

    fn start_renaming(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.services.get(index) else {
            return;
        };
        let current_name = service.name.to_string();

        let rename_editor = cx.new(|cx| Editor::single_line(window, cx));
        let rename_editor_subscription = cx.subscribe_in(&rename_editor, window, {
            let rename_editor = rename_editor.clone();
            move |_this, _, event, window, cx| {
                if let editor::EditorEvent::Blurred = event {
                    let rename_editor = rename_editor.clone();
                    cx.defer_in(window, move |this, window, cx| {
                        let still_current = this
                            .services
                            .get(index)
                            .and_then(|service| service.rename_editor.as_ref())
                            .is_some_and(|current| current == &rename_editor);
                        if still_current && !rename_editor.focus_handle(cx).is_focused(window) {
                            this.finish_renaming(index, false, window, cx);
                        }
                    });
                }
            }
        });

        if let Some(service) = self.services.get_mut(index) {
            service.rename_editor = Some(rename_editor.clone());
            service.rename_editor_subscription = Some(rename_editor_subscription);
        }

        rename_editor.update(cx, |editor, cx| {
            editor.set_text(current_name, window, cx);
            editor.select_all(&SelectAll, window, cx);
            editor.focus_handle(cx).focus(window, cx);
        });
        cx.notify();
    }

    fn finish_renaming(
        &mut self,
        index: usize,
        save: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.services.get_mut(index) else {
            return;
        };
        let Some(editor) = service.rename_editor.take() else {
            return;
        };
        service.rename_editor_subscription = None;
        if save {
            let new_name = editor.read(cx).text(cx).trim().to_string();
            if !new_name.is_empty() {
                service.name = new_name.into();
            }
        }
        cx.notify();
    }

    fn set_service_icon(&mut self, index: usize, icon: Option<IconName>, cx: &mut Context<Self>) {
        let Some(service) = self.services.get_mut(index) else {
            return;
        };
        if service.custom_icon != icon {
            service.custom_icon = icon;
            cx.notify();
        }
    }
}

impl Focusable for ServicesPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ServicesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Lazily create terminal views for newly-expanded services.
        for service in &mut self.services {
            if service.is_expanded && service.terminal_view.is_none() {
                let workspace = self.workspace.clone();
                let project = self.project.clone();
                let terminal = service.terminal.clone();
                service.terminal_view =
                    Some(cx.new(|cx| {
                        TerminalView::new(terminal, workspace, None, project, window, cx)
                    }));
            }
        }

        // Collect display data before building elements to avoid borrow conflicts.
        let service_data: Vec<(
            usize,
            SharedString,
            Option<IconName>,
            bool,
            Option<Entity<Editor>>,
            Option<Entity<TerminalView>>,
        )> = self
            .services
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    i,
                    s.name.clone(),
                    s.custom_icon,
                    s.is_expanded,
                    s.rename_editor.clone(),
                    s.terminal_view.clone(),
                )
            })
            .collect();

        let is_empty = service_data.is_empty();
        let panel_entity = cx.entity();

        v_flex()
            .size_full()
            .gap_1()
            .py_1()
            .when(is_empty, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .px_4()
                        .child(
                            Label::new(
                                "No services. Right-click a terminal tab and select \"Convert To Service\".",
                            )
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                        ),
                )
            })
            .children(service_data.into_iter().map(
                |(index, name, custom_icon, is_expanded, rename_editor, terminal_view)| {
                    let toggle_icon = if is_expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    };
                    let row_icon = custom_icon.unwrap_or(IconName::Server);

                    v_flex()
                        .w_full()
                        .child(
                            right_click_menu(("service-row", index)).trigger({
                                let panel_entity = panel_entity.clone();
                                move |_, _, _| {
                                    ListItem::new(("service-row-item", index))
                                        .start_slot(
                                            h_flex()
                                                .gap_1()
                                                .child(
                                                    Icon::new(toggle_icon)
                                                        .size(IconSize::XSmall)
                                                        .color(Color::Muted),
                                                )
                                                .child(
                                                    Icon::new(row_icon)
                                                        .size(IconSize::Small)
                                                        .color(Color::Muted),
                                                ),
                                        )
                                        .end_slot({
                                            let panel = panel_entity.clone();
                                            IconButton::new(
                                                ("remove-service", index),
                                                IconName::Trash,
                                            )
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::text("Remove Service")(window, cx)
                                            })
                                            .on_click(move |_: &ClickEvent, _window, cx| {
                                                panel.update(cx, |this, cx| {
                                                    this.remove_service(index, cx);
                                                });
                                            })
                                        })
                                        .child(
                                            div()
                                                .relative()
                                                .child(
                                                    Label::new(name.clone())
                                                        .size(LabelSize::Small)
                                                        .when(rename_editor.is_some(), |this| {
                                                            this.alpha(0.)
                                                        }),
                                                )
                                                .when_some(rename_editor.clone(), |this, editor| {
                                                    let panel = panel_entity.clone();
                                                    let cancel_panel = panel_entity.clone();
                                                    this.child(
                                                        div()
                                                            .absolute()
                                                            .top_0()
                                                            .left_0()
                                                            .size_full()
                                                            .child(editor)
                                                            .on_action(
                                                                move |_: &menu::Confirm,
                                                                      window,
                                                                      cx| {
                                                                    panel.update(cx, |this, cx| {
                                                                        this.finish_renaming(
                                                                            index, true, window, cx,
                                                                        )
                                                                    });
                                                                },
                                                            )
                                                            .on_action(
                                                                move |_: &menu::Cancel,
                                                                      window,
                                                                      cx| {
                                                                    cancel_panel.update(
                                                                        cx,
                                                                        |this, cx| {
                                                                            this.finish_renaming(
                                                                                index,
                                                                                false,
                                                                                window,
                                                                                cx,
                                                                            )
                                                                        },
                                                                    );
                                                                },
                                                            ),
                                                    )
                                                }),
                                        )
                                        .on_click({
                                            let panel = panel_entity.clone();
                                            move |_: &ClickEvent, _window, cx| {
                                                panel.update(cx, |this, cx| {
                                                    this.toggle_expand(index, cx);
                                                });
                                            }
                                        })
                                }
                            })
                            .menu({
                                let panel = panel_entity.clone();
                                move |window, cx| {
                                    ContextMenu::build(window, cx, {
                                        let panel = panel.clone();
                                        move |menu, _window, _cx| {
                                            menu.entry("Rename", None, {
                                                let panel = panel.clone();
                                                move |window, cx| {
                                                    panel.update(cx, |this, cx| {
                                                        this.start_renaming(index, window, cx);
                                                    });
                                                }
                                            })
                                            .submenu_with_icon("Icon", row_icon, {
                                                let panel = panel.clone();
                                                move |menu, _window, _cx| {
                                                    build_terminal_icon_context_menu(
                                                        menu,
                                                        custom_icon,
                                                        {
                                                            let panel = panel.clone();
                                                            move |icon, _window, cx| {
                                                                panel.update(cx, |this, cx| {
                                                                    this.set_service_icon(
                                                                        index, icon, cx,
                                                                    );
                                                                });
                                                            }
                                                        },
                                                    )
                                                }
                                            })
                                            .separator()
                                            .entry("Remove Service", None, {
                                                let panel = panel.clone();
                                                move |_window, cx| {
                                                    panel.update(cx, |this, cx| {
                                                        this.remove_service(index, cx);
                                                    });
                                                }
                                            })
                                        }
                                    })
                                }
                            }),
                        )
                        .when(is_expanded, |this| {
                            this.when_some(terminal_view, |this, view| {
                                this.child(div().w_full().h(MINI_TERMINAL_HEIGHT).child(view))
                            })
                        })
                },
            ))
    }
}

impl Panel for ServicesPanel {
    fn persistent_name() -> &'static str {
        SERVICES_PANEL_KEY
    }

    fn panel_key() -> &'static str {
        SERVICES_PANEL_KEY
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        self.position
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        position: DockPosition,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.position = position;
        cx.notify();
    }

    fn default_size(&self, _window: &Window, _cx: &App) -> Pixels {
        px(300.)
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::Server)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Services Panel")
    }

    fn toggle_action(&self) -> Box<dyn gpui::Action> {
        Box::new(Toggle)
    }

    fn activation_priority(&self) -> u32 {
        4
    }
}
