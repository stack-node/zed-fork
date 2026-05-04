use gpui::{DismissEvent, Entity, EventEmitter, Focusable, ScrollHandle, SharedString};
use ui::prelude::*;
use ui::{Button, ButtonStyle, Color, Label, LabelSize, Modal, ModalFooter, ModalHeader, Section};
use ui_input::InputField;
use workspace::ModalView;

pub struct NewContainerModal {
    name_input: Entity<InputField>,
    description_input: Entity<InputField>,
    focus_handle: gpui::FocusHandle,
    error: Option<SharedString>,
}

impl EventEmitter<DismissEvent> for NewContainerModal {}
impl ModalView for NewContainerModal {}

impl Focusable for NewContainerModal {
    fn focus_handle(&self, _cx: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl NewContainerModal {
    pub fn new(window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> Self {
        let name_input = cx.new(|cx| {
            InputField::new(window, cx, "e.g. Work, Personal, etc.")
                .label("Container name")
                .tab_index(1)
                .tab_stop(true)
        });

        let description_input = cx.new(|cx| {
            InputField::new(window, cx, "Optional description")
                .label("Description")
                .tab_index(2)
                .tab_stop(true)
        });

        Self {
            name_input,
            description_input,
            focus_handle: cx.focus_handle(),
            error: None,
        }
    }

    fn create_container(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        let name = self.name_input.read(cx).text(cx).to_string();
        let description = self.description_input.read(cx).text(cx).to_string();

        if name.is_empty() {
            self.error = Some("Container name is required".into());
            cx.notify();
            return;
        }

        match crate::containers::create_container(&name, &description) {
            Ok(_) => {
                cx.emit(DismissEvent);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn cancel(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        cx.emit(DismissEvent);
    }
}

impl gpui::Render for NewContainerModal {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let mut section_children = vec![self.name_input.clone().into_any_element()];
        section_children.push(self.description_input.clone().into_any_element());

        if let Some(error) = self.error.clone() {
            section_children.push(Label::new(error).into_any_element());
        }

        v_flex()
            .id("new-container-modal")
            .key_context("NewContainerModal")
            .w(rems(32.))
            .elevation_3(cx)
            .child(
                Modal::new("new-container-modal", None::<ScrollHandle>)
                    .header(ModalHeader::new().headline("Create Container"))
                    .child(Section::new().children(section_children))
                    .footer(
                        ModalFooter::new().end_slot(
                            h_flex()
                                .gap_2()
                                .child(Button::new("cancel", "Cancel").on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.cancel(window, cx);
                                    },
                                )))
                                .child(
                                    Button::new("create", "Create")
                                        .style(ButtonStyle::Filled)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.create_container(window, cx);
                                        })),
                                ),
                        ),
                    ),
            )
    }
}

pub struct ManageContainersModal {
    containers: Vec<crate::containers::ContainerConfig>,
    focus_handle: gpui::FocusHandle,
}

impl EventEmitter<DismissEvent> for ManageContainersModal {}
impl ModalView for ManageContainersModal {}

impl Focusable for ManageContainersModal {
    fn focus_handle(&self, _cx: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl ManageContainersModal {
    pub fn new(_window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            containers: crate::containers::list_containers(),
            focus_handle: cx.focus_handle(),
        }
    }

    fn delete_container(&mut self, index: usize, cx: &mut gpui::Context<Self>) {
        if let Some(container) = self.containers.get(index) {
            if let Err(e) = crate::containers::delete_container(&container.name) {
                log::error!("Failed to delete container: {}", e);
            } else {
                self.containers.remove(index);
                cx.notify();
            }
        }
    }

    fn close(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        cx.emit(DismissEvent);
    }
}

impl gpui::Render for ManageContainersModal {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        v_flex()
            .id("manage-containers-modal")
            .key_context("ManageContainersModal")
            .w(rems(36.))
            .elevation_3(cx)
            .child(
                Modal::new("manage-containers-modal", None::<ScrollHandle>)
                    .header(ModalHeader::new().headline("Manage Containers"))
                    .child(
                        Section::new().children(self.containers.iter().enumerate().map(
                            |(i, container)| {
                                h_flex()
                                    .gap_2()
                                    .p_1()
                                    .rounded_md()
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .child(Label::new(container.name.clone()))
                                            .child(
                                                Label::new(container.description.clone())
                                                    .size(LabelSize::Small)
                                                    .color(Color::Muted),
                                            ),
                                    )
                                    .child(
                                        Button::new(format!("delete-{}", i), "Delete")
                                            .style(ButtonStyle::Outlined)
                                            .on_click({
                                                let i = i;
                                                cx.listener(move |this, _, _, cx| {
                                                    this.delete_container(i, cx);
                                                })
                                            }),
                                    )
                                    .into_any_element()
                            },
                        )),
                    )
                    .footer(
                        ModalFooter::new().end_slot(
                            Button::new("close", "Close")
                                .style(ButtonStyle::Filled)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.close(window, cx);
                                })),
                        ),
                    ),
            )
    }
}
