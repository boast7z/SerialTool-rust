use iced::widget;
use iced::{Element, Event, Length, Subscription, Theme};

use crate::types::{DragTarget, Message};

/// 渲染一个可拖拽的分隔条（4px 宽透明细线）。
/// 悬停或拖拽时高亮显示；鼠标样式切换为水平缩放箭头，给用户明确的交互反馈。
pub fn split_handle<'a>(
    target: DragTarget,
    is_dragging: bool,
    is_hovering: bool,
) -> Element<'a, Message> {
    let highlight = is_dragging || is_hovering;
    // on_press 和 on_enter 各自的闭包会 move 所有权，因此需要两个独立的克隆。
    // 不能共用同一个变量——第一个闭包 move 之后，第二个闭包就无法再借用它。
    let target_clone = target.clone();
    let target_clone2 = target.clone();

    widget::mouse_area(
        widget::container(widget::space::Space::new().width(4).height(Length::Fill))
            .height(Length::Fill)
            .style(move |_theme: &Theme| {
                if highlight {
                    widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            0.3, 0.55, 0.95, 0.6,
                        ))),
                        border: iced::Border {
                            radius: 2.0.into(),
                            width: 0.0,
                            color: iced::Color::TRANSPARENT,
                        },
                        ..Default::default()
                    }
                } else {
                    widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                        border: iced::Border {
                            radius: 2.0.into(),
                            width: 0.0,
                            color: iced::Color::TRANSPARENT,
                        },
                        ..Default::default()
                    }
                }
            }),
    )
    .on_press(Message::DragStarted(target_clone))
    .on_enter(Message::HandleHoverChanged(Some(target_clone2)))
    .on_exit(Message::HandleHoverChanged(None))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

/// 拖拽期间的全局鼠标事件订阅。
/// 仅在 dragging.is_some() 时激活（监听鼠标移动和左键释放），否则返回 Subscription::none() 节省资源。
/// 使用全局事件而非控件事件，是因为用户可能把鼠标拖出分隔条区域，仍需捕获移动和释放。
pub fn drag_subscription(dragging: Option<DragTarget>) -> Subscription<Message> {
    if dragging.is_some() {
        iced::event::listen_with(|event, _status, _id| match event {
            Event::Mouse(mouse_event) => match mouse_event {
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left) => {
                    Some(Message::DragEnded)
                }
                iced::mouse::Event::CursorMoved { position } => {
                    Some(Message::DragMoved(position.x))
                }
                _ => None,
            },
            _ => None,
        })
    } else {
        Subscription::none()
    }
}
