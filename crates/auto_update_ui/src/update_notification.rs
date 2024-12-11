use gpui::{
    div, AppContext, DismissEvent, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, SemanticVersion, StatefulInteractiveElement, Styled, WeakView,
};
use menu::Cancel;
use release_channel::ReleaseChannel;
use util::ResultExt;
use workspace::{
    ui::{h_flex, v_flex, Icon, IconName, Label, StyledExt},
    Workspace,
};

pub struct UpdateNotification {
    version: SemanticVersion,
    workspace: WeakModel<Workspace>,
}

impl EventEmitter<DismissEvent> for UpdateNotification {}

impl Render for UpdateNotification {
    fn render(
        &mut self,
        model: &Model<Self>,
        window: &mut gpui::Window,
        cx: &mut AppContext,
    ) -> impl IntoElement {
        let app_name = ReleaseChannel::global(cx).display_name();

        v_flex()
            .on_action(cx.listener(UpdateNotification::dismiss))
            .elevation_3(cx)
            .p_4()
            .child(
                h_flex()
                    .justify_between()
                    .child(Label::new(format!(
                        "Updated to {app_name} {}",
                        self.version
                    )))
                    .child(
                        div()
                            .id("cancel")
                            .child(Icon::new(IconName::Close))
                            .cursor_pointer()
                            .on_click(
                                model
                                    .listener(|this, model, _, cx| this.dismiss(&menu::Cancel, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .id("notes")
                    .child(Label::new("View the release notes"))
                    .cursor_pointer()
                    .on_click(model.listener(|this, model, _, cx| {
                        this.workspace
                            .update(cx, |workspace, model, cx| {
                                crate::view_release_notes_locally(workspace, model, cx);
                            })
                            .log_err();
                        this.dismiss(&menu::Cancel, cx)
                    })),
            )
    }
}

impl UpdateNotification {
    pub fn new(version: SemanticVersion, workspace: WeakModel<Workspace>) -> Self {
        Self { version, workspace }
    }

    pub fn dismiss(&mut self, _: &Cancel, model: &Model<Self>, cx: &mut AppContext) {
        model.emit(DismissEvent, cx);
    }
}
