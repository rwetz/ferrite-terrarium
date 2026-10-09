use ferrite_design::prelude::*;
use gpui::{App, Context, IntoElement, Window, px};

use crate::{APPEARANCES, Terrarium};

impl Terrarium {
    pub(crate) fn settings_drawer(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let weak = cx.weak_entity();
        drawer("settings")
            .open(self.settings_open)
            .title("Settings")
            .width(px(420.))
            .on_close(move |_, cx: &mut App| {
                let _ = weak.update(cx, |this, cx| {
                    this.settings_open = false;
                    cx.notify();
                });
            })
            .child(rule(Some("scheme"), window, cx))
            .children(SCHEMES.iter().map(|scheme| {
                Button::new(gpui::ElementId::Name(format!("settings-scheme-{}", scheme.key).into()))
                    .label(scheme.name)
                    .secondary()
                    .selected(self.settings.scheme == scheme.key)
                    .on_click(cx.listener(move |this, _, window, cx| this.change(|s| s.scheme = scheme.key.into(), window, cx)))
            }))
            .child(
                field("appearance", "Appearance").child(
                    APPEARANCES
                        .iter()
                        .fold(segmented("appearance-seg"), |seg, (_, label)| seg.option(*label))
                        .selected(APPEARANCES.iter().position(|(key, _)| *key == self.settings.appearance).unwrap_or(0))
                        .on_select(
                            cx.listener(|this, i: &usize, window, cx| this.change(|s| s.appearance = APPEARANCES[*i].0.into(), window, cx)),
                        ),
                ),
            )
            .child(
                field("fps", "Refresh rate").child(
                    [25, 60, 120, 240]
                        .iter()
                        .fold(segmented("fps-seg"), |seg, fps| seg.option(fps.to_string()))
                        .selected([25, 60, 120, 240].iter().position(|fps| *fps == self.settings.fps).unwrap_or(0))
                        .on_select(cx.listener(|this, i: &usize, window, cx| this.change(|s| s.fps = [25, 60, 120, 240][*i], window, cx))),
                ),
            )
    }
}
